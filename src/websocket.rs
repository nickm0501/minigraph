use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::state::AppState;
use crate::types::{ClientMessage, DocumentId, ServerMessage, WebSocketError};

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let client_id = Uuid::new_v4().to_string();
    println!("Client connected: {}", client_id);

    let (mut sender, mut receiver) = socket.split();

    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    let mut current_room: Option<DocumentId> = None;

    // Spawn send task
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match serde_json::to_string(&msg) {
                Ok(json) => {
                    if sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    println!("[WS] Failed to serialize message: {}", e);
                    break;
                }
            }
        }
    });

    // Receive loop
    let client_id_clone = client_id.clone();
    // Do we need to clone here, based on how axum handles State(state) in the handler, doesn't
    // that clone for us??
    let state_clone = state.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                println!("Received from {}: {}", client_id_clone, text);

                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(ClientMessage::Join { document_id }) => {
                        println!(
                            "[WS] Client {} joining room '{}'",
                            client_id_clone, document_id
                        );

                        state_clone
                            .join_room(document_id.clone(), client_id_clone.clone(), tx.clone())
                            .await;
                        current_room = Some(document_id.clone());

                        let response = ServerMessage::Joined {
                            client_id: client_id_clone.clone(),
                            document_id,
                        };

                        let _ = tx.send(response);
                    }
                    Ok(ClientMessage::SendMessage { text }) => {
                        let result = if let Some(ref room) = current_room {
                            println!(
                                "[WS] Client {} sending message to room '{}'",
                                client_id_clone, room
                            );

                            let message = ServerMessage::new_message(client_id_clone.clone(), text);
                            state_clone.broadcast_to_room(room, message).await;
                            Ok(())
                        } else {
                            println!(
                                "[WS] Client {} tried to send message without joining a room",
                                client_id_clone
                            );
                            Err(WebSocketError::NotInRoom)
                        };

                        if let Err(err) = result {
                            let error = ServerMessage::Error {
                                message: err.to_string(),
                            };
                            let _ = tx.send(error);
                        }
                    }
                    Err(e) => {
                        println!(
                            "[WS] Failed to parse message from {}: {}",
                            client_id_clone, e
                        );

                        let err = WebSocketError::InvalidMessage(e.to_string());
                        let error = ServerMessage::Error {
                            message: err.to_string(),
                        };
                        let _ = tx.send(error);
                    }
                }
            }
        }
        (client_id_clone, current_room)
    });

    // Wait for either task to finish
    tokio::select! {
        result = &mut recv_task => {
            match result {
                Ok((client_id, room)) => {
                    println!("Client disconnected: {}", client_id);

                    if let Some(document_id) = room {
                        state.leave_room(&document_id, &client_id).await;
                    }
                }
                Err(e) => {
                    println!("[WS] Receive task error: {}", e);
                }
            }

            send_task.abort();
        }
        _ = &mut send_task => {
            println!("Send task completed for client");
            recv_task.abort();
        }
    }
}
