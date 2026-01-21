use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use pgwire_replication::{
    client::ReplicationEvent, Lsn, ReplicationClient, ReplicationConfig, TlsConfig,
};
use tokio::sync::mpsc;

use crate::metrics::Metrics;
use crate::postgres::{self, PostgresConfig};
use crate::wal::{
    generate_invalidation_hints, parse_pgoutput_messages, pgoutput_to_wal_event, HintGenError,
    HintRouter, InvalidationSink, PgOutputConversionError, Relation, TransactionBuffer,
};

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
    pub(crate) fn start(
        config: PostgresConfig,
        sink: Arc<dyn InvalidationSink>,
        metrics: Arc<Metrics>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(WAL_READER_CHANNEL_CAPACITY);
        tokio::spawn(wal_reader_actor(rx, config, sink, metrics));
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
    config: PostgresConfig,
    sink: Arc<dyn InvalidationSink>,
    metrics: Arc<Metrics>,
) {
    crate::logging::vprintln(format_args!("[WAL] wal reader started"));

    let conn = match config.connection_info() {
        Ok(conn) => conn,
        Err(err) => {
            eprintln!("[WAL][ERR] invalid DATABASE_URL: {err}");
            return;
        }
    };

    // `Lsn(0)` means "start from slot's confirmed_flush_lsn".
    let start_lsn = Lsn(0);

    let slot_name = config.slot_name();
    let publication_name = config.publication_name.clone();

    let repl_cfg = ReplicationConfig {
        host: conn.host,
        port: conn.port,
        user: conn.user,
        password: conn.password.unwrap_or_default(),
        database: conn.database,
        tls: TlsConfig::disabled(),
        slot: slot_name.clone(),
        publication: publication_name,
        start_lsn,
        stop_at_lsn: None,

        status_interval: std::time::Duration::from_secs(10),
        idle_wakeup_interval: std::time::Duration::from_secs(10),
        buffer_events: 8192,
    };

    let mut repl = match ReplicationClient::connect(repl_cfg).await {
        Ok(client) => client,
        Err(err) => {
            eprintln!("[WAL][ERR] replication connect failed: {err}");
            return;
        }
    };

    const SLOT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
    const SLOT_POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);
    const SLOT_HEALTH_QUERY: &str = r#"
SELECT
  s.active,
  COALESCE(pg_wal_lsn_diff(pg_current_wal_lsn(), s.restart_lsn), 0)::bigint AS retained_bytes,
  COALESCE(EXTRACT(EPOCH FROM sr.flush_lag), 0)::bigint AS lag_seconds
