use tokio::sync::{mpsc, oneshot};

use crate::rooms::RoomCommand;
use crate::types::{ClientId, DocumentId, ServerMessage, WebSocketError};

#[derive(Clone)]
pub struct RoomsHandle {
    tx: mpsc::UnboundedSender<RoomCommand>,
}

impl RoomsHandle {
    pub(crate) fn new(tx: mpsc::UnboundedSender<RoomCommand>) -> Self {
        Self { tx }
    }

    pub fn join_room(
        &self,
        room: DocumentId,
        client_id: ClientId,
        client_tx: mpsc::UnboundedSender<ServerMessage>,
        respond_to: oneshot::Sender<()>,
    ) -> Result<(), WebSocketError> {
        self.tx
            .send(RoomCommand::Join {
                room,
                client_id,
                tx: client_tx,
                respond_to,
            })
            .map_err(|_| WebSocketError::SendFailed)
    }

    pub fn leave_room(&self, room: DocumentId, client_id: ClientId) -> Result<(), WebSocketError> {
        self.tx
            .send(RoomCommand::Leave { room, client_id })
            .map_err(|_| WebSocketError::SendFailed)
    }

    pub fn broadcast_to_room(
        &self,
        room: DocumentId,
        message: ServerMessage,
    ) -> Result<(), WebSocketError> {
        self.tx
            .send(RoomCommand::Broadcast { room, message })
            .map_err(|_| WebSocketError::SendFailed)
    }
}

#[derive(Clone)]
pub struct AppState {
    pub rooms: RoomsHandle,
}

impl AppState {
    pub fn new(rooms_tx: mpsc::UnboundedSender<RoomCommand>) -> Self {
        println!("[STATE] Creating new AppState");
        AppState {
            rooms: RoomsHandle::new(rooms_tx),
        }
    }
}
