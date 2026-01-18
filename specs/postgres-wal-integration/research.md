---
spec: postgres-wal-integration
phase: research
created: 2026-01-16T00:00:00Z
---

# Research: postgres-wal-integration

## Executive Summary

PostgreSQL Write-Ahead Log (WAL) streaming via logical replication provides a proven mechanism for capturing database changes in real-time. This research evaluates integrating WAL streaming into mini-graph's existing actor-based architecture using the pgoutput protocol. The integration is technically feasible and aligns well with the bounded channel pattern already established in the codebase. Recommended approach: dedicated WAL reader task consuming pgoutput events, emitting invalidations through a bounded channel to either the existing rooms actor or a new events actor for fan-out to subscribed clients.

**Key Findings:**
- pgoutput is the standard, built-in logical replication format (Postgres 10+)
- Rust ecosystem offers specialized crates: pgwire-replication (low-level wire protocol) and pg_replicate/Supabase ETL (production framework)
- Bounded channels with backpressure are essential to prevent WAL consumer lag
- Replication slot management is critical: unused slots cause WAL bloat and storage exhaustion
- Testing strategies must account for Postgres setup, long-running transactions, and failover scenarios

## External Research

### Best Practices

**Use pgoutput Plugin (Standard Output Format)**

pgoutput is the recommended logical decoding plugin with three key advantages: available out-of-the-box with PostgreSQL 10+, uses efficient binary Postgres replication message format instead of JSON, and provides fine-grained control over replicated tables, columns, and rows through publications. The protocol supports versions 1-4, with Version 2 adding streaming of large in-progress transactions (PG 14+), Version 3 adding two-phase commit support (PG 15+), and Version 4 enabling parallel application of large transactions (PG 16+).

