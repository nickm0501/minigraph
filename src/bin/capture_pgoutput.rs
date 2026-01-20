use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Parser;
use pgwire_replication::{
    client::ReplicationEvent, Lsn, ReplicationClient, ReplicationConfig, TlsConfig,
};

use mini_graph::postgres::{self, PostgresConfig};

#[derive(Debug, Parser)]
#[command(name = "capture_pgoutput")]
struct Args {
    /// Output file path for raw pgoutput bytes.
    ///
    /// This writes the XLogData payloads exactly as mini-graph receives them and passes them
    /// into `parse_pgoutput_messages`.
    #[arg(long, default_value = "tests/fixtures/pgoutput_capture.bin")]
    out: PathBuf,

    /// Maximum number of XLogData payloads to write.
    #[arg(long, default_value_t = 1)]
    max_xlog: usize,

    /// Stop after this many seconds even if `max_xlog` wasn't reached.
    #[arg(long, default_value_t = 30)]
    timeout_secs: u64,

    /// Run the repo's Postgres setup (tables, publication, slot) before capturing.
    #[arg(long, default_value_t = true)]
    setup: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    let pg_config = PostgresConfig::from_env();

    if args.setup {
        postgres::setup_postgres(&pg_config).await?;
    }

    if let Some(parent) = args.out.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let mut out = fs::File::create(&args.out)?;

    let conn = pg_config.connection_info()?;

    let repl_cfg = ReplicationConfig {
        host: conn.host,
        port: conn.port,
        user: conn.user,
        password: conn.password.unwrap_or_default(),
        database: conn.database,
        tls: TlsConfig::disabled(),
        slot: pg_config.slot_name(),
        publication: pg_config.publication_name.clone(),
        start_lsn: Lsn(0),
        stop_at_lsn: None,
        status_interval: Duration::from_secs(10),
        idle_wakeup_interval: Duration::from_secs(10),
        buffer_events: 8192,
    };

    println!("[CAPTURE] connecting to Postgres logical replication...");
    println!(
        "[CAPTURE] slot={} publication={} out={}",
        repl_cfg.slot,
        repl_cfg.publication,
        args.out.display()
    );

    let mut repl = ReplicationClient::connect(repl_cfg).await?;

    let deadline = Instant::now() + Duration::from_secs(args.timeout_secs);
    let mut written = 0usize;

    while written < args.max_xlog {
        if Instant::now() >= deadline {
            break;
        }

        let ev = tokio::select! {
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => None,
            ev = repl.recv() => Some(ev),
        };

        let Some(ev) = ev else {
            break;
        };

        match ev? {
            Some(ReplicationEvent::XLogData { data, .. }) => {
                out.write_all(&data)?;
                out.flush()?;

                written += 1;
                println!(
                    "[CAPTURE] wrote xlog payload {}/{} bytes={}",
                    written,
                    args.max_xlog,
                    data.len()
                );
            }
            Some(ReplicationEvent::KeepAlive { .. }) => {}
            Some(_) => {}
            None => break,
        }
    }

    repl.stop();

    println!("[CAPTURE] done; wrote {} payload(s)", written);

    Ok(())
}
