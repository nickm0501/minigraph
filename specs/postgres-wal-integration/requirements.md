---
spec: postgres-wal-integration
phase: requirements
created: 2026-01-17T00:00:00Z
---

# Requirements: PostgreSQL WAL Integration for Real-time Document Invalidation

## Goal

Enable real-time change data capture from PostgreSQL using Write-Ahead Log (WAL) streaming to automatically invalidate document subscriptions when comments or documents change. This establishes the Invalidator module foundation, allowing the Edge module to deliver timely updates to WebSocket clients subscribed to specific documents.

## User Stories

### US-1: Stream WAL Changes for Comments Table
**As a** system operator
**I want to** capture INSERT, UPDATE, and DELETE operations on the `comments` table via PostgreSQL logical replication
**So that** clients subscribed to affected documents receive invalidation notifications immediately after comment changes

**Acceptance Criteria:**
- [ ] AC-1.1: WAL reader task connects to PostgreSQL using logical replication with pgoutput protocol
- [ ] AC-1.2: Replication slot is created on startup with a stable name by default (e.g., `mini_graph_slot`), optionally configurable/suffixed if multiple instances are run
- [ ] AC-1.3: Publication includes only `comments` and `documents` tables
- [ ] AC-1.4: Comments table has `REPLICA IDENTITY FULL` enabled so DELETE events include all columns
- [ ] AC-1.5: INSERT events on `comments` table trigger invalidation hint generation
- [ ] AC-1.6: UPDATE events on `comments` table trigger invalidation hint generation
- [ ] AC-1.7: DELETE events on `comments` table trigger invalidation hint generation with `document_id` extracted from old row data

### US-2: Stream Document Deletion Events
**As a** system operator
**I want to** capture DELETE operations on the `documents` table
**So that** clients subscribed to deleted documents are notified to handle cleanup

**Acceptance Criteria:**
- [ ] AC-2.1: DELETE events on `documents` table trigger invalidation hint generation
- [ ] AC-2.2: Documents table uses default replica identity (primary key `id` is sufficient for Phase 1)
- [ ] AC-2.3: INSERT and UPDATE operations on `documents` table are consumed but do not generate invalidations (Phase 1 scope limitation)

### US-3: Generate Document-Scoped Invalidation Hints
**As a** Edge module
**I want to** receive invalidation hints in the format `comments:document_id:<document_id>`
**So that** I can use the inverted index to match affected subscriptions

**Acceptance Criteria:**
- [ ] AC-3.1: Comment INSERT generates hint `comments:document_id:<document_id>` extracted from new row
- [ ] AC-3.2: Comment UPDATE generates hint `comments:document_id:<document_id>` extracted from new row
- [ ] AC-3.3: Comment DELETE generates hint `comments:document_id:<document_id>` extracted from old row
- [ ] AC-3.4: Document DELETE generates hint `documents:id:<document_id>` extracted from old row
- [ ] AC-3.5: Hint generation is unit-testable with mock WAL event input

### US-4: Deliver Invalidations to Rooms Actor
**As a** WAL reader task
**I want to** send invalidation hints to the rooms actor via the existing bounded channel pattern
**So that** document rooms receive notifications using the established actor model

**Acceptance Criteria:**
- [ ] AC-4.1: WAL reader sends invalidation hints to rooms actor using `broadcast_to_room` method
- [ ] AC-4.2: Invalidation is wrapped in a new `ServerMessage::Invalidation` variant
- [ ] AC-4.3: Document ID is extracted from hint to determine target room
- [ ] AC-4.4: Failed sends due to full channel are counted in metrics and logged (no crash)
- [ ] AC-4.5: WAL reader continues processing subsequent events after delivery failure

### US-5: Track Replication Progress with LSN Management
**As a** WAL reader task
**I want to** report consumed LSN positions to PostgreSQL at transaction commit boundaries
**So that** PostgreSQL can recycle WAL files and prevent disk exhaustion

