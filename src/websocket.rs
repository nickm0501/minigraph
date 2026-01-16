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

async fn handle_client_message(
    msg: ClientMessage,
    client_id: &str,
    current_room: &mut Option<DocumentId>,
    state: &AppState,
    tx: &mpsc::Sender<ServerMessage>,
) -> Result<(), WebSocketError> {
    match msg {
        ClientMessage::Join { document_id } => {
            println!("[WS] Client {} joining room '{}'", client_id, document_id);

            let (respond_to, respond_rx) = tokio::sync::oneshot::channel();
            state.rooms.join_room(
                document_id.clone(),
                client_id.to_string(),
                tx.clone(),
                respond_to,
            )?;

            const JOIN_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);
            match tokio::time::timeout(JOIN_TIMEOUT, respond_rx).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) => return Err(WebSocketError::SendFailed),
                Err(_) => return Err(WebSocketError::SendFailed),
            }

            *current_room = Some(document_id.clone());

            let response = ServerMessage::Joined {
                client_id: client_id.to_string(),
                document_id,
            };

            let _ = tx.try_send(response);
            Ok(())
        }
        ClientMessage::SendMessage { text } => {
            if let Some(ref room) = current_room {
                println!(
                    "[WS] Client {} sending message to room '{}'",
                    client_id, room
                );

                let message = ServerMessage::new_message(client_id.to_string(), text);
                state.rooms.broadcast_to_room(room.clone(), message)?;
                Ok(())
            } else {
                println!(
                    "[WS] Client {} tried to send message without joining a room",
                    client_id
                );
                Err(WebSocketError::NotInRoom)
            }
        }
    }
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let client_id = Uuid::new_v4().to_string();
    println!("Client connected: {}", client_id);

    let (mut sender, mut receiver) = socket.split();

    let (tx, mut rx) = mpsc::channel::<ServerMessage>(256);

    let mut current_room: Option<DocumentId> = None;

    // Loop in a thread and receive messages in an channel
    // and send them back to the WS client.
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

    // Loop in a thread and receive messages from the client
    let client_id_clone = client_id.clone();
    let state_clone = state.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                println!("Received from {}: {}", client_id_clone, text);

                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(client_msg) => {
                        let result = handle_client_message(
                            client_msg,
                            &client_id_clone,
                            &mut current_room,
                            &state_clone,
                            &tx,
                        )
                        .await;

                        if let Err(err) = result {
                            let error = ServerMessage::Error {
                                message: err.to_string(),
                            };
                            let _ = tx.try_send(error);
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
                        let _ = tx.try_send(error);
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
                        let _ = state.rooms.leave_room(document_id, client_id);
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
