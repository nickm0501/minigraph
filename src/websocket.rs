use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::state::AppState;
use crate::types::{ClientMessage, DocumentId, ServerMessage};

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
            let json = serde_json::to_string(&msg).unwrap();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    // Receive loop
    let client_id_clone = client_id.clone();
    let state_clone = state.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                println!("Received from {}: {}", client_id_clone, text);

                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(ClientMessage::Join { document_id }) => {
                        println!("[WS] Client {} joining room '{}'", client_id_clone, document_id);

                        state_clone.join_room(document_id.clone(), client_id_clone.clone(), tx.clone()).await;
                        current_room = Some(document_id.clone());

                        let response = ServerMessage::Joined {
                            client_id: client_id_clone.clone(),
                            document_id,
                        };

                        let _ = tx.send(response);
                    }
                    Ok(ClientMessage::SendMessage { text }) => {
                        if let Some(ref room) = current_room {
                            println!("[WS] Client {} sending message to room '{}'", client_id_clone, room);

                            let message = ServerMessage::new_message(client_id_clone.clone(), text);
                            state_clone.broadcast_to_room(room, message).await;
                        } else {
                            println!("[WS] Client {} tried to send message without joining a room", client_id_clone);

                            let error = ServerMessage::Error {
                                message: "Must join a room before sending messages".to_string(),
                            };
                            let _ = tx.send(error);
                        }
                    }
                    Err(e) => {
                        println!("[WS] Failed to parse message from {}: {}", client_id_clone, e);

                        let error = ServerMessage::Error {
                            message: format!("Invalid message format: {}", e),
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
            let (client_id, room) = result.unwrap();
            println!("Client disconnected: {}", client_id);

            if let Some(document_id) = room {
                state.leave_room(&document_id, &client_id).await;
            }

            send_task.abort();
        }
        _ = &mut send_task => {
            println!("Send task completed for client");
            recv_task.abort();
        }
    }
}