**Acceptance Criteria:**
- [ ] AC-5.1: LSN is tracked in-memory using atomic or actor-owned state
- [ ] AC-5.2: LSN is updated after invalidation is successfully enqueued to rooms actor (not after client delivery)
- [ ] AC-5.3: LSN progress is reported to PostgreSQL via `update_applied_lsn()` at COMMIT message boundaries
- [ ] AC-5.4: On restart, replication resumes from a new LSN (acceptable loss for Phase 1)
- [ ] AC-5.5: Current LSN position is exposed via metrics for monitoring

### US-6: Handle Backpressure with Bounded Channels
**As a** WAL reader task
**I want to** use bounded channels with explicit capacity limits
**So that** the system handles overload gracefully without memory exhaustion

**Acceptance Criteria:**
- [ ] AC-6.1: Internal WAL event channel has capacity of 256-512 messages
- [ ] AC-6.2: When channel is full, oldest event is dropped and `wal_events_dropped_total` metric increments
- [ ] AC-6.3: Dropped events are logged with table name and operation type
- [ ] AC-6.4: Channel capacity is configurable via constant for tuning
- [ ] AC-6.5: Backpressure does not block PostgreSQL replication stream (async handling)

### US-7: Manage Replication Slot Lifecycle
**As a** system operator
**I want to** automatic replication slot creation on startup and manual cleanup capability
**So that** WAL retention is managed without causing storage bloat

**Acceptance Criteria:**
- [ ] AC-7.1: Replication slot is created automatically on WAL reader startup if not exists
- [ ] AC-7.2: Existing slot with matching name is reused on restart (resume from last position)
- [ ] AC-7.3: Slot name is stable by default (e.g., `mini_graph_slot`), optionally configurable/suffixed if multiple instances are run
- [ ] AC-7.4: Slot health metrics are exposed: `wal_slot_active` (boolean) and `wal_retained_bytes`
- [ ] AC-7.5: Manual slot drop is available via admin command or manual `psql` (not automatic on shutdown)

### US-8: Monitor WAL Consumer Health
**As a** system operator
**I want to** observe WAL consumption metrics
**So that** I can detect lag, dropped events, and slot health issues

**Acceptance Criteria:**
- [ ] AC-8.1: Metric `wal_events_consumed_total` increments for each processed WAL event
- [ ] AC-8.2: Metric `wal_events_dropped_total` increments when bounded channel is full
- [ ] AC-8.3: Metric `wal_lag_seconds` tracks time between commit timestamp and processing time
- [ ] AC-8.4: Metric `wal_slot_active` reports 1 when slot is active, 0 otherwise
- [ ] AC-8.5: Metric `wal_retained_bytes` reports bytes retained in replication slot from `pg_replication_slots`
- [ ] AC-8.6: Metrics are exposed via existing `/debug/metrics` endpoint

### US-9: Automate PostgreSQL Setup for Local Development
**As a** developer
**I want to** PostgreSQL configuration automated on server startup
**So that** I can run the project locally without manual database setup

**Acceptance Criteria:**
- [ ] AC-9.1: On startup, server executes DDL to create `documents` and `comments` tables if not exist
- [ ] AC-9.2: On startup, server sets `REPLICA IDENTITY FULL` for `comments` only
- [ ] AC-9.3: On startup, server creates publication if not exists: `CREATE PUBLICATION mini_graph_pub FOR TABLE documents, comments`
- [ ] AC-9.4: Automated setup is idempotent (safe to run multiple times)
- [ ] AC-9.5: Setup failures are logged with actionable error messages (e.g., insufficient permissions)

### US-10: Manual Verification Workflow
**As a** developer
**I want to** manually trigger database changes with simple SQL
**So that** I can verify the end-to-end WAL invalidation flow during development

**Acceptance Criteria:**
- [ ] AC-10.1: A developer can run simple SQL INSERT/UPDATE/DELETE statements against `comments` and observe invalidations in the browser client
- [ ] AC-10.2: A developer can DELETE a `documents` row and observe an invalidation for that document_id
- [ ] AC-10.3: The documentation includes example SQL statements for each operation type (INSERT/UPDATE/DELETE)

