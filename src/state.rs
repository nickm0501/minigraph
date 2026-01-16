use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use crate::metrics::Metrics;
use crate::rooms::RoomCommand;
use crate::types::{ClientId, DocumentId, ServerMessage, WebSocketError};

#[derive(Clone)]
pub struct RoomsHandle {
    tx: mpsc::Sender<RoomCommand>,
    metrics: Arc<Metrics>,
}

impl RoomsHandle {
    pub(crate) fn new(tx: mpsc::Sender<RoomCommand>, metrics: Arc<Metrics>) -> Self {
        Self { tx, metrics }
    }

    pub fn join_room(
        &self,
        room: DocumentId,
        client_id: ClientId,
        client_tx: mpsc::Sender<ServerMessage>,
        respond_to: oneshot::Sender<()>,
    ) -> Result<(), WebSocketError> {
        self.tx
            .try_send(RoomCommand::Join {
                room,
                client_id,
                tx: client_tx,
                respond_to,
            })
            .map_err(|err| {
                if matches!(err, mpsc::error::TrySendError::Full(_)) {
                    self.metrics.inc_actor_cmd_drop();
                }
                WebSocketError::SendFailed
            })
    }

    pub fn leave_room(&self, room: DocumentId, client_id: ClientId) -> Result<(), WebSocketError> {
        self.tx
            .try_send(RoomCommand::Leave { room, client_id })
            .map_err(|err| {
                if matches!(err, mpsc::error::TrySendError::Full(_)) {
                    self.metrics.inc_actor_cmd_drop();
                }
                WebSocketError::SendFailed
            })
    }

    pub fn broadcast_to_room(
        &self,
        room: DocumentId,
        message: ServerMessage,
    ) -> Result<(), WebSocketError> {
        self.tx
            .try_send(RoomCommand::Broadcast { room, message })
            .map_err(|err| {
                if matches!(err, mpsc::error::TrySendError::Full(_)) {
                    self.metrics.inc_actor_cmd_drop();
                }
                WebSocketError::SendFailed
            })
    }
}

#[derive(Clone)]
pub struct AppState {
    pub rooms: RoomsHandle,
    pub metrics: Arc<Metrics>,
}

impl AppState {
    pub fn new(rooms_tx: mpsc::Sender<RoomCommand>, metrics: Arc<Metrics>) -> Self {
        crate::logging::vprintln(format_args!("[STATE] Creating new AppState"));
        AppState {
            rooms: RoomsHandle::new(rooms_tx, metrics.clone()),
            metrics,
        }
    }
}
