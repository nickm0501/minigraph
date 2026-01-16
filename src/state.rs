use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use crate::metrics::Metrics;
use crate::rooms::RoomCommand;
use crate::types::{ClientId, DocumentId, RoomCommandError, ServerMessage};

#[derive(Clone)]
pub struct RoomsHandle {
    tx: mpsc::Sender<RoomCommand>,
}

impl RoomsHandle {
    pub(crate) fn new(tx: mpsc::Sender<RoomCommand>) -> Self {
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

#[derive(Clone)]
pub struct AppState {
    pub rooms: RoomsHandle,
    pub metrics: Arc<Metrics>,
}

impl AppState {
    pub fn new(rooms_tx: mpsc::Sender<RoomCommand>, metrics: Arc<Metrics>) -> Self {
        crate::logging::vprintln(format_args!("[STATE] Creating new AppState"));
        AppState {
            rooms: RoomsHandle::new(rooms_tx),
            metrics,
        }
    }
}
