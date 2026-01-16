# MiniGraph

A toy project to implement a [LiveGraph](https://www.figma.com/blog/livegraph-real-time-data-at-scale/)-like project. Minimal, room-based Websocket server built with Rust, Axum, and Tokio.

## Run

This repo has two binaries:

- `mini-graph`: the server
- `loadtest`: the WebSocket load test harness

### Server

- Start the server: `cargo run --bin mini-graph`
- WebSocket endpoint: `ws://localhost:3030/ws`
- Debug metrics (JSON): `http://localhost:3030/debug/metrics`

Verbose logging:

- Default: enabled for debug builds (`cargo run`)
- Disable: `VERBOSE=0 cargo run --bin mini-graph`

## Manual Websocket Testing (wscat)

In one terminal:
- Connect: `wscat -c ws://localhost:3030/ws`
- Join a room: `{"type":"join","document_id":"doc1"}`
- Send a message (to current room): `{"type":"send_message","text":"hello"}`
- Send a message (explicit room): `{"type":"send_message_to","document_id":"doc1","text":"hello"}`

In a second terminal:
- Connect: `wscat -c ws://localhost:3030/ws`
- Join the same room: `{"type":"join","document_id":"doc1"}`

Messages sent from either client should be broadcast to both clients in the same room.

## Load testing

Start the server in one terminal:

- `VERBOSE=0 cargo run --bin mini-graph`

Run the harness in another terminal:

- `cargo run --bin loadtest -- --subscribers 100 --producer-rate 1000 --duration-secs 10`

Sample output:

```
=== Load test report ===
settings duration_secs=10 drain_ms=500 subscribers=100 producer_rate_msgs_per_sec=1000 document_id=doc ws_url=ws://127.0.0.1:3030/ws http_base_url=http://127.0.0.1:3030

-- Throughput --
producer_sent_expected_total=10000
producer_sent_total=10000
subscriber_received_total=1000000
subscriber_seq_gaps_total=0
server_actor_cmd_drops_total=0
server_fanout_drops_total=0

-- Latency (ms) --
latency_ms_p50=0.500
latency_ms_p95=1.500
latency_ms_p99=2.500
latency_ms_max=10.000

-- Server resources --
server_samples_total=10
server_cpu_pct_last=50.00
server_cpu_pct_avg=45.00
server_cpu_pct_max=60.00
server_memory_mib_last=120.0
server_memory_mib_avg=118.0
server_memory_mib_max=121.0
```
