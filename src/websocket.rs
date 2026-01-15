use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

pub async fn websocket_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(socket: WebSocket) {
    let client_id = Uuid::new_v4();
    println!("Client connected: {}", client_id);

    let (mut sender, mut receiver) = socket.split();

    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Spawn send task
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Receive loop
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                println!("Received from {}: {}", client_id, text);
            }
        }
        client_id
    });

    // Wait for either task to finish
    tokio::select! {
        client_id = &mut recv_task => {
            println!("Client disconnected: {}", client_id.unwrap());
            send_task.abort();
        }
        _ = &mut send_task => {
            println!("Send task completed for client");
            recv_task.abort();
        }
    }
}