*Source: [PostgreSQL Documentation: Logical Streaming Replication Protocol](https://www.postgresql.org/docs/current/protocol-logical-replication.html), [Npgsql Logical Replication Documentation](https://www.npgsql.org/doc/replication.html)*

**Monitor and Manage Replication Slots Actively**

Logical decoding must be monitored rigorously. Any unused replication slot must be dropped immediately, as slots hold on to PostgreSQL WAL logs and system catalogs until changes have been consumed. Unconsumed logs pile up and fill storage, increase transaction ID wraparound risk, and can cause the server to become unavailable. Set `max_slot_wal_keep_size` (Postgres 13+) to prevent unlimited WAL retention—this invalidates slots exceeding the threshold rather than allowing disk exhaustion.

Track these metrics continuously:
- Total and retained WAL size per slot
- Slot status (active/inactive/invalid)
- Safe remaining WAL capacity
- Replication lag (`MilliSecondsBehindSource`)
- Disk utilization and spill statistics

*Source: [Mastering Postgres Replication Slots - Gunnar Morling](https://www.morling.dev/blog/mastering-postgres-replication-slots/)*

**Progress Reporting and LSN Management**

Always report replication progress by updating LastAppliedLsn and LastFlushedLsn so that Postgres can remove/recycle WAL files. For CDC pipelines, progress should typically be reported at transaction commit boundaries, not for every message. This prevents WAL bloat while ensuring transactional consistency. LSN updates are monotonic—older values become no-ops.

*Source: [Npgsql Logical Replication Documentation](https://www.npgsql.org/doc/replication.html), [pgwire-replication GitHub](https://github.com/vnvo/pgwire-replication)*

**Enable Heartbeats for Low-Traffic Databases**

For databases with sporadic changes, heartbeat messages prevent slot stagnation. PostgreSQL 14+ supports `pg_logical_emit_message()` for table-less heartbeats without requiring dedicated infrastructure. This ensures slots advance even when no table data changes occur.

*Source: [Mastering Postgres Replication Slots - Gunnar Morling](https://www.morling.dev/blog/mastering-postgres-replication-slots/)*

**Optimize Publications for Minimal Overhead**

Create table-level publications instead of capturing all tables to save resources (CPU, network I/O) on the database side and drastically reduce egress cost when streaming to another availability zone. PostgreSQL 15+ supports column-level filtering to exclude unnecessary large columns and row-level WHERE clauses to filter out test or logically deleted data.

*Source: [Logical Replication Postgres Basics - EnterpriseDB](https://www.enterprisedb.com/blog/logical-replication-postgres-basics), [Mastering Postgres Replication Slots - Gunnar Morling](https://www.morling.dev/blog/mastering-postgres-replication-slots/)*

**Bounded Channels for Backpressure Control**

Bounded channels act as queues with capacity limits, controlling how much data is allowed in the system at any given time. When a bounded channel reaches capacity, the send operation is rejected or blocks, ensuring applications handle overload scenarios gracefully. If the bounded channel is full, `Sender::send().await` will asynchronously block until the consumer has processed more items. This prevents memory exhaustion when WAL events arrive faster than consumers can process them.

*Source: [Handling Backpressure in Rust Async Systems - Sling Academy](https://www.slingacademy.com/article/handling-backpressure-in-rust-async-systems-with-bounded-channels/), [Tokio mpsc Documentation](https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html)*

### Prior Art

**pgwire-replication: Low-Level Wire Protocol Client**

pgwire-replication is a lean tokio-based Postgres wire-protocol logical replication client that supports pgoutput with TLS (rustls) and SCRAM authentication. It intentionally avoids libpq, tokio-postgres, and other higher-level PostgreSQL clients for the replication path, communicating directly with PostgreSQL using `START_REPLICATION ... LOGICAL` commands.

API Design:
- **ReplicationClient**: Async tokio-based interface with `connect()`, `recv()`, and `update_applied_lsn()` methods
- **ReplicationConfig**: Configuration struct holding connection details, LSN parameters (`start_lsn`, optional `stop_at_lsn`), and timing settings
- **ReplicationEvent**: Enum variants including `XLogData`, `KeepAlive`, and `StoppedAt`

The design emphasizes explicit LSN handling—users provide `start_lsn` to resume from specific positions and optionally `stop_at_lsn` for bounded replay. Configurable buffer sizes (`buffer_events: 8192`) support deterministic batching. Non-goals explicitly exclude general SQL client functionality, automatic checkpoint persistence, exactly-once guarantees, DDL interpretation, and full pgoutput decoding—these remain higher-layer responsibilities.

Requirements: Rust 1.88+, PostgreSQL 15+ with logical replication enabled.

*Source: [pgwire-replication GitHub](https://github.com/vnvo/pgwire-replication)*

**Supabase ETL (pg_replicate): Production Framework**

Supabase ETL is a Rust framework for building real-time Postgres data replication applications with production-ready features including configurable batching and parallelism to maximize throughput, fault-tolerant error handling with retry logic, and official support for PostgreSQL 14-18.

Architecture:
- **Pipeline**: Orchestrates the replication workflow
- **Store**: Maintains state and schema information (memory or custom implementations)
- **Destination**: Outputs replicated data (BigQuery, Apache Iceberg, or custom)
- **PostgreSQL Connection**: Manages logical replication protocol

The framework provides a type-safe, Rust-native interface with compile-time guarantees and extensible trait-based destination system. Error handling includes configurable retry parameters per pipeline with exponential backoff. Active development with 2.2k GitHub stars; continuous integration testing, code coverage monitoring, and security audits.

*Source: [Supabase ETL GitHub](https://github.com/supabase/pg_replicate), [Supabase ETL Building Real-Time PostgreSQL Replication Pipelines](https://joshuaberkowitz.us/blog/github-repos-8/supabase-etl-building-real-time-postgresql-replication-pipelines-in-rust-1767)*

**Other Rust Implementations**

- **pglogrepl-rust**: Proof-of-concept for Postgres streaming logical replication using rust-postgres (Tableland)
- **pgwire**: Library implementing PostgreSQL wire protocol with support for logical streaming replication protocol messages
- **postgres-protocol**: Official low-level Postgres protocol APIs crate

*Source: [pglogrepl-rust GitHub](https://github.com/tablelandnetwork/pglogrepl-rust), [pgwire on Lib.rs](https://lib.rs/crates/pgwire), [postgres-protocol on crates.io](https://crates.io/crates/postgres-protocol)*

### Pitfalls to Avoid

**Forgetting to Drop Unused Slots**

Manually delete inactive slots to prevent WAL blocking. Set up automated monitoring with alerts for slots inactive beyond 30 minutes and establish baseline thresholds (disk usage exceeding 60-70%, WAL retention surpassing 10-20 GB, rapid disk utilization increases).

*Source: [Mastering Postgres Replication Slots - Gunnar Morling](https://www.morling.dev/blog/mastering-postgres-replication-slots/)*

**Mismatched Publication Filters**

Ensure any column lists or row filters in your consumer configuration match the publication definitions in Postgres. Mismatched filters can cause silent data loss or replication errors.

*Source: [Mastering Postgres Replication Slots - Gunnar Morling](https://www.morling.dev/blog/mastering-postgres-replication-slots/)*

**Ignoring Long-Running Transactions**

Monitor `pg_stat_replication_slots` for disk spill events. Long-running transactions can prevent WAL from being consumed, causing slots to accumulate unbounded WAL data.

*Source: [Mastering Postgres Replication Slots - Gunnar Morling](https://www.morling.dev/blog/mastering-postgres-replication-slots/)*

**Missing Heartbeats in Idle Databases**

Without heartbeats, slots on idle databases (e.g., multi-tenant systems with inactive tenants) won't advance LSN positions, preventing WAL cleanup. Use `pg_logical_emit_message()` (PG 14+) for periodic heartbeats.

*Source: [Mastering Postgres Replication Slots - Gunnar Morling](https://www.morling.dev/blog/mastering-postgres-replication-slots/)*

**Unbounded Channels Leading to Memory Exhaustion**

Using unbounded channels between WAL consumer and downstream processors can cause memory leaks if processors can't keep up with WAL volume. Always use bounded channels with reasonable limits and monitor buffer depth.

*Source: [Handling Backpressure in Rust Async Systems - Sling Academy](https://www.slingacademy.com/article/handling-backpressure-in-rust-async-systems-with-bounded-channels/)*

## Codebase Analysis

### Existing Patterns

**Actor Model with Bounded Channels**

The codebase already implements a robust actor model pattern for the rooms system (see `src/rooms.rs`):

```rust
pub struct RoomsHandle {
    tx: mpsc::Sender<RoomCommand>,
}

impl RoomsHandle {
    pub(crate) fn start(metrics: Arc<Metrics>) -> Self {
        let (tx, rx) = mpsc::channel(ROOMS_ACTOR_CHANNEL_CAPACITY);
        tokio::spawn(rooms_actor(rx, metrics));
        Self { tx }
    }
}
```

The rooms actor runs as a spawned Tokio task, receiving `RoomCommand` enum messages via a bounded channel (capacity 256). Commands include `Join`, `Leave`, and `Broadcast`. The actor maintains exclusive ownership of the `rooms` HashMap, eliminating lock contention. This pattern is directly applicable to WAL integration.

*Location: `/Users/nickmaietta/projects/mini-graph/src/rooms.rs` lines 12-21, 26-109*

**Bounded Channel Capacity Configuration**

The system uses `mpsc::channel(256)` for both the rooms actor command queue and per-client outbound queues. This establishes a precedent for backpressure handling:
- Full command channel: `inc_actor_cmd_drop()` metric, TrySendError::Full drops newest message
- Full per-client queue during broadcast: `inc_fanout_drop()` metric, drop message for slow client
- Closed per-client queue: Remove client from room

*Location: `/Users/nickmaietta/projects/mini-graph/src/rooms.rs` lines 70-106, `/Users/nickmaietta/projects/mini-graph/src/websocket.rs` lines 143*

**Metrics and Observability**

The codebase includes a metrics system using `AtomicU64` counters for lock-free tracking:
- `actor_cmd_drops_total`: Commands dropped due to channel full
- `fanout_drops_total`: Broadcast messages dropped for slow clients
- CPU and memory sampling every 1 second via `run_resource_sampler`

WAL integration should extend this pattern with WAL-specific metrics (events consumed, lag, slot health).

*Location: `/Users/nickmaietta/projects/mini-graph/src/metrics.rs` lines 1-127*

**Message Type Enum Pattern**

The WebSocket layer uses tagged enum types for type-safe message handling:

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Join { document_id: DocumentId },
    SendMessage { text: String },
    SendMessageTo { document_id: DocumentId, text: String },
}
```

A similar pattern is appropriate for WAL events: `WalEvent::Insert`, `WalEvent::Update`, `WalEvent::Delete` with table-specific data.

*Location: `/Users/nickmaietta/projects/mini-graph/src/types.rs` lines 46-76*

**Session and State Management**

The WebSocket handler uses a `Session` struct wrapping client state with a `tokio::sync::Mutex<Option<DocumentId>>` for current room tracking. For WAL integration, a similar session-like abstraction could track subscription state: which queries a client has subscribed to and their associated invalidation hints.

*Location: `/Users/nickmaietta/projects/mini-graph/src/websocket.rs` lines 17-31*

### Dependencies

**Current Dependencies (Cargo.toml):**

```toml
tokio = { version = "1", features = ["full"] }
axum = { version = "0.7", features = ["ws"] }
tower-http = { version = "0.6", features = ["fs"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.0", features = ["v4"] }
futures-util = "0.3.31"
sysinfo = "0.30"
```

**Additional Dependencies Needed for WAL Integration:**

Option 1 (Low-level control):
- `pgwire-replication = "0.x"` - Direct wire protocol access, explicit LSN management
- OR custom implementation using `postgres-protocol = "0.x"` and `tokio-postgres = "0.x"`

Option 2 (Production framework):
- `pg_replicate = "0.x"` (Supabase ETL) - Higher-level abstractions, built-in retry/batching

Option 3 (Hybrid):
- `tokio-postgres = "0.x"` - For control-plane queries (creating publications, managing slots)
- `pgwire-replication = "0.x"` - For data-plane replication stream

For a learning-focused project like mini-graph, **Option 1 with pgwire-replication** is recommended: it provides explicit control over LSN tracking and integrates cleanly with the existing bounded channel architecture while avoiding heavyweight framework abstractions.

*Location: `/Users/nickmaietta/projects/mini-graph/Cargo.toml`*

### Constraints

**Single-Process Architecture**

All state is in-process memory. WAL consumer state (last consumed LSN) must be tracked in memory and will be lost on restart. This is acceptable for the demo but means replication will restart from a new LSN on each process restart, potentially missing events.

For production, LSN checkpoints should be persisted (e.g., SQLite file or separate Postgres table).

*Source: architecture.md lines 446-462*

**No Database Persistence Yet**

Current phase (WebSocket setup) explicitly excludes database integration. WAL integration will be the first component requiring a Postgres connection, establishing patterns for future Cache module database interactions.

*Source: websocket-setup/requirements.md lines 263-266*

**Actor Model Commitment**

The architecture document already recommends: "WAL reader task emits events into a bounded channel feeding the rooms actor (or a dedicated 'events' actor), which then fans out to rooms." This establishes the integration shape: WAL consumer should fit the existing actor pattern rather than introducing new concurrency models.

*Source: `/Users/nickmaietta/projects/mini-graph/docs/architecture.md` lines 56-58*

**Bounded Channels are Mandatory**

Based on the existing rooms implementation and WebSocket research findings, unbounded channels are considered an anti-pattern. WAL integration must use bounded channels with explicit backpressure handling (drop events, apply timeouts, or close connections).

*Source: websocket-setup/research.md lines 619-663*

## Related Specs

| Spec Name | Relevance | Relationship | May Need Update |
|-----------|-----------|--------------|-----------------|
| websocket-setup | High | WebSocket infrastructure provides the Edge module that will consume WAL-triggered invalidations and deliver updates to clients. WAL integration depends on rooms actor patterns established here. | Yes - After WAL integration, WebSocket layer needs to handle subscription management (query subscriptions) and invalidation-triggered re-fetch logic instead of simple echo broadcasting. |

**Analysis:**

The websocket-setup spec is the only existing spec and has **high relevance** because:
1. It established the actor model pattern (rooms actor with bounded channels) that WAL integration should follow
2. The WebSocket layer serves as the "Edge" module in DESIGN.md, which is the consumer of WAL-triggered invalidations
3. Current implementation is a simple echo server, but WAL integration will require extending it with:
   - Query subscription tracking (inverted index)
   - Invalidation event handling
   - Cache query result delivery instead of text broadcasting

**May Need Update: Yes**

Once WAL integration is complete, the WebSocket layer will need modifications:
- Add subscription state management (which clients subscribe to which queries)
- Implement inverted index mapping invalidation hints to subscriptions
- Change broadcast model from "echo text" to "send query results on invalidation"
- Add cache module interaction (requesting query results)

## Feasibility Assessment

| Aspect | Assessment | Notes |
|--------|------------|-------|
| Technical Viability | High | pgoutput protocol is stable (Postgres 10+), mature Rust crates available (pgwire-replication, pg_replicate), wire protocol well-documented. Integration pattern aligns with existing actor model. |
| Effort Estimate | L (Large) | Components required: (1) WAL consumer task with pgoutput decoding, (2) LSN tracking and progress reporting, (3) Replication slot lifecycle management (create/drop/monitor), (4) Invalidation message generation from WAL events, (5) Bounded channel integration with rooms/events actor, (6) Postgres connection pooling for control-plane, (7) Metrics for lag/health, (8) Error handling and reconnection logic. Estimate: 2-3 weeks for experienced Rust developer. |
| Risk Level | Medium | Key risks: (1) Replication slot management complexity—WAL bloat if slots aren't monitored/dropped, (2) Testing requires Postgres setup with logical replication enabled, (3) Mapping WAL events to invalidation hints requires schema knowledge, (4) Long-running transactions can cause lag, (5) Failover/restart scenarios need careful LSN checkpoint handling. Mitigations available through bounded channels, monitoring metrics, and existing best practices. |
| Learning Curve | Medium-High | Requires understanding: Postgres logical replication concepts (slots, publications, LSN), pgoutput binary protocol (relation messages, tuple data), WAL consumption patterns (commit boundaries, progress reporting), bounded channel backpressure strategies. However, pgwire-replication crate abstracts much of the wire protocol complexity. |
| Integration Complexity | Medium | Fits well into existing architecture—follows established actor pattern, uses same bounded channel approach, extends existing metrics system. Main challenge is coordinating between WAL consumer (data plane) and control-plane operations (managing slots, publications). |
| Production Readiness | Low (Phase 1) | Acceptable for demo/learning: in-memory LSN tracking, manual slot management, basic error handling. Production gaps: no LSN persistence, no automatic slot cleanup, no failover support, no distributed deployment. These can be layered on later. |

## Recommendations for Requirements

### 1. Use pgwire-replication for WAL Consumption

**Rationale:** Provides direct wire protocol access with explicit LSN control, avoiding heavyweight framework abstractions while offering TLS and SCRAM support. Clean async Tokio integration matches existing architecture. Bounded replay via `stop_at_lsn` supports deterministic testing.

**Alternative Considered:** Supabase ETL (pg_replicate) offers production features but adds complexity unnecessary for learning-focused mini-graph.

### 2. Implement Dedicated WAL Reader Actor

**Rationale:** Follow the established pattern from rooms actor: dedicated Tokio task consuming from WAL stream, emitting events via bounded channel. This provides isolation (WAL consumer doesn't block other tasks), backpressure control (bounded channel prevents memory exhaustion), and testability (can inject mock WAL events via channel).

**Recommended Structure:**
```rust
pub struct WalHandle {
    tx: mpsc::Sender<WalCommand>,
}

enum WalCommand {
    Start { start_lsn: Lsn },
    Stop,
}

enum WalEvent {
    Insert { table: String, data: HashMap<String, Value> },
    Update { table: String, old: HashMap<String, Value>, new: HashMap<String, Value> },
    Delete { table: String, data: HashMap<String, Value> },
    Commit { lsn: Lsn },
}
```

### 3. Configure Bounded Event Channel with Appropriate Capacity

**Rationale:** Consistent with existing 256-message buffer for rooms actor. WAL events may arrive in bursts during transaction commits, so buffer should accommodate typical transaction sizes without frequent drops.

**Recommended:** Start with 256 or 512 events. Monitor `wal_event_drops_total` metric. Adjust based on workload characteristics.

### 4. Track LSN Progress In-Memory (Accept Restart Limitations)

**Rationale:** For Phase 1 demo, accept that replication position is lost on restart. Store last applied LSN in an `AtomicU64` or within the WAL actor's state. Report progress to Postgres at transaction commit boundaries via `update_applied_lsn()`.

**Future:** Persist LSN checkpoints to SQLite or a dedicated Postgres table for production deployments.

### 5. Implement Replication Slot Lifecycle Management

**Rationale:** Slots are persistent server-side resources. Create slot on WAL consumer startup, track slot name, expose control-plane operations (drop slot on shutdown). Emit metrics for slot lag and WAL size.

**Recommended:**
- Slot naming convention: `mini_graph_slot_{instance_id}`
- Use tokio-postgres for control-plane queries (`CREATE_REPLICATION_SLOT`, `DROP_REPLICATION_SLOT`)
- Separate control-plane connection from data-plane replication stream

### 6. Create Publications Matching Demo Schema

**Rationale:** Publications define which tables are replicated. For the demo schema (users, files, comments), create a publication including only those tables. Use column filtering (PG 15+) to exclude large columns if present.

**Example:**
```sql
CREATE PUBLICATION mini_graph_pub FOR TABLE users, files, comments;
```

### 7. Generate Invalidation Hints from WAL Events

**Rationale:** WAL events contain row data. Extract column values to generate query hints matching the inverted index pattern from DESIGN.md (`table:column:value`).

**Example:** For an INSERT into `comments` with `{id: 456, file_id: 123, user_id: 789, text: "Hello"}`, generate hints:
- `comments:id:456`
- `comments:file_id:123`
- `comments:user_id:789`

These hints are broadcast to the Edge module (WebSocket layer) for subscription matching.

### 8. Extend Metrics System for WAL Health

**Rationale:** Observability is critical for WAL consumers. Expose metrics for monitoring and alerting.

**Recommended Metrics:**
- `wal_events_consumed_total` (counter)
- `wal_events_dropped_total` (counter, due to full channel)
- `wal_lag_seconds` (gauge, based on commit timestamp vs. current time)
- `wal_slot_active` (gauge, 0/1 boolean)
- `wal_retained_bytes` (gauge, from `pg_replication_slots`)

### 9. Implement Graceful Shutdown and Reconnection

**Rationale:** WAL consumer should handle Postgres connection loss gracefully, reconnecting with exponential backoff (similar to WebSocket client pattern).

**Recommended:**
- On connection loss: log error, report metric, retry with exponential backoff (1s, 2s, 4s, max 30s)
- On shutdown signal: flush pending LSN update, cleanly close replication connection
- For slot cleanup: provide admin command to drop slot manually (don't auto-drop on restart to preserve position)

### 10. Design for Testability with Mock WAL Events

**Rationale:** Full integration tests requiring Postgres setup are heavy. Provide injection points for mock WAL events to test invalidation logic independently.

**Recommended:**
- WAL consumer accepts events from either real replication stream or test channel
- Unit tests inject `WalEvent` enums directly into processing pipeline
- Integration tests use Postgres with test schema and scripted INSERT/UPDATE/DELETE operations

### Testing Plan (Phase 1)

**Decision:** Prefer fast unit tests + manual end-to-end verification (no automated integration tests in Phase 1).

- Unit tests:
  - Verify `comments` INSERT/UPDATE/DELETE map to the correct invalidation hint shape: `comments:document_id:<document_id>`
  - Include DELETE coverage explicitly (requires `REPLICA IDENTITY FULL` so `document_id` is available on deletes)
  - Verify the invalidation is routed to the correct document room (fanout path via rooms actor)
- Manual verification:
  - Use Docker Postgres for a reproducible local environment
  - Use a reset script (planned) + churn script (planned) to make repeating scenarios easy

## Open Questions

### Phase 1 Scope Decisions (2026-01-17)

Desired behavior for Phase 1 is document-scoped invalidation hints based on two tables.

- Tables: `documents`, `comments`
- Client subscription key: `document_id`
- Events to replicate:
  - `comments`: INSERT / UPDATE / DELETE
  - `documents`: DELETE only (no document INSERT/UPDATE streaming yet)
- Routing rule:
  - Any change to a comment must invalidate subscribers of `comments.document_id`
  - Document deletes invalidate subscribers of that `document_id`
  - Edge case: if a comment UPDATE changes `document_id`, invalidate both the old and new `document_id`
  - Within one transaction, it is acceptable (and preferred) to de-duplicate invalidations so each affected `document_id` is invalidated at most once per commit
- Hint shape:
  - Use the inverted-index direction from DESIGN.md (e.g., `comments:document_id:<document_id>`)
- Actor topology:
  - WAL reader emits invalidation hints directly to the rooms actor (no separate events actor in Phase 1)
- Postgres target:
  - PostgreSQL 18 (or latest available) for Phase 1 development
- Postgres replication requirement:
  - `ALTER TABLE comments REPLICA IDENTITY FULL` so DELETE events include `document_id`
- Postgres setup automation:
  - By default, the server should run the required Postgres DDL/commands on startup (local/dev ergonomics)
  - Also provide a shell-wrapper reset script (planned) that can drop/recreate the publication + slot (and optionally truncate demo tables) to return local dev to a clean slate
  - Also provide a shell-wrapper churn script (planned) that generates comment INSERT/UPDATE/DELETE traffic for manual testing
- Client behavior on receipt:
  - Phase 1 milestone: see WAL-driven invalidations arrive in the application and get broadcast to browser clients in the correct document room
  - Out of scope: cache/refetch loop, query subscriptions, and any attempt to de-throttle client refresh (thundering herd mitigation)
  - Note: later work should avoid a thundering herd when many clients re-fetch on the same invalidation

### Q1: Should WAL events go to rooms actor or a dedicated events actor?

**Options:**
- **Option A:** Extend rooms actor to handle both `RoomCommand` and `WalEvent` on same channel
- **Option B:** Create separate `EventsActor` that receives `WalEvent`, generates invalidations, and forwards to rooms actor
- **Option C:** WAL consumer directly emits invalidations to a broadcast channel, Edge module subscribes

**Recommendation Needed:** Does the rooms actor need to be aware of WAL events, or should invalidation generation happen upstream?

**Preliminary Analysis:** Option B (separate EventsActor) provides better separation of concerns. Rooms actor focuses on client lifecycle (join/leave/broadcast), while EventsActor focuses on data plane (WAL events → invalidations). However, this adds another actor and channel hop.

**Decision (Phase 1):** Keep rooms actor focused on fan-out. The WAL reader should generate document-scoped invalidation hints and deliver them via existing room broadcast commands (no `WalEvent` handling in rooms actor, and no separate EventsActor).

### Q2: How should we handle schema changes (DDL) during replication?

**Context:** Logical replication streams DDL changes (ALTER TABLE, etc.) as relation messages. The system must update its understanding of table schemas to correctly decode future WAL events.

**Options:**
- Ignore schema changes (restart required on DDL)
- Parse relation messages and update in-memory schema cache
- Fail fast on DDL, alert operator to restart consumer

**Preliminary Analysis:** For Phase 1 demo with fixed schema, ignoring schema changes is acceptable. Production system needs schema versioning.

**Decision (Phase 1):** Fail fast if relation/schema metadata changes mid-stream. Treat as an operator action (restart consumer after schema is stable) rather than attempting hot-reload.

### Q3: Should we support filtering WAL events by publication at the application level?

**Context:** Publications define which tables are replicated at the Postgres level. Should the application add additional filtering (e.g., only process events for tables with registered queries)?

**Options:**
- Process all events from publication, generate hints for all tables
- Filter events by tables referenced in query definitions
- Make filtering configurable

**Preliminary Analysis:** Start by processing all events from publication for simplicity. Add application-level filtering if performance profiling shows unnecessary work.

**Decision (Phase 1):** Keep filtering in Postgres via a minimal publication (only `documents` and `comments`). The application processes all events from that publication and applies routing rules (e.g., ignore `documents` INSERT/UPDATE since Phase 1 only cares about document deletes).

### Q4: How should we handle long-running transactions that haven't committed?

**Context:** Postgres 14+ supports streaming of in-progress transactions (pgoutput protocol v2+). Should we process uncommitted changes or wait for commit?

**Options:**
- Wait for COMMIT before processing (transactional consistency)
- Process streaming changes incrementally (lower latency, but may rollback)

**Recommendation Needed:** Depends on consistency requirements for mini-graph invalidations.

**Preliminary Analysis:** For Phase 1, wait for COMMIT to ensure transactional consistency. Streaming in-progress transactions is an optimization for large transactions (>1GB).

**Decision (Phase 1):** Wait for COMMIT before emitting invalidation hints. Treat invalidations as commit-bounded (no notifications for rolled-back changes).

### Q5: What should be the relationship between LSN checkpointing and invalidation delivery guarantees?

**Context:** If we report LSN progress to Postgres before invalidations are delivered to all clients, a crash could lose invalidations (Postgres discards WAL, clients never notified).

**Options:**
- Report LSN only after all clients acknowledged invalidation (strong guarantee, complex)
- Report LSN after broadcasting to channels (at-least-once delivery, simpler)
- Report LSN immediately after consuming (best for Postgres, weakest guarantee)

**Preliminary Analysis:** For demo, report LSN after broadcasting to channels. Accept that crash during fanout loses events for some clients. Document as "at-least-once delivery with best-effort fanout."

**Decision (Phase 1):** Advance/acknowledge LSN after the invalidation is successfully enqueued to the rooms actor (not after per-client delivery). This keeps WAL retention under control but means a crash can cause missed invalidations.

## Sources

### PostgreSQL Documentation
- [PostgreSQL: Logical Streaming Replication Protocol](https://www.postgresql.org/docs/current/protocol-logical-replication.html)
- [PostgreSQL: Logical Replication](https://www.postgresql.org/docs/current/logical-replication.html)
- [PostgreSQL: Logical Replication Architecture](https://www.postgresql.org/docs/current/logical-replication-architecture.html)
- [PostgreSQL Documentation (v16): Logical Streaming Replication Protocol](https://www.postgresql.org/docs/16/protocol-logical-replication.html)
- [PostgreSQL Documentation (v15): Logical Streaming Replication Protocol](https://www.postgresql.org/docs/15/protocol-logical-replication.html)

### Rust Crates and Libraries
- [pgwire-replication GitHub](https://github.com/vnvo/pgwire-replication)
- [Supabase ETL GitHub (pg_replicate)](https://github.com/supabase/pg_replicate)
- [tokio-postgres - crates.io](https://crates.io/crates/tokio-postgres)
- [postgres-protocol - crates.io](https://crates.io/crates/postgres-protocol)
- [pgwire - Lib.rs](https://lib.rs/crates/pgwire)
- [pglogrepl-rust GitHub](https://github.com/tablelandnetwork/pglogrepl-rust)

### Best Practices and Guides
- [Npgsql: Logical and Physical Replication Documentation](https://www.npgsql.org/doc/replication.html)
- [Mastering Postgres Replication Slots - Gunnar Morling](https://www.morling.dev/blog/mastering-postgres-replication-slots/)
- [Logical Replication in Postgres: Understand the Basics - EnterpriseDB](https://www.enterprisedb.com/blog/logical-replication-postgres-basics)
- [Logical replication and logical decoding - Microsoft Learn](https://learn.microsoft.com/en-us/azure/postgresql/configure-maintain/concepts-logical)
- [Getting Postgres logical replication changes using pgoutput plugin - Medium](https://medium.com/@film42/getting-postgres-logical-replication-changes-using-pgoutput-plugin-b752e57bfd58)

### Rust Async and Backpressure Patterns
- [Handling Backpressure in Rust Async Systems with Bounded Channels - Sling Academy](https://www.slingacademy.com/article/handling-backpressure-in-rust-async-systems-with-bounded-channels/)
- [Tokio sync::mpsc Documentation](https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html)
- [Rust Concurrency: Streaming Workflow with Backpressure - Medium](https://medium.com/@polyglot_factotum/rust-concurrency-a-streaming-workflow-served-with-a-side-of-back-pressure-955bdf0266b5)
- [Rust: Stateful Streaming Actors with Channels and Backpressure - Medium](https://medium.com/@wedevare/rust-stateful-streaming-actors-with-channels-and-backpressure-940b99acc544)

### Community Discussions
- [Show HN: Pg_replicate – Build Postgres replication applications in Rust - Hacker News](https://news.ycombinator.com/item?id=41209994)
- [PostgreSQL Logical Replication Explained - Hacker News](https://news.ycombinator.com/item?id=35203571)
- [Streaming replication protocol - rust-postgres GitHub Issue](https://github.com/sfackler/rust-postgres/issues/116)

## Quality Commands

Based on analysis of the project structure:

| Type | Command | Source |
|------|---------|--------|
| Lint | `cargo clippy` | CLAUDE.md project instructions |
| TypeCheck | Not applicable (Rust) | Rust compiler performs type checking during `cargo build` |
| Unit Test | `cargo test` | CLAUDE.md project instructions |
| Integration Test | Not found | No dedicated integration test command; use `cargo test` |
| E2E Test | Not found | Manual testing documented in docs/ |
| Test (all) | `cargo test` | CLAUDE.md project instructions |
| Build | `cargo build` | CLAUDE.md project instructions |
| Format Check | `cargo fmt -- --check` | CLAUDE.md project instructions |
| Format | `cargo fmt` | CLAUDE.md project instructions |

**Local CI Equivalent**:
```bash
cargo fmt -- --check && cargo clippy && cargo test && cargo build
```

**Note:** The project uses Cargo as the build system. There is no package.json, Makefile, or GitHub Actions workflows configured yet. Quality checks rely on standard Cargo commands.
