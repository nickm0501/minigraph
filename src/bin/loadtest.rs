use std::time::Duration;

use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use hdrhistogram::Histogram;
use mini_graph::{metrics::MetricsSnapshot, types::ClientMessage, types::ServerMessage};
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Parser)]
#[command(name = "loadtest")]
struct Args {
    /// WebSocket URL (server must be running)
    #[arg(long, default_value = "ws://127.0.0.1:3030/ws")]
    ws_url: String,

    /// HTTP base URL for debug metrics
    #[arg(long, default_value = "http://127.0.0.1:3030")]
    http_base_url: String,

    /// Document/room ID to join
    #[arg(long, default_value = "doc")]
    document_id: String,

    /// Number of subscriber clients
    #[arg(long, default_value_t = 100)]
    subscribers: usize,

    /// Producer send rate (messages per second total)
    #[arg(long, default_value_t = 1000)]
    producer_rate: u64,

    /// Test duration in seconds
    #[arg(long, default_value_t = 10)]
    duration_secs: u64,

    /// Time to keep subscribers draining after producer stops.
    #[arg(long, default_value_t = 500)]
    drain_ms: u64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Payload {
    seq: u64,
    sent_at_us: u64,
}

#[derive(Debug)]
struct SubscriberResult {
    received: u64,
    seq_gaps: u64,
    latency_hist: Histogram<u64>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let metrics_url = format!("{}/debug/metrics", args.http_base_url.trim_end_matches('/'));

    let start_metrics = fetch_metrics(&metrics_url)
        .await
        .expect("Failed to fetch initial /debug/metrics");

    let (start_tx, start_rx) = tokio::sync::watch::channel::<Option<tokio::time::Instant>>(None);

    let (joined_tx, mut joined_rx) = tokio::sync::mpsc::channel::<()>(args.subscribers);
    let mut subscriber_handles = Vec::with_capacity(args.subscribers);

    for _ in 0..args.subscribers {
        let ws_url = args.ws_url.clone();
        let document_id = args.document_id.clone();
        let joined_tx = joined_tx.clone();
        let start_rx = start_rx.clone();
        let duration = Duration::from_secs(args.duration_secs);
        let drain = Duration::from_millis(args.drain_ms);

        subscriber_handles.push(tokio::spawn(async move {
            run_subscriber(ws_url, document_id, joined_tx, start_rx, duration, drain).await
        }));
    }

    drop(joined_tx);

    for _ in 0..args.subscribers {
        joined_rx
            .recv()
            .await
            .expect("Subscriber tasks dropped before joining");
    }

    let start = tokio::time::Instant::now();
    let _ = start_tx.send(Some(start));

    let test_end = start + Duration::from_secs(args.duration_secs);

    let producer_handle = tokio::spawn(run_producer(
        args.ws_url.clone(),
        args.document_id.clone(),
        args.producer_rate,
        test_end,
    ));

    let sent = producer_handle
        .await
        .expect("Producer task panicked")
        .expect("Producer task failed");

    let mut total_received = 0u64;
    let mut total_seq_gaps = 0u64;
    let mut merged_hist = new_latency_hist().expect("Failed to create histogram");

    for handle in subscriber_handles {
        let result = handle
            .await
            .expect("Subscriber task panicked")
            .expect("Subscriber task failed");
        total_received += result.received;
        total_seq_gaps += result.seq_gaps;
        merged_hist
            .add(&result.latency_hist)
            .expect("Failed to merge histogram");
    }

    let end_metrics = fetch_metrics(&metrics_url)
        .await
        .expect("Failed to fetch final /debug/metrics");

    let actor_cmd_drops = end_metrics
        .actor_cmd_drops_total
        .saturating_sub(start_metrics.actor_cmd_drops_total);
    let fanout_drops = end_metrics
        .fanout_drops_total
        .saturating_sub(start_metrics.fanout_drops_total);

    println!("=== Load test report ===");

    println!(
        "settings duration_secs={} drain_ms={} subscribers={} producer_rate_msgs_per_sec={} document_id={} ws_url={} http_base_url={}",
        args.duration_secs,
        args.drain_ms,
        args.subscribers,
        args.producer_rate,
        args.document_id,
        args.ws_url,
        args.http_base_url
    );

    println!("\n-- Throughput --");
    println!(
        "producer_sent_expected_total={}",
        args.producer_rate.saturating_mul(args.duration_secs)
    );
    println!("producer_sent_total={}", sent);
    println!("subscriber_received_total={}", total_received);
    println!("subscriber_seq_gaps_total={}", total_seq_gaps);
    println!("server_actor_cmd_drops_total={}", actor_cmd_drops);
    println!("server_fanout_drops_total={}", fanout_drops);

    println!("\n-- Latency (ms) --");
    if merged_hist.is_empty() {
        println!("latency_ms_count=0");
    } else {
        println!(
            "latency_ms_p50={:.3}",
            decode_latency_us(merged_hist.value_at_quantile(0.50)) as f64 / 1000.0
        );
        println!(
            "latency_ms_p95={:.3}",
            decode_latency_us(merged_hist.value_at_quantile(0.95)) as f64 / 1000.0
        );
        println!(
            "latency_ms_p99={:.3}",
            decode_latency_us(merged_hist.value_at_quantile(0.99)) as f64 / 1000.0
        );
        println!(
            "latency_ms_max={:.3}",
            decode_latency_us(merged_hist.max()) as f64 / 1000.0
        );
    }

    println!("\n-- Server resources --");
    println!("server_samples_total={}", end_metrics.server_samples_total);

