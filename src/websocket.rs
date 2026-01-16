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
use crate::types::{ClientId, ClientMessage, DocumentId, ServerMessage, WebSocketError};

struct Session {
    client_id: ClientId,
    current_room: tokio::sync::Mutex<Option<DocumentId>>,
    tx: mpsc::Sender<ServerMessage>,
}

impl Session {
    fn new(client_id: ClientId, tx: mpsc::Sender<ServerMessage>) -> Self {
        Self {
            client_id,
            current_room: tokio::sync::Mutex::new(None),
            tx,
        }
    }
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_client_message(
    msg: ClientMessage,
    state: &AppState,
    session: &Session,
) -> Result<(), WebSocketError> {
    match msg {
        ClientMessage::Join { document_id } => {
            crate::logging::vprintln(format_args!(
                "[WS] Client {} joining room '{}'",
                session.client_id, document_id
            ));

            let (respond_to, respond_rx) = tokio::sync::oneshot::channel();
            state.rooms.join_room(
                document_id.clone(),
                session.client_id.clone(),
                session.tx.clone(),
                respond_to,
            )?;

            const JOIN_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);
            match tokio::time::timeout(JOIN_TIMEOUT, respond_rx).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) => return Err(WebSocketError::SendFailed),
                Err(_) => return Err(WebSocketError::SendFailed),
            }

            {
                let mut room = session.current_room.lock().await;
                *room = Some(document_id.clone());
            }

            let response = ServerMessage::Joined {
                client_id: session.client_id.clone(),
                document_id,
            };

            let _ = session.tx.try_send(response);
            Ok(())
        }
        ClientMessage::SendMessage { text } => {
            let room = { session.current_room.lock().await.clone() };

            if let Some(room) = room {
                crate::logging::vprintln(format_args!(
                    "[WS] Client {} sending message to room '{}'",
                    session.client_id, room
                ));

                let message = ServerMessage::new_message(session.client_id.clone(), text);
                state.rooms.broadcast_to_room(room, message)?;
                Ok(())
            } else {
                crate::logging::vprintln(format_args!(
                    "[WS] Client {} tried to send message without joining a room",
                    session.client_id
                ));
                Err(WebSocketError::NotInRoom)
            }
        }
        ClientMessage::SendMessageTo { document_id, text } => {
            crate::logging::vprintln(format_args!(
                "[WS] Client {} sending message to room '{}'",
                session.client_id, document_id
            ));

            let message = ServerMessage::new_message(session.client_id.clone(), text);
            state.rooms.broadcast_to_room(document_id, message)?;
            Ok(())
        }
    }
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let client_id = Uuid::new_v4().to_string();
    crate::logging::vprintln(format_args!("Client connected: {}", client_id));

    let (mut sender, mut receiver) = socket.split();

    let (tx, mut rx) = mpsc::channel::<ServerMessage>(256);
    let session = std::sync::Arc::new(Session::new(client_id.clone(), tx));

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
                    println!("[WS][ERR] Failed to serialize message: {}", e);
                    break;
                }
            }
        }
    });

    // Loop in a thread and receive messages from the client
    let state_clone = state.clone();
    let session_clone = session.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                crate::logging::vprintln(format_args!(
                    "Received from {}: {}",
                    session_clone.client_id, text
                ));

                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(client_msg) => {
                        let result =
                            handle_client_message(client_msg, &state_clone, &session_clone).await;

                        if let Err(err) = result {
                            let error = ServerMessage::Error {
                                message: err.to_string(),
                            };
                            let _ = session_clone.tx.try_send(error);
                        }
                    }
                    Err(e) => {
                        println!(
                            "[WS][ERR] Failed to parse message from {}: {}",
                            session_clone.client_id, e
                        );

                        let err = WebSocketError::InvalidMessage(e.to_string());
                        let error = ServerMessage::Error {
                            message: err.to_string(),
                        };
                        let _ = session_clone.tx.try_send(error);
                    }
                }
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        result = &mut recv_task => {
            if let Err(e) = result {
                println!("[WS][ERR] Receive task error: {}", e);
            }

            send_task.abort();
        }
        _ = &mut send_task => {
            crate::logging::vprintln(format_args!("Send task completed for client"));
            recv_task.abort();
        }
    }

    let room = { session.current_room.lock().await.clone() };
    if let Some(document_id) = room {
        let _ = state.rooms.leave_room(document_id, client_id);
    }
}
