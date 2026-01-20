use std::sync::Arc;

use tokio::sync::mpsc;

use crate::rooms::RoomsHandle;
use crate::types::{DocumentId, RoomCommandError, ServerMessage};
use crate::wal::QueryHint;

pub trait InvalidationSink: Send + Sync + 'static {
    fn send_invalidation(
        &self,
        document_id: DocumentId,
        hints: Vec<QueryHint>,
        timestamp: u64,
    ) -> Result<(), RoomCommandError>;
}

#[derive(Clone)]
pub struct RoomsInvalidationSink {
    rooms: RoomsHandle,
}

impl RoomsInvalidationSink {
    pub fn new(rooms: RoomsHandle) -> Self {
        Self { rooms }
    }
}

impl InvalidationSink for RoomsInvalidationSink {
    fn send_invalidation(
        &self,
        document_id: DocumentId,
        hints: Vec<QueryHint>,
        timestamp: u64,
    ) -> Result<(), RoomCommandError> {
        let message = ServerMessage::Invalidation {
            hints: hints.into_iter().map(|h| h.to_key()).collect(),
            timestamp,
        };

        self.rooms.broadcast_to_room(document_id, message)
    }
}

// Convenience for callers that already store their sink behind Arc.
impl<T: InvalidationSink> InvalidationSink for Arc<T> {
    fn send_invalidation(
        &self,
        document_id: DocumentId,
        hints: Vec<QueryHint>,
        timestamp: u64,
    ) -> Result<(), RoomCommandError> {
        (**self).send_invalidation(document_id, hints, timestamp)
    }
}

// Compile-time assertion that the messages we produce are cheap to queue.
// If this ever changes, it impacts our backpressure/drop behavior.
const _: () = {
    fn _assert_clone<T: Clone>() {}
    fn _check() {
        _assert_clone::<ServerMessage>();
        _assert_clone::<mpsc::Sender<ServerMessage>>();
    }
};