## Functional Requirements

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-1 | WAL reader task consumes pgoutput logical replication stream | High | Connected to PostgreSQL 18 with logical replication enabled; receives and parses INSERT/UPDATE/DELETE events |
| FR-2 | Invalidation hints use inverted index format `comments:document_id:<id>` | High | Hint parsing extracts document_id from WAL row data; format matches DESIGN.md pattern |
| FR-3 | Replication slot lifecycle is managed automatically | High | Slot created on startup, reused on restart, exposes health metrics |
| FR-4 | LSN progress is reported at transaction commit boundaries | High | `update_applied_lsn()` called after invalidation enqueued; WAL files can be recycled |
| FR-5 | Bounded channels prevent memory exhaustion under load | High | Channel capacity 256-512; full channel drops events with metric increment |
| FR-6 | Only `comments` and `documents` tables are replicated | Medium | Publication definition excludes all other tables; application ignores events from unlisted tables |
| FR-7 | DELETE events include full row data via REPLICA IDENTITY FULL | High | `document_id` available in DELETE events for hint generation |
| FR-8 | PostgreSQL setup is automated for local development | Medium | DDL execution on startup; idempotent; creates tables, sets replica identity, creates publication |
| FR-9 | WAL consumer health is observable via metrics | High | Counters for consumed/dropped events; gauges for lag and slot health |
| FR-10 | Manual end-to-end verification is straightforward | Low | Simple SQL changes produce observable invalidations in browser client |

## Non-Functional Requirements

| ID | Requirement | Metric | Target |
|----|-------------|--------|--------|
| NFR-1 | WAL event processing latency | Time from commit to invalidation enqueued | < 100ms at p99 under normal load |
| NFR-2 | Backpressure handling | Dropped events under sustained load | < 1% of total events at 10x normal rate |
| NFR-3 | Replication slot stability | Slot remains active during operation | 100% uptime during normal operation |
| NFR-4 | Memory footprint | Bounded channel buffer size | < 10MB for 512-event buffer |
| NFR-5 | PostgreSQL compatibility | Supported PostgreSQL versions | PostgreSQL 15+ (tested on 18) |
| NFR-6 | Code testability | Unit test coverage for invalidation mapping | 100% of hint generation logic |
| NFR-7 | Error recovery | Reconnection on connection loss | Exponential backoff: 1s, 2s, 4s, max 30s |
| NFR-8 | Observability | Metric update frequency | Real-time (per-event counters, 1s gauge sampling) |

## Glossary

- **LSN (Log Sequence Number)**: PostgreSQL's monotonically increasing identifier for WAL positions; used to track replication progress
- **pgoutput**: PostgreSQL's built-in logical replication output plugin using binary message format (Postgres 10+)
- **Replication Slot**: Server-side resource that reserves WAL data until consumed by a client; must be dropped to prevent disk bloat
- **REPLICA IDENTITY FULL**: PostgreSQL table setting that includes all column values in DELETE events (required on `comments` so DELETE includes `document_id`)
- **Invalidation Hint**: String key in format `table:column:value` used by inverted index to match affected subscriptions
- **Bounded Channel**: Tokio MPSC channel with fixed capacity that provides backpressure via blocking or dropping when full
- **Commit Boundary**: Point in WAL stream where a transaction commits; invalidations are processed per-transaction for consistency
- **Inverted Index**: Data structure mapping hints to subscription IDs (implemented in Edge module)
- **Rooms Actor**: Existing actor managing per-document client broadcast (target for invalidation delivery)
- **Publication**: PostgreSQL logical replication object defining which tables/columns are replicated

## Out of Scope

