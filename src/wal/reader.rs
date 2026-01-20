use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use pgwire_replication::{
    client::ReplicationEvent, Lsn, ReplicationClient, ReplicationConfig, TlsConfig,
};
use tokio::sync::mpsc;

use crate::metrics::Metrics;
use crate::postgres::PostgresConfig;
use crate::wal::{
    generate_invalidation_hints, parse_pgoutput_messages, HintGenError, HintRouter,
    InvalidationSink, PgOutputMessage, Relation, TransactionBuffer, TupleData, Value, WalEvent,
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
    _metrics: Arc<Metrics>,
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

    let repl_cfg = ReplicationConfig {
        host: conn.host,
        port: conn.port,
        user: conn.user,
        password: conn.password.unwrap_or_default(),
        database: conn.database,
        tls: TlsConfig::disabled(),
        slot: config.slot_name(),
        publication: config.publication_name,
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
            ev = repl.recv() => {
                match ev {
                    Ok(Some(ReplicationEvent::XLogData { data, .. })) => {
                        crate::logging::vprintln(format_args!("[WAL] XLogData bytes={}", data.len()));

                        match parse_pgoutput_messages(&data) {
                            Ok(messages) => {
                                for msg in messages {
                                    let event = match pgoutput_to_wal_event(&mut relations, msg) {
                                        Ok(ev) => ev,
                                        Err(PgoutputAction::SchemaChange) => return,
                                    };

                                    let Some(event) = event else {
                                        continue;
                                    };

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
                        let timestamp = now_millis();

                        if let Some(mut tx) = transaction.take() {
                            let hints = tx.take();
                            let routed = HintRouter::route(hints);

                            for (document_id, hints) in routed {
                                if let Err(err) = sink.send_invalidation(document_id, hints, timestamp) {
                                    eprintln!("[WAL][ERR] failed to deliver invalidation: {err}");
                                }
                            }
                        }

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

enum PgoutputAction {
    SchemaChange,
}

fn pgoutput_to_wal_event(
    relations: &mut HashMap<u32, Relation>,
    msg: PgOutputMessage,
) -> Result<Option<WalEvent>, PgoutputAction> {
    match msg {
        PgOutputMessage::Relation(rel) => {
            if let Some(existing) = relations.get(&rel.id) {
                if existing != &rel {
                    eprintln!(
                        "[WAL][ERR] schema change detected for relation_id={} ({}.{}); restart required",
                        rel.id, rel.namespace, rel.name
                    );
                    return Err(PgoutputAction::SchemaChange);
                }

                // Same relation message seen again; ignore.
                return Ok(None);
            }

            crate::logging::vprintln(format_args!(
                "[WAL] relation {}.{} id={} columns={}",
                rel.namespace,
                rel.name,
                rel.id,
                rel.columns.len()
            ));

            relations.insert(rel.id, rel);
            Ok(None)
        }
        PgOutputMessage::Insert {
            relation_id,
            new_values,
        } => {
            let Some(rel) = relations.get(&relation_id) else {
                eprintln!("[WAL][ERR] insert for unknown relation_id={relation_id}");
                return Ok(None);
            };

            let tuple = tuple_data_from_values(rel, new_values);
            let event = WalEvent::Insert {
                relation_id,
                relation_name: rel.name.clone(),
                new_tuple: tuple,
            };

            crate::logging::vprintln(format_args!("[WAL] parsed: {event:?}"));
            Ok(Some(event))
        }
        PgOutputMessage::Update {
            relation_id,
            old_values,
            new_values,
        } => {
            let Some(rel) = relations.get(&relation_id) else {
                eprintln!("[WAL][ERR] update for unknown relation_id={relation_id}");
                return Ok(None);
            };

            let old_tuple = old_values.map(|values| tuple_data_from_values(rel, values));
            let new_tuple = tuple_data_from_values(rel, new_values);

            let event = WalEvent::Update {
                relation_id,
                relation_name: rel.name.clone(),
                old_tuple,
                new_tuple,
            };

            crate::logging::vprintln(format_args!("[WAL] parsed: {event:?}"));
            Ok(Some(event))
        }
        PgOutputMessage::Delete {
            relation_id,
            old_values,
        } => {
            let Some(rel) = relations.get(&relation_id) else {
                eprintln!("[WAL][ERR] delete for unknown relation_id={relation_id}");
                return Ok(None);
            };

            let old_tuple = tuple_data_from_values(rel, old_values);
            let event = WalEvent::Delete {
                relation_id,
                relation_name: rel.name.clone(),
                old_tuple,
            };

            crate::logging::vprintln(format_args!("[WAL] parsed: {event:?}"));
            Ok(Some(event))
        }
    }
}

fn tuple_data_from_values(relation: &Relation, values: Vec<Value>) -> TupleData {
    let mut columns = std::collections::HashMap::new();

    for (idx, column_name) in relation.columns.iter().enumerate() {
        let value = values.get(idx).cloned().unwrap_or(Value::Null);
        columns.insert(column_name.clone(), value);
    }

    TupleData { columns }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