    println!(
        "server_cpu_pct_last={:.2}",
        end_metrics.server_cpu_usage_x100 as f64 / 100.0
    );
    println!(
        "server_cpu_pct_avg={:.2}",
        end_metrics.server_cpu_usage_x100_avg as f64 / 100.0
    );
    println!(
        "server_cpu_pct_max={:.2}",
        end_metrics.server_cpu_usage_x100_max as f64 / 100.0
    );

    println!(
        "server_memory_mib_last={:.1}",
        end_metrics.server_memory_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "server_memory_mib_avg={:.1}",
        end_metrics.server_memory_bytes_avg as f64 / (1024.0 * 1024.0)
    );
    println!(
        "server_memory_mib_max={:.1}",
        end_metrics.server_memory_bytes_max as f64 / (1024.0 * 1024.0)
    );
}

async fn fetch_metrics(url: &str) -> Result<MetricsSnapshot, reqwest::Error> {
    reqwest::get(url).await?.json::<MetricsSnapshot>().await
}

async fn run_subscriber(
    ws_url: String,
    document_id: String,
    joined_tx: tokio::sync::mpsc::Sender<()>,
    mut start_rx: tokio::sync::watch::Receiver<Option<tokio::time::Instant>>,
    duration: Duration,
    drain: Duration,
) -> Result<SubscriberResult, Box<dyn std::error::Error + Send + Sync>> {
    let (ws, _) = tokio_tungstenite::connect_async(&ws_url).await?;
    let (mut sink, mut stream) = ws.split();

    let join = serde_json::to_string(&ClientMessage::Join { document_id })?;
    sink.send(Message::Text(join)).await?;

    let join_deadline = tokio::time::Instant::now() + Duration::from_secs(5);

    loop {
        let maybe_msg = tokio::select! {
            _ = tokio::time::sleep_until(join_deadline) => None,
            msg = stream.next() => msg,
        };

        let Some(Ok(msg)) = maybe_msg else {
            return Ok(SubscriberResult {
                received: 0,
                seq_gaps: 0,
                latency_hist: new_latency_hist()?,
            });
        };

        if let Message::Text(text) = msg {
            if let Ok(ServerMessage::Joined { .. }) = serde_json::from_str::<ServerMessage>(&text) {
                break;
            }
        }
    }

    let _ = joined_tx.send(()).await;

    while start_rx.borrow().is_none() {
        if start_rx.changed().await.is_err() {
            return Ok(SubscriberResult {
                received: 0,
                seq_gaps: 0,
                latency_hist: new_latency_hist()?,
            });
        }
    }

    let start = start_rx
        .borrow()
        .expect("start instant must be set")
        .to_owned();
    let drain_end = start + duration + drain;

    let mut received = 0u64;
    let mut seq_gaps = 0u64;
    let mut last_seq: Option<u64> = None;
    let mut latency_hist = new_latency_hist()?;

    loop {
        let maybe_msg = tokio::select! {
            _ = tokio::time::sleep_until(drain_end) => None,
            msg = stream.next() => msg,
        };

        let Some(Ok(msg)) = maybe_msg else {
            break;
        };

        if let Message::Text(text) = msg {
            let Ok(ServerMessage::Message { text, .. }) =
                serde_json::from_str::<ServerMessage>(&text)
            else {
                continue;
            };

            let Ok(payload) = serde_json::from_str::<Payload>(&text) else {
                continue;
            };

            received += 1;

            if let Some(prev) = last_seq {
                if payload.seq > prev + 1 {
                    seq_gaps += payload.seq - prev - 1;
                }
            }
            last_seq = Some(payload.seq);

            let now_us = now_micros();
            if now_us >= payload.sent_at_us {
                let latency_us = now_us - payload.sent_at_us;
                // hdrhistogram doesn't accept 0 by default; store +1.
                let _ = latency_hist.record(latency_us.saturating_add(1));
            }
        }
    }

    Ok(SubscriberResult {
        received,
        seq_gaps,
        latency_hist,
    })
}

async fn run_producer(
    ws_url: String,
    document_id: String,
    rate_msgs_per_sec: u64,
    deadline: tokio::time::Instant,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let (ws, _) = tokio_tungstenite::connect_async(&ws_url).await?;
    let (mut sink, mut stream) = ws.split();

    // Drain and ignore any server messages (e.g., errors).
    tokio::spawn(async move { while let Some(Ok(_)) = stream.next().await {} });

    if rate_msgs_per_sec == 0 {
        tokio::time::sleep_until(deadline).await;
        return Ok(0);
    }

    let interval_ns = 1_000_000_000u64 / rate_msgs_per_sec;
    let mut interval = tokio::time::interval(Duration::from_nanos(interval_ns.max(1)));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Burst);

    let mut sent = 0u64;

    while tokio::time::Instant::now() < deadline {
        interval.tick().await;

        let payload = Payload {
            seq: sent,
            sent_at_us: now_micros(),
        };

        let text = serde_json::to_string(&payload)?;
        let msg = serde_json::to_string(&ClientMessage::SendMessageTo {
            document_id: document_id.clone(),
            text,
        })?;

        if sink.send(Message::Text(msg)).await.is_err() {
            break;
        }

        sent += 1;
    }

    Ok(sent)
}

const LATENCY_HIST_MAX_US: u64 = 60_000_000;

fn new_latency_hist() -> Result<Histogram<u64>, hdrhistogram::errors::CreationError> {
    Histogram::<u64>::new_with_bounds(1, LATENCY_HIST_MAX_US.saturating_add(1), 3)
}

fn decode_latency_us(encoded_us_plus_one: u64) -> u64 {
    encoded_us_plus_one.saturating_sub(1)
}

fn now_micros() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_micros() as u64
}
