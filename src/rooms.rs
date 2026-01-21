use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use crate::metrics::Metrics;
use crate::types::{ClientId, DocumentId, RoomCommandError, ServerMessage};

const ROOMS_COORDINATOR_CHANNEL_CAPACITY: usize = 256;

#[derive(Clone)]
pub struct RoomsHandle {
    tx: mpsc::Sender<RoomCommand>,
}

impl RoomsHandle {
    pub(crate) fn start(metrics: Arc<Metrics>) -> Self {
        let (tx, rx) = mpsc::channel(ROOMS_COORDINATOR_CHANNEL_CAPACITY);
        tokio::spawn(rooms_coordinator(rx, metrics));
        Self { tx }
    }

    pub fn join_room(
        &self,
        room: DocumentId,
        client_id: ClientId,
        client_tx: mpsc::Sender<ServerMessage>,
        respond_to: oneshot::Sender<()>,
    ) -> Result<(), RoomCommandError> {
        self.tx
            .try_send(RoomCommand::Join {
                room,
                client_id,
                tx: client_tx,
                respond_to,
            })
            .map_err(|err| match err {
                mpsc::error::TrySendError::Full(_) => RoomCommandError::ChannelFull,
                mpsc::error::TrySendError::Closed(_) => RoomCommandError::ChannelClosed,
            })
    }

    pub fn leave_room(
        &self,
        room: DocumentId,
        client_id: ClientId,
    ) -> Result<(), RoomCommandError> {
        self.tx
            .try_send(RoomCommand::Leave { room, client_id })
            .map_err(|err| match err {
                mpsc::error::TrySendError::Full(_) => RoomCommandError::ChannelFull,
                mpsc::error::TrySendError::Closed(_) => RoomCommandError::ChannelClosed,
            })
    }

    pub fn broadcast_to_room(
        &self,
        room: DocumentId,
        message: ServerMessage,
    ) -> Result<(), RoomCommandError> {
        self.tx
            .try_send(RoomCommand::Broadcast { room, message })
            .map_err(|err| match err {
                mpsc::error::TrySendError::Full(_) => RoomCommandError::ChannelFull,
                mpsc::error::TrySendError::Closed(_) => RoomCommandError::ChannelClosed,
            })
    }
}

enum RoomCommand {
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

async fn rooms_coordinator(mut rx: mpsc::Receiver<RoomCommand>, metrics: Arc<Metrics>) {
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

                crate::logging::vprintln(format_args!(
                    "[ROOMS] Client {} joined room '{}' (now {} clients)",
                    client_id,
                    room,
                    clients.len()
                ));

                let _ = respond_to.send(());
            }
            RoomCommand::Leave { room, client_id } => {
                if let Some(clients) = rooms.get_mut(&room) {
                    clients.retain(|(id, _)| id != &client_id);

                    crate::logging::vprintln(format_args!(
                        "[ROOMS] Client {} left room '{}' ({} remaining)",
                        client_id,
                        room,
                        clients.len()
                    ));

                    if clients.is_empty() {
                        rooms.remove(&room);
                        crate::logging::vprintln(format_args!(
                            "[ROOMS] Room '{}' is now empty",
                            room
                        ));
                    }
                }
            }
            RoomCommand::Broadcast { room, message } => {
                if let Some(clients) = rooms.get_mut(&room) {
                    let mut failed_clients = Vec::new();

                    crate::logging::vprintln(format_args!(
                        "[ROOMS] Broadcasting to room '{}' ({} clients)",
                        room,
                        clients.len()
                    ));

                    for (client_id, tx) in clients.iter() {
                        match tx.try_send(message.clone()) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                // Drop newest message for this slow client.
                                metrics.inc_fanout_drop();
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                println!("[ROOMS][ERR] Client channel closed: {}", client_id);
                                failed_clients.push(client_id.clone());
                            }
                        }
                    }

                    if !failed_clients.is_empty() {
                        clients.retain(|(id, _)| !failed_clients.contains(id));

                        if clients.is_empty() {
                            rooms.remove(&room);
                            crate::logging::vprintln(format_args!(
                                "[ROOMS] Room '{}' emptied after cleanup",
                                room
                            ));
                        }
                    }
                }
            }
        }
    }
}
