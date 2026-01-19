use std::fmt;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::metrics::Metrics;
use crate::postgres::PostgresConfig;
use crate::rooms::RoomsHandle;

const WAL_READER_CHANNEL_CAPACITY: usize = 256;

#[derive(Debug)]
pub enum WalReaderCommandError {
    ChannelFull,
    ChannelClosed,
}

impl fmt::Display for WalReaderCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WalReaderCommandError::ChannelFull => write!(f, "WAL reader command channel is full"),
            WalReaderCommandError::ChannelClosed => {
                write!(f, "WAL reader command channel is closed")
            }
        }
    }
}

impl std::error::Error for WalReaderCommandError {}

#[derive(Clone)]
pub struct WalReaderHandle {
    tx: mpsc::Sender<WalReaderCommand>,
}

impl WalReaderHandle {
    pub(crate) fn start(config: PostgresConfig, rooms: RoomsHandle, metrics: Arc<Metrics>) -> Self {
        let (tx, rx) = mpsc::channel(WAL_READER_CHANNEL_CAPACITY);
        tokio::spawn(wal_reader_actor(rx, config, rooms, metrics));
        Self { tx }
    }

    pub fn stop(&self) -> Result<(), WalReaderCommandError> {
        self.tx
            .try_send(WalReaderCommand::Stop)
            .map_err(|err| match err {
                mpsc::error::TrySendError::Full(_) => WalReaderCommandError::ChannelFull,
                mpsc::error::TrySendError::Closed(_) => WalReaderCommandError::ChannelClosed,
            })
    }
}

enum WalReaderCommand {
    Stop,
}

async fn wal_reader_actor(
    mut rx: mpsc::Receiver<WalReaderCommand>,
    _config: PostgresConfig,
    _rooms: RoomsHandle,
    _metrics: Arc<Metrics>,
) {
    crate::logging::vprintln(format_args!("[WAL] wal reader started"));

    // Placeholder loop until replication streaming is implemented.
    // We keep a real loop here (instead of waiting on Stop only) because the
    // final implementation will concurrently:
    // - read from Postgres replication stream
    // - handle stop/shutdown commands
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            cmd = rx.recv() => {
                match cmd {
                    Some(WalReaderCommand::Stop) | None => break,
                }
            }
            _ = tick.tick() => {
                crate::logging::vprintln(format_args!("[WAL] wal reader idle"));
            }
        }
    }

    crate::logging::vprintln(format_args!("[WAL] wal reader stopped"));
}
