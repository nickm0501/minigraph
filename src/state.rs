use std::sync::Arc;

use crate::metrics::Metrics;
use crate::rooms::RoomsHandle;
use crate::wal::WalReaderHandle;

#[derive(Clone)]
pub struct AppState {
    pub rooms: RoomsHandle,
    // TODO: This will be used by the WAL integration flow (stop/shutdown, debug, etc.).
    // For now we keep it in state to keep the WAL reader task alive.
    #[allow(dead_code)]
    pub wal_reader: WalReaderHandle,
    pub metrics: Arc<Metrics>,
}

impl AppState {
    pub fn new(rooms: RoomsHandle, wal_reader: WalReaderHandle, metrics: Arc<Metrics>) -> Self {
        crate::logging::vprintln(format_args!("[STATE] Creating new AppState"));
        AppState {
            rooms,
            wal_reader,
            metrics,
        }
    }
}
