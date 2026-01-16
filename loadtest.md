# WebSocket Load Test Harness

This repo includes a simple, two-process WebSocket load test setup:

- **Server**: `mini-graph` (Axum + Tokio WebSocket server)
- **Harness**: `loadtest` (spawns one producer + N subscribers)

The harness is designed to approximate the future architecture: **one producer stream publishing updates that the server fans out to many subscribers**.

## What We Implemented

### 1) Producer → fanout model

- `loadtest` opens:
  - **1 producer** connection
  - **N subscribers** (default `100`) that join the same `document_id` (room)
- The producer sends at **a total rate** of `--producer-rate` messages/sec (not per subscriber).
- Each produced message is broadcast by the server to all subscribers in the room.

### 2) Protocol support (`send_message_to`)

The server protocol now supports an additive client message:

- `ClientMessage::SendMessageTo { document_id, text }`

This allows a single producer connection to target any room/document per message (useful once the producer stream contains updates for many documents). The existing `send_message` behavior still works.

See: `src/types.rs`.

### 3) Dropped message tracking

Drops are counted in two distinct places:

- **Actor command drops** (`server_actor_cmd_drops_total`)
  - Happens when the bounded channel into the rooms actor is full.
  - Source: `RoomsHandle::try_send(...)` in `src/state.rs`.

- **Fanout drops** (`server_fanout_drops_total`)
  - Happens when broadcasting to a subscriber’s bounded per-connection channel is full.
  - Source: `tx.try_send(...)` per client during broadcast in `src/rooms.rs`.

Fanout drops are counted per delivery (i.e. one produced message can contribute multiple drops if many subscribers are slow).

### 4) Server metrics endpoint

The server exposes:

- `GET /debug/metrics`

It returns JSON containing drop counters and sampled resource usage.

See: `src/lib.rs` and `src/metrics.rs`.

### 5) Latency and loss measurement (client-observed)

The harness embeds a JSON payload inside `text`:

- `{ "seq": <u64>, "sent_at_us": <u64> }`

Subscribers compute one-way latency (`now - sent_at`) using microsecond timestamps, record it in a histogram, and report percentiles in **milliseconds**.

Loss is detected via sequence gaps per subscriber and summed across subscribers.

### 6) Report formatting

The load test report is printed in blocks:

- Settings
- Throughput (sent/received/gaps/drops)
- Latency
- Server resources

### 7) Logging behavior

High-volume server logs are gated behind an env var:

- `MINI_GRAPH_VERBOSE=1` enables verbose logs.
- Error logs remain on by default.

This avoids `println!` overhead dominating load tests.

## Running

### Start the server

```bash
cargo run --bin mini-graph
```

(Optional verbose logs)

```bash
MINI_GRAPH_VERBOSE=1 cargo run --bin mini-graph
```

### Run the harness (in another terminal)

```bash
cargo run --bin loadtest -- \
  --subscribers 100 \
  --producer-rate 1000 \
  --duration-secs 10
```

Useful knobs:

- `--subscribers`: fanout width
- `--producer-rate`: total messages/sec produced
- `--duration-secs`: steady-state load phase duration
- `--drain-ms`: grace period for subscribers to drain buffered/in-flight frames

## Important Semantics / Invariants

- **Duration starts after subscribers are ready**: `--duration-secs` measures the steady-state load window, beginning once all subscribers have connected and joined.
- **Drain period**: subscribers continue reading until `start + duration + drain` so end-of-test totals align better.
- **Resource stats sampling**:
  - Server samples CPU/memory once per second.
  - `/debug/metrics` reports **last/avg/max** based on the samples collected so far.
  - This is currently over the server’s lifetime; it is not reset per test.

## Interpreting CPU%

The reported CPU percentage comes from `sysinfo`’s per-process CPU usage. On multi-core machines it can exceed 100% (e.g. ~200% means roughly 2 cores fully utilized).

## Not Implemented Yet (Documented Approach)

### Slow subscribers

To intentionally trigger `fanout_drops_total`, a future harness option can simulate slow subscribers by delaying reads (e.g. sleep after each received WS frame). That increases backpressure and can fill the per-client bounded channel on the server, causing `TrySendError::Full` drops.

## Next Extensions (if desired)

- Per-test resource avg/max (reset endpoint or periodic polling)
- Slow-subscriber controls (`--slow-subscriber-pct`, `--subscriber-read-delay-ms`)
- Ramp-up / burst traffic profiles
- Multi-room distribution (once needed)