FROM pg_replication_slots s
LEFT JOIN pg_stat_replication sr ON sr.pid = s.active_pid
WHERE s.slot_name = $1
"#;

    let sql_client = match postgres::connect(&config).await {
        Ok(client) => Some(client),
        Err(err) => {
            if let Some(db_err) = err.as_db_error() {
                eprintln!(
                    "[WAL][ERR] slot health connection failed: {} (SQLSTATE {:?})",
                    db_err.message(),
                    db_err.code()
                );
            } else {
                eprintln!("[WAL][ERR] slot health connection failed: {err:?}");
            }

            None
        }
    };

    let mut slot_poll = tokio::time::interval(SLOT_POLL_INTERVAL);
    slot_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut relations: HashMap<u32, Relation> = HashMap::new();
    let mut transaction = None::<TransactionBuffer>;

    loop {
        tokio::select! {
            cmd = rx.recv() => {
                match cmd {
                    Some(WalReaderCommand::Stop) | None => {
                        repl.stop();
                        break;
                    }
                }
            }
            _ = slot_poll.tick(), if sql_client.is_some() => {
                let Some(client) = sql_client.as_ref() else {
                    continue;
                };

                // `timeout(.., query_opt(..)).await` returns a nested result:
                // - outer `Err(_)` => timed out
                // - inner `Err(_)` => query failed
                // - `Ok(None)` => slot not found
                let row = match tokio::time::timeout(
                    SLOT_POLL_TIMEOUT,
                    client.query_opt(SLOT_HEALTH_QUERY, &[&slot_name]),
                )
                .await
                {
                    Ok(Ok(row)) => row,
                    Ok(Err(err)) => {
                        if let Some(db_err) = err.as_db_error() {
                            eprintln!(
                                "[WAL][ERR] slot health query failed: {} (SQLSTATE {:?})",
                                db_err.message(),
                                db_err.code()
                            );
                        } else {
                            eprintln!("[WAL][ERR] slot health query failed: {err:?}");
                        }

                        continue;
                    }
                    Err(_) => {
                        eprintln!("[WAL][ERR] slot health query timed out");
                        continue;
                    }
                };

                let Some(row) = row else {
                    metrics.set_wal_slot_active(false);
                    metrics.set_wal_retained_bytes(0);
                    metrics.set_wal_lag_seconds(0);
                    continue;
                };

                let active: bool = row.get(0);
                let retained_bytes: i64 = row.get(1);
                let lag_seconds: i64 = row.get(2);

                metrics.set_wal_slot_active(active);
                metrics.set_wal_retained_bytes(retained_bytes.max(0) as u64);
                metrics.set_wal_lag_seconds(lag_seconds.max(0) as u64);
            }
            ev = repl.recv() => {
                match ev {
                    Ok(Some(ReplicationEvent::XLogData { data, .. })) => {
                        crate::logging::vprintln(format_args!("[WAL] XLogData bytes={}", data.len()));

                        match parse_pgoutput_messages(&data) {
                            Ok(messages) => {
                                for msg in messages {
                                    let event = match pgoutput_to_wal_event(&mut relations, msg) {
                                        Ok(ev) => ev,
                                        Err(PgOutputConversionError::SchemaChange) => return,
                                    };

                                    let Some(event) = event else {
                                        continue;
                                    };

                                    metrics.inc_wal_events_consumed();

                                    let Some(tx) = transaction.as_mut() else {
                                        eprintln!("[WAL][ERR] received row change outside of BEGIN/COMMIT; ignoring");
                                        continue;
                                    };

                                    match generate_invalidation_hints(&event) {
                                        Ok(hints) => {
                                            tx.add_all(hints);
                                        }
                                        Err(HintGenError::MissingColumn { table, column }) => {
                                            eprintln!("[WAL][ERR] missing column for hints: {table}.{column}");
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                eprintln!("[WAL][ERR] failed to parse pgoutput messages: {err}");
                            }
                        }
                    }
                    Ok(Some(ReplicationEvent::Begin { .. })) => {
                        transaction = Some(TransactionBuffer::default());
                    }
                    Ok(Some(ReplicationEvent::Commit { end_lsn, .. })) => {
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        if let Some(mut tx) = transaction.take() {
                            let hints = tx.take();
                            let routed = HintRouter::route(hints);

                            for (document_id, hints) in routed {
                                match sink.send_invalidation(document_id, hints, timestamp) {
                                    Ok(()) => {}
                                    Err(err) => {
                                        if matches!(err, crate::types::RoomCommandError::ChannelFull) {
                                            metrics.inc_wal_events_dropped();
                                        }

                                        eprintln!("[WAL][ERR] failed to deliver invalidation: {err}");
                                    }
                                }
                            }
                        }

                        metrics.set_wal_lsn(end_lsn.0);

                        // Best-effort delivery; we still advance LSN.
                        repl.update_applied_lsn(end_lsn);
                    }
                    Ok(Some(ReplicationEvent::KeepAlive { wal_end, reply_requested, .. })) => {
                        crate::logging::vprintln(format_args!("[WAL] KeepAlive wal_end={wal_end} reply_requested={reply_requested}"));
                    }
                    Ok(Some(ReplicationEvent::StoppedAt { reached })) => {
                        crate::logging::vprintln(format_args!("[WAL] StoppedAt reached={reached}"));
                        break;
                    }
                    Ok(None) => {
                        crate::logging::vprintln(format_args!("[WAL] replication ended"));
                        break;
                    }
                    Err(err) => {
                        eprintln!("[WAL][ERR] replication recv failed: {err}");
                        break;
                    }
                }
            }
        }
    }

    crate::logging::vprintln(format_args!("[WAL] wal reader stopped"));
}
