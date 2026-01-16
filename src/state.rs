use std::sync::Arc;

use crate::metrics::Metrics;
use crate::rooms::RoomsHandle;

#[derive(Clone)]
pub struct AppState {
    pub rooms: RoomsHandle,
    pub metrics: Arc<Metrics>,
}

impl AppState {
    pub fn new(rooms: RoomsHandle, metrics: Arc<Metrics>) -> Self {
        crate::logging::vprintln(format_args!("[STATE] Creating new AppState"));
        AppState { rooms, metrics }
    }
}