- Incremental View Maintenance (IVM) - invalidation-and-refetch only
- LSN checkpoint persistence - in-memory tracking acceptable for Phase 1
- Automatic replication slot cleanup - manual drop via `psql`
- Schema change handling (DDL) - fail fast and require restart
- Streaming of in-progress transactions - wait for COMMIT
- Client acknowledgment before LSN advance - best-effort delivery
- Multiple PostgreSQL database shards - single database only
- Horizontal scaling of WAL reader - single instance
- Complex invalidation patterns - document-scoped only
- GraphQL or SQL query interface - predefined queries only

## Dependencies

### External Dependencies
- PostgreSQL 15+ with logical replication enabled (`wal_level = logical`)
- PostgreSQL configuration: `max_replication_slots >= 1`
- PostgreSQL configuration: `max_wal_senders >= 1`
- Network connectivity from application to PostgreSQL port

### Internal Dependencies
- Rooms actor infrastructure (established in `websocket-setup` spec)
- Metrics system (`src/metrics.rs`) for observability
- Bounded channel pattern (established in `src/rooms.rs`)
- ServerMessage enum for type-safe message passing

### Rust Crate Dependencies
- `pgwire-replication` (or alternative: `tokio-postgres` + custom wire protocol handling)
- `tokio` with MPSC channel support (already present)
- `serde`/`serde_json` for message serialization (already present)

### Development Dependencies
- Docker for running PostgreSQL 18 locally
- PostgreSQL client tools (`psql`) for manual slot inspection
- Bash (optional) for ad-hoc local automation

## Success Criteria

- Comment INSERT/UPDATE/DELETE events trigger invalidation hints within 100ms
- Document DELETE events trigger invalidation hints within 100ms
- Unit tests verify hint generation for all event types (INSERT/UPDATE/DELETE)
- Manual verification: simple SQL changes generate invalidations observable in client WebSocket messages
- Replication slot does not accumulate unbounded WAL (verified via `pg_replication_slots.wal_retained_bytes`)
- System handles 100 events/second with < 1% drop rate
- WAL reader reconnects automatically after PostgreSQL connection loss
- Metrics endpoint exposes all 5 WAL-specific metrics
- Local developer can run project with zero manual PostgreSQL setup

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Replication slot not dropped causes disk bloat | Medium | High | Expose slot health metrics; document manual cleanup via `psql` |
| Long-running transactions cause lag spikes | Low | Medium | Monitor `wal_lag_seconds`; document transaction best practices |
| Schema changes break WAL parsing | Low | Medium | Fail fast on relation message changes; require restart |
| Bounded channel drop rate too high | Medium | Low | Make capacity configurable; monitor `wal_events_dropped_total`; tune based on workload |
| PostgreSQL connection loss during startup | Medium | Medium | Implement retry with exponential backoff; log connection errors clearly |
| DELETE events missing `document_id` due to wrong replica identity | High | High | Automated setup enforces `REPLICA IDENTITY FULL` on `comments`; unit tests verify DELETE hint generation |
| pgwire-replication crate immaturity | Medium | Medium | Evaluate crate stability; fallback to `tokio-postgres` + manual protocol if needed |

## Open Questions

**Q1: Should we validate hint format during generation?**
**Proposed Answer**: Yes - add runtime assertion that generated hints match regex `^\w+:\w+:.+$` to catch bugs early.

**Q2: How should we handle multiple events within a single transaction?**
**Proposed Answer**: Generate hints for all events, emit batch at COMMIT boundary. Deduplicate hints before sending to avoid redundant invalidations.

**Q3: What happens if rooms actor is backpressured during invalidation delivery?**
**Proposed Answer**: WAL reader uses `try_send` (non-blocking). If rooms channel is full, increment metric and log warning. Accept potential missed invalidations as Phase 1 limitation.

**Q4: Should we expose raw WAL events via admin endpoint for debugging?**
**Proposed Answer**: Out of scope for Phase 1. Future enhancement: `/admin/wal/recent` endpoint showing last N events.

**Q5: How do we test WAL integration without running PostgreSQL?**
**Proposed Answer**: Inject mock `WalEvent` enums directly into hint generation logic for unit tests. Integration tests require Docker PostgreSQL.
