use std::collections::HashMap;

use tokio::sync::{mpsc, oneshot};

use crate::types::{ClientId, DocumentId, ServerMessage};

pub(crate) enum RoomCommand {
    Join {
        room: DocumentId,
        client_id: ClientId,
        tx: mpsc::Sender<ServerMessage>,
        respond_to: oneshot::Sender<()>,
    },
    Leave {
        room: DocumentId,
        client_id: ClientId,
    },
    Broadcast {
        room: DocumentId,
        message: ServerMessage,
    },
}

pub(crate) async fn rooms_actor(mut rx: mpsc::Receiver<RoomCommand>) {
    let mut rooms: HashMap<DocumentId, Vec<(ClientId, mpsc::Sender<ServerMessage>)>> =
        HashMap::new();

    while let Some(cmd) = rx.recv().await {
        match cmd {
            RoomCommand::Join {
                room,
                client_id,
                tx,
                respond_to,
            } => {
                let clients = rooms.entry(room.clone()).or_default();
                clients.push((client_id.clone(), tx));

                println!(
                    "[ACTOR] Client {} joined room '{}' (now {} clients)",
                    client_id,
                    room,
                    clients.len()
                );

                let _ = respond_to.send(());
            }
            RoomCommand::Leave { room, client_id } => {
                if let Some(clients) = rooms.get_mut(&room) {
                    clients.retain(|(id, _)| id != &client_id);

                    println!(
                        "[ACTOR] Client {} left room '{}' ({} remaining)",
                        client_id,
                        room,
                        clients.len()
                    );

                    if clients.is_empty() {
                        rooms.remove(&room);
                        println!("[ACTOR] Room '{}' is now empty", room);
                    }
                }
            }
            RoomCommand::Broadcast { room, message } => {
                if let Some(clients) = rooms.get_mut(&room) {
                    let mut failed_clients = Vec::new();

                    println!(
                        "[ACTOR] Broadcasting to room '{}' ({} clients)",
                        room,
                        clients.len()
                    );

                    for (client_id, tx) in clients.iter() {
                        match tx.try_send(message.clone()) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                // Drop newest message for this slow client.
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                println!("[ACTOR] Client channel closed: {}", client_id);
                                failed_clients.push(client_id.clone());
                            }
                        }
                    }

                    if !failed_clients.is_empty() {
                        clients.retain(|(id, _)| !failed_clients.contains(id));

                        if clients.is_empty() {
                            rooms.remove(&room);
                            println!("[ACTOR] Room '{}' emptied after cleanup", room);
                        }
                    }
                }
            }
        }
    }
}
