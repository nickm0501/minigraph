---
spec: postgres-wal-integration
phase: tasks
total_tasks: 30
created: 2026-01-17T00:00:00Z
---

# Tasks: PostgreSQL WAL Integration for Real-time Document Invalidation

## Phase 1: Make It Work (POC)

Focus: Validate WAL streaming works end-to-end. Keep iteration tight with small unit tests for pure logic (hint generation, hint routing) plus manual E2E verification. Accept hardcoded values and minimal error handling in POC.

### 1.1 Infrastructure Setup

- [ ] 1.1.1 Add PostgreSQL + replication dependencies to Cargo.toml
  - **Do**: Add `tokio-postgres` (control-plane DDL/setup), `postgres-types` (type helpers), and `pgwire-replication` (data-plane logical replication stream).
  - **Files**: `/Users/nickmaietta/projects/mini-graph/Cargo.toml`
  - **Done when**: `cargo build` succeeds with new dependencies
  - **Verify**: `cargo build`
  - **Commit**: `feat(wal): add postgres replication dependencies`
  - _Requirements: FR-1, Dependencies section_
  - _Design: Dependencies section_

- [ ] 1.1.2 Create WAL module structure with type definitions
  - **Do**: Create `src/wal/mod.rs` as module entry point, create `src/wal/types.rs` with `WalEvent`, `TupleData`, `Value`, `QueryHint` types as defined in design.md. Export public types from mod.rs. Add `pub mod wal;` to `src/lib.rs`.
  - **Files**:
    - `/Users/nickmaietta/projects/mini-graph/src/wal/mod.rs` (create)
    - `/Users/nickmaietta/projects/mini-graph/src/wal/types.rs` (create)
    - `/Users/nickmaietta/projects/mini-graph/src/lib.rs` (modify)
  - **Done when**: Types compile, module is accessible via `crate::wal::types::*`
  - **Verify**: `cargo build`
  - **Commit**: `feat(wal): create wal module with core types`
  - _Requirements: FR-2_
  - _Design: File Structure, Invalidation Hint Generator interfaces_

- [ ] 1.1.3 Create PostgreSQL module structure with config types
  - **Do**: Create `src/postgres/mod.rs` and `src/postgres/config.rs` with `PostgresConfig` struct (host, port, user, password, database, slot_name fields). Add `from_env()` method that reads from environment variables with sensible defaults (localhost:5432, postgres/postgres, mini_graph database). Add `pub mod postgres;` to `src/lib.rs`.
  - **Files**:
    - `/Users/nickmaietta/projects/mini-graph/src/postgres/mod.rs` (create)
    - `/Users/nickmaietta/projects/mini-graph/src/postgres/config.rs` (create)
    - `/Users/nickmaietta/projects/mini-graph/src/lib.rs` (modify)
  - **Done when**: `PostgresConfig::from_env()` compiles and returns config struct
  - **Verify**: `cargo build`
  - **Commit**: `feat(postgres): create postgres module with config types`
  - _Requirements: US-9, AC-9.1-9.5_
  - _Design: PostgreSQL Setup Task interfaces_

- [ ] 1.1.4 [VERIFY] Quality checkpoint: cargo fmt && cargo clippy
  - **Do**: Run `cargo fmt -- --check` and `cargo clippy` to verify code quality
  - **Verify**: Both commands exit 0 with no errors or warnings
  - **Done when**: No formatting issues, no clippy warnings
  - **Commit**: `chore(wal): pass quality checkpoint` (only if fixes needed)

### 1.2 PostgreSQL Setup Automation

- [ ] 1.2.1 Implement PostgreSQL connection helper
  - **Do**: Create `src/postgres/connection.rs` with async `connect()` function that uses tokio-postgres to establish connection using `PostgresConfig`. Include basic error handling that logs connection failures with actionable messages.
  - **Files**:
    - `/Users/nickmaietta/projects/mini-graph/src/postgres/connection.rs` (create)
    - `/Users/nickmaietta/projects/mini-graph/src/postgres/mod.rs` (modify to export)
  - **Done when**: Can establish async connection to local PostgreSQL
  - **Verify**: `cargo build` (runtime test requires PostgreSQL)
  - **Commit**: `feat(postgres): implement connection helper`
  - _Requirements: AC-9.5_
  - _Design: PostgreSQL Setup Task_

- [ ] 1.2.2 Implement PostgreSQL setup task (tables, replica identity, publication)
  - **Do**: Create `src/postgres/setup.rs` with `setup_postgres()` async function. Execute idempotent DDL: (1) CREATE TABLE IF NOT EXISTS documents (id TEXT PRIMARY KEY), (2) CREATE EXTENSION IF NOT EXISTS pgcrypto, (3) CREATE TABLE IF NOT EXISTS comments (id UUID PRIMARY KEY DEFAULT gen_random_uuid(), document_id TEXT NOT NULL, text TEXT), (4) ALTER TABLE comments REPLICA IDENTITY FULL, (5) CREATE PUBLICATION mini_graph_pub FOR TABLE documents, comments (treat "already exists" as success). Log each action. Handle errors with `SetupError` enum.
  - **Files**:
    - `/Users/nickmaietta/projects/mini-graph/src/postgres/setup.rs` (create)
    - `/Users/nickmaietta/projects/mini-graph/src/postgres/mod.rs` (modify to export)
  - **Done when**: Running setup creates tables and publication (verify via psql)
  - **Verify**: `cargo build` then manual test with local PostgreSQL
  - **Commit**: `feat(postgres): implement automated setup task`
  - _Requirements: US-9, AC-9.1-9.4, FR-6, FR-7, FR-8_
  - _Design: PostgreSQL Setup Task, Technical Decisions (setup automation)_

- [ ] 1.2.3 Integrate setup task into server startup
  - **Do**: Modify `src/lib.rs` `run_server()` to call `postgres::setup::setup_postgres()` before starting rooms actor. Add error handling: if setup fails, log error and exit (fail fast). Use `PostgresConfig::from_env()` for configuration.
  - **Files**: `/Users/nickmaietta/projects/mini-graph/src/lib.rs`
  - **Done when**: Server startup runs PostgreSQL setup, logs actions, fails gracefully on DB unavailable
  - **Verify**: `cargo run` with PostgreSQL running shows setup logs
  - **Commit**: `feat(postgres): integrate setup into server startup`
  - _Requirements: AC-9.1-9.5_
  - _Design: Startup Coordination_

- [ ] 1.2.4 [VERIFY] Quality checkpoint: cargo fmt && cargo clippy && cargo test
  - **Do**: Run quality commands to verify code compiles and passes checks
  - **Verify**: All commands exit 0
  - **Done when**: No lint errors, no type errors, existing tests pass
  - **Commit**: `chore(postgres): pass quality checkpoint` (only if fixes needed)

### 1.3 ServerMessage Extension

- [ ] 1.3.1 Add ServerMessage::Invalidation variant
  - **Do**: Extend `ServerMessage` enum in `src/types.rs` with new variant: `Invalidation { hints: Vec<String>, timestamp: u64 }`. Ensure serde serialization works (tagged enum pattern already in use).
  - **Files**: `/Users/nickmaietta/projects/mini-graph/src/types.rs`
  - **Done when**: `ServerMessage::Invalidation { hints: vec![], timestamp: 0 }` compiles and serializes to JSON
  - **Verify**: `cargo build`
  - **Commit**: `feat(types): add ServerMessage::Invalidation variant`
  - _Requirements: US-4, AC-4.2_
  - _Design: ServerMessage Extension_

### 1.4 Metrics Extension

- [ ] 1.4.1 Add WAL metrics fields and methods to Metrics struct
  - **Do**: Extend `src/metrics.rs` Metrics struct with 5 new AtomicU64 fields: `wal_events_consumed_total`, `wal_events_dropped_total`, `wal_lag_seconds`, `wal_lsn`, `wal_slot_active`, `wal_retained_bytes`. Add corresponding `inc_*` methods for counters and `set_*` methods for gauges. Extend `MetricsSnapshot` with same fields and update `snapshot()` method.
  - **Files**: `/Users/nickmaietta/projects/mini-graph/src/metrics.rs`
  - **Done when**: All 5 new metrics accessible via `/debug/metrics` endpoint
  - **Verify**: `cargo build` then curl `/debug/metrics` shows new fields
  - **Commit**: `feat(metrics): add WAL-specific metrics`
  - _Requirements: US-8, AC-8.1-8.6, FR-9_
  - _Design: Metrics Extension_

- [ ] 1.4.2 [VERIFY] Quality checkpoint: cargo fmt && cargo clippy && cargo test
  - **Do**: Run quality commands
  - **Verify**: All commands exit 0
  - **Done when**: No lint errors, no type errors, existing tests pass
  - **Commit**: `chore(metrics): pass quality checkpoint` (only if fixes needed)

### 1.5 Hint Generation

- [ ] 1.5.1 Implement invalidation hint generator
  - **Do**: Create `src/wal/hint_generator.rs` with `generate_invalidation_hints(event: &WalEvent) -> Vec<QueryHint>` function. Implement routing rules: (1) comments INSERT/UPDATE/DELETE -> `comments:document_id:<document_id>`, (2) documents DELETE -> `documents:id:<id>`, (3) comments UPDATE with changed document_id -> two hints (old and new). Use pattern matching on WalEvent variants. Log errors for missing document_id column (return empty vec). Export from wal/mod.rs.
  - **Files**:
    - `/Users/nickmaietta/projects/mini-graph/src/wal/hint_generator.rs` (create)
    - `/Users/nickmaietta/projects/mini-graph/src/wal/mod.rs` (modify to export)
  - **Done when**: Function compiles and handles all WalEvent variants
  - **Verify**: `cargo build`
  - **Commit**: `feat(wal): implement invalidation hint generator`
  - _Requirements: US-3, AC-3.1-3.5, FR-2_
  - _Design: Invalidation Hint Generator_

- [ ] 1.5.2 Implement QueryHint to_key() method
  - **Do**: Add `to_key(&self) -> String` method to `QueryHint` struct in `src/wal/types.rs` that formats hint as `table:column:value` string for WebSocket delivery.
  - **Files**: `/Users/nickmaietta/projects/mini-graph/src/wal/types.rs`
  - **Done when**: `QueryHint { table: "comments".into(), column: "document_id".into(), value: "doc123".into() }.to_key()` returns `"comments:document_id:doc123"`
  - **Verify**: `cargo build`
  - **Commit**: `feat(wal): implement QueryHint to_key serialization`
  - _Requirements: AC-3.1-3.4_
  - _Design: Invalidation Hint Generator interfaces_

- [ ] 1.5.3 Unit tests for hint generation + QueryHint serialization
  - **Do**: Create `tests/wal_hint_generation_test.rs` with focused unit tests for:
    - `generate_invalidation_hints()` (all Phase 1 cases)
    - `QueryHint::to_key()`

    Keep tests pure by constructing `WalEvent` directly (no Postgres needed).
  - **Files**: `/Users/nickmaietta/projects/mini-graph/tests/wal_hint_generation_test.rs` (create)
  - **Done when**: Tests cover INSERT/UPDATE/DELETE and document delete cases and pass
  - **Verify**: `cargo test wal_hint`
  - **Commit**: `test(wal): add unit tests for hint generation`
  - _Requirements: AC-3.5, NFR-6_
  - _Design: Test Strategy - Unit Tests_

### 1.6 WAL Reader Actor

- [ ] 1.6.1 Create WAL reader handle and command types
  - **Do**: Create `src/wal/reader.rs` with `WalReaderHandle` struct (matches RoomsHandle pattern) and `WalReaderCommand` enum with `Stop` variant. Implement `WalReaderHandle::start()` that spawns actor task. Define channel capacity constant `WAL_READER_CHANNEL_CAPACITY = 256`. Export from wal/mod.rs.
  - **Files**:
    - `/Users/nickmaietta/projects/mini-graph/src/wal/reader.rs` (create)
    - `/Users/nickmaietta/projects/mini-graph/src/wal/mod.rs` (modify to export)
  - **Done when**: `WalReaderHandle::start()` compiles (actor body can be placeholder)
  - **Verify**: `cargo build`
  - **Commit**: `feat(wal): create WAL reader handle and actor skeleton`
  - _Requirements: US-1, FR-1_
  - _Design: WAL Reader Actor, Existing Patterns (Actor Model)_

- [ ] 1.6.2 Implement replication slot creation in setup task
  - **Do**: Add replication slot creation to `src/postgres/setup.rs`: (1) Check if slot exists via `pg_replication_slots`, (2) If not, create using `SELECT pg_create_logical_replication_slot('mini_graph_slot', 'pgoutput')`. Log slot status. Handle case where slot already exists (reuse).
  - **Files**: `/Users/nickmaietta/projects/mini-graph/src/postgres/setup.rs`
  - **Done when**: Running setup creates or reuses replication slot
  - **Verify**: Manual test: run server, check `SELECT * FROM pg_replication_slots`
  - **Commit**: `feat(postgres): add replication slot creation to setup`
  - _Requirements: US-7, AC-7.1-7.3, FR-3_
  - _Design: PostgreSQL Setup Task_

- [ ] 1.6.3 [VERIFY] Quality checkpoint: cargo fmt && cargo clippy && cargo test
  - **Do**: Run quality commands
  - **Verify**: All commands exit 0
  - **Done when**: No lint errors, no type errors, existing tests pass
  - **Commit**: `chore(wal): pass quality checkpoint` (only if fixes needed)

- [ ] 1.6.4 Implement WAL reader actor core loop (POC - simplified)
  - **Do**: Implement `wal_reader_actor()` async function using `pgwire-replication` that: (1) Connects to PostgreSQL using the replication protocol, (2) Starts logical replication for slot `mini_graph_slot` and publication `mini_graph_pub`, (3) Receives replication events in a loop, (4) Logs raw/decoded events. For POC: minimal error handling, no reconnection logic yet.
  - **Files**: `/Users/nickmaietta/projects/mini-graph/src/wal/reader.rs`
  - **Done when**: WAL reader connects and logs WAL events when SQL changes occur
  - **Verify**: `cargo run`, then in psql: `INSERT INTO comments (document_id, text) VALUES ('doc1', 'hello')`, observe logs
  - **Commit**: `feat(wal): implement WAL reader core loop (POC)`
  - _Requirements: US-1, AC-1.1-1.7, FR-1_
  - _Design: WAL Reader Actor, Technical Decisions (WAL consumption crate)_

- [ ] 1.6.5 Parse pgoutput messages into WalEvent types
  - **Do**: Implement pgoutput decoding to produce `WalEvent` values. Depending on what `pgwire-replication` exposes, either:
    - decode from its pgoutput event representation into `WalEvent`, or
    - parse the pgoutput payload bytes into (BEGIN, RELATION, INSERT, UPDATE, DELETE, COMMIT)

    Cache relation metadata (relation_id -> table name, column names) and extract `comments.document_id` values for hint generation.
  - **Files**:
    - `/Users/nickmaietta/projects/mini-graph/src/wal/pgoutput.rs` (create)
    - `/Users/nickmaietta/projects/mini-graph/src/wal/reader.rs` (modify)
    - `/Users/nickmaietta/projects/mini-graph/src/wal/mod.rs` (modify to export)
  - **Done when**: WalEvent structs populated with parsed table name and column values
  - **Verify**: `cargo run`, INSERT comment, logs show parsed WalEvent with document_id value
  - **Commit**: `feat(wal): implement pgoutput message parsing`
  - _Requirements: AC-1.5-1.7, AC-2.1_
  - _Design: WAL Reader Actor, Invalidation Hint Generator_

- [ ] 1.6.6 [VERIFY] Quality checkpoint: cargo fmt && cargo clippy && cargo test
  - **Do**: Run quality commands
  - **Verify**: All commands exit 0
  - **Done when**: No lint errors, no type errors, existing tests pass
  - **Commit**: `chore(wal): pass quality checkpoint` (only if fixes needed)

### 1.7 Integration with Rooms Actor

- [ ] 1.7.1 Add RoomsHandle to WAL reader actor
  - **Do**: Modify `WalReaderHandle::start()` to accept `RoomsHandle` parameter. Pass to actor task. Store in actor state for broadcasting invalidations.
  - **Files**: `/Users/nickmaietta/projects/mini-graph/src/wal/reader.rs`
  - **Done when**: WAL reader actor has access to RoomsHandle
  - **Verify**: `cargo build`
  - **Commit**: `feat(wal): pass RoomsHandle to WAL reader actor`
  - _Requirements: US-4, AC-4.1_
  - _Design: WAL Reader Actor interfaces_

- [ ] 1.7.2 Implement transaction buffer for hint accumulation
  - **Do**: Add `TransactionBuffer` struct to `src/wal/reader.rs` with `HashSet<QueryHint>` for de-duplication. Implement `add_hint()` and `flush()` methods. Accumulate hints on INSERT/UPDATE/DELETE, flush on COMMIT.
  - **Files**: `/Users/nickmaietta/projects/mini-graph/src/wal/reader.rs`
  - **Done when**: Hints accumulated per-transaction, flushed at COMMIT
  - **Verify**: `cargo build`
  - **Commit**: `feat(wal): implement transaction buffer for hint de-duplication`
  - _Requirements: Open Question Q2 (transaction handling)_
  - _Design: Transaction Buffer + Routing_

- [ ] 1.7.2a Unit tests for TransactionBuffer + HintRouter
  - **Do**: Extend `tests/wal_hint_generation_test.rs` with tests for:
    - TransactionBuffer de-duplication
    - HintRouter grouping into document_id rooms
    - Flush clears the buffer
  - **Files**: `/Users/nickmaietta/projects/mini-graph/tests/wal_hint_generation_test.rs`
  - **Done when**: Tests pass and don’t require Postgres
  - **Verify**: `cargo test transaction_buffer`
  - **Commit**: `test(wal): add transaction buffer routing tests`

- [ ] 1.7.3 Route invalidations to rooms via a HintRouter
  - **Do**: Implement a minimal `HintRouter` (Phase 1) that maps a set of `QueryHint` values to target rooms.

    Suggested approach:
    1. `HintRouter::route(&HashSet<QueryHint>) -> HashMap<DocumentId, Vec<QueryHint>>`
    2. Phase 1 routing rules:
       - `comments:document_id:<doc>` routes to room `<doc>`
       - `documents:id:<doc>` routes to room `<doc>`
    3. On transaction flush:
       - Call router
       - For each (document_id, hints) group, create `ServerMessage::Invalidation` where `hints` are `QueryHint::to_key()`
       - Call `rooms.broadcast_to_room(document_id, message)`

    Note: this avoids parsing routing information back out of serialized `table:column:value` strings.

  - Handle `RoomCommandError::ChannelFull` by incrementing `wal_events_dropped_total` metric and logging.
  - **Files**: `/Users/nickmaietta/projects/mini-graph/src/wal/reader.rs`
  - **Done when**: Invalidation messages delivered to rooms actor
  - **Verify**: `cargo run`, browser client joins room "doc1", INSERT comment with document_id="doc1", observe Invalidation message in browser console
  - **Commit**: `feat(wal): route invalidations to rooms actor`
  - _Requirements: US-4, AC-4.1-4.5_
  - _Design: Data Flow, Transaction Buffer + Routing_

- [ ] 1.7.4 Add Metrics tracking to WAL reader
  - **Do**: Pass `Arc<Metrics>` to WAL reader actor. Call `inc_wal_events_consumed()` for each processed event. Call `inc_wal_events_dropped()` on channel full. Call `set_wal_lsn()` on COMMIT processed.
  - **Files**: `/Users/nickmaietta/projects/mini-graph/src/wal/reader.rs`
  - **Done when**: WAL metrics update as events are processed
  - **Verify**: `cargo run`, INSERT comment, check `/debug/metrics` shows `wal_events_consumed_total > 0`
  - **Commit**: `feat(wal): add metrics tracking to WAL reader`
  - _Requirements: US-8, AC-8.1-8.5_
  - _Design: Metrics Extension_

- [ ] 1.7.5 [VERIFY] Quality checkpoint: cargo fmt && cargo clippy && cargo test
  - **Do**: Run quality commands
  - **Verify**: All commands exit 0
  - **Done when**: No lint errors, no type errors, existing tests pass
  - **Commit**: `chore(wal): pass quality checkpoint` (only if fixes needed)

### 1.8 LSN Tracking

- [ ] 1.8.1 Implement LSN progress reporting to PostgreSQL
  - **Do**: After COMMIT processed and invalidation enqueue attempts are made (even if some invalidations are dropped due to backpressure), report replication progress to PostgreSQL. Track `last_applied_lsn` in actor state and expose it via metrics.
  - **Files**: `/Users/nickmaietta/projects/mini-graph/src/wal/reader.rs`
  - **Done when**: LSN reported to PostgreSQL, `pg_replication_slots.restart_lsn` advances
  - **Verify**: Query `SELECT restart_lsn FROM pg_replication_slots WHERE slot_name = 'mini_graph_slot'` before and after INSERTs
  - **Commit**: `feat(wal): implement LSN progress reporting`
  - _Requirements: US-5, AC-5.1-5.5, FR-4_
  - _Design: LSN Tracking Atomicity_

### 1.9 AppState Integration

- [ ] 1.9.1 Add WalReaderHandle to AppState
  - **Do**: Modify `src/state.rs` to add `wal_reader: WalReaderHandle` field. Update `AppState::new()` signature. Modify `src/lib.rs` to create WalReaderHandle and pass to AppState. Start WAL reader after rooms actor.
  - **Files**:
    - `/Users/nickmaietta/projects/mini-graph/src/state.rs`
    - `/Users/nickmaietta/projects/mini-graph/src/lib.rs`
  - **Done when**: Server starts with WAL reader connected and processing events
  - **Verify**: `cargo run` shows WAL reader connection logs, then test with SQL INSERT
  - **Commit**: `feat(state): integrate WalReaderHandle into AppState`
  - _Requirements: FR-1_
  - _Design: Startup Coordination_

- [ ] 1.9.2 [VERIFY] Quality checkpoint: cargo fmt && cargo clippy && cargo test
  - **Do**: Run quality commands
  - **Verify**: All commands exit 0
  - **Done when**: No lint errors, no type errors, existing tests pass
  - **Commit**: `chore(state): pass quality checkpoint` (only if fixes needed)

### 1.10 POC Checkpoint

- [ ] 1.10.1 End-to-end manual verification
  - **Do**: (1) Start server with `cargo run`, (2) Open browser to `http://localhost:3030`, (3) Join document room "doc1" via WebSocket, (4) In psql: `INSERT INTO comments (document_id, text) VALUES ('doc1', 'test')`, (5) Observe `ServerMessage::Invalidation` in browser DevTools console, (6) Verify `/debug/metrics` shows WAL metrics. Document any issues found.
  - **Files**: None (verification only)
  - **Done when**: Full flow works: DB change -> WAL -> hint -> rooms -> browser
  - **Verify**: Manual verification as described
  - **Commit**: `feat(wal): complete POC - end-to-end invalidation working`
  - _Requirements: US-10, AC-10.1-10.3, Success Criteria_
  - _Design: E2E Tests (Manual Verification)_

## Phase 2: Refactoring

After POC validated, clean up code structure and add resilience.

- [ ] 2.1 Extract PostgreSQL connection management
  - **Do**: Replace ad-hoc connections with clearer connection management. Keep control-plane (setup/DDL) and data-plane (replication stream) connections separate. If pooling is needed later, add it then.
  - **Files**: `/Users/nickmaietta/projects/mini-graph/src/postgres/connection.rs`
  - **Done when**: Clean connection management, no duplicate connection code
  - **Verify**: `cargo build`, manual test startup
  - **Commit**: `refactor(postgres): extract connection pooling`
  - _Design: Control-plane vs data-plane connections_

- [ ] 2.2 Add exponential backoff reconnection logic
  - **Do**: Implement `reconnect_with_backoff()` function for WAL reader. On connection loss: log error, retry with exponential backoff (1s, 2s, 4s, max 30s). Resume from last applied LSN on reconnect.
  - **Files**: `/Users/nickmaietta/projects/mini-graph/src/wal/reader.rs`
  - **Done when**: WAL reader reconnects automatically after PostgreSQL restart
  - **Verify**: Start server, stop PostgreSQL, observe retry logs, start PostgreSQL, observe reconnection
  - **Commit**: `refactor(wal): add exponential backoff reconnection`
  - _Requirements: NFR-7_
  - _Design: Exponential Backoff Reconnection_

- [ ] 2.3 Add comprehensive error handling
  - **Do**: Replace panics with proper error types. Add `WalError` enum in `src/wal/types.rs`. Propagate errors up to actor loop. Log errors with context (table name, operation, LSN). Handle partial failures gracefully.
  - **Files**:
    - `/Users/nickmaietta/projects/mini-graph/src/wal/types.rs`
    - `/Users/nickmaietta/projects/mini-graph/src/wal/reader.rs`
  - **Done when**: No panics in WAL path, all errors logged with context
  - **Verify**: `cargo clippy`, manual test with invalid data
  - **Commit**: `refactor(wal): add comprehensive error handling`
  - _Design: Error Handling table_

- [ ] 2.4 [VERIFY] Quality checkpoint: cargo fmt && cargo clippy && cargo test
  - **Do**: Run quality commands after refactoring
  - **Verify**: All commands exit 0
  - **Done when**: No lint errors, no type errors, existing tests pass
  - **Commit**: `chore(wal): pass quality checkpoint after refactoring` (only if fixes needed)

## Phase 4: Quality Gates

- [ ] 4.1 [VERIFY] Full local CI: cargo fmt && cargo clippy && cargo test && cargo build --release
  - **Do**: Run complete local CI suite
  - **Verify**: All commands pass:
    - `cargo fmt -- --check`
    - `cargo clippy -- -D warnings`
    - `cargo test`
    - `cargo build --release`
  - **Done when**: Build succeeds, all tests pass, no warnings
  - **Commit**: `chore(wal): pass local CI` (if fixes needed)

- [ ] 4.2 Create PR and verify CI
  - **Do**:
    1. Verify current branch is feature branch: `git branch --show-current`
    2. If on main, STOP and alert user
    3. Push branch: `git push -u origin <branch-name>`
    4. Create PR: `gh pr create --title "feat(wal): PostgreSQL WAL integration for real-time invalidation" --body "Implements WAL streaming from PostgreSQL to generate document invalidation hints. Enables real-time updates to WebSocket clients when database changes occur.\n\n## Changes\n- WAL reader actor consuming pgoutput logical replication\n- Invalidation hint generation for comments/documents tables\n- Transaction de-duplication and routing to rooms actor\n- WAL-specific metrics (consumed, dropped, lag, LSN)\n- Automated PostgreSQL setup (tables, replica identity, publication, slot)\n\n## Testing\n- Unit tests for hint generation\n- Manual E2E verification with browser client\n\nCloses postgres-wal-integration spec."`
  - **Verify**: `gh pr checks --watch` shows all green
  - **Done when**: CI pipeline passes, PR ready for review
  - **If CI fails**: Read failures with `gh pr checks`, fix locally, push fixes, re-verify
  - **Commit**: None (PR creation)

- [ ] 4.3 [VERIFY] AC checklist verification
  - **Do**: Read requirements.md, verify each acceptance criterion is satisfied:
    - AC-1.1 through AC-1.7: WAL reader connects, replicates comments table events
    - AC-2.1 through AC-2.3: Document DELETE triggers invalidation, INSERT/UPDATE ignored
    - AC-3.1 through AC-3.5: Hint format correct, unit testable
    - AC-4.1 through AC-4.5: Invalidations delivered to rooms actor, failures handled
    - AC-5.1 through AC-5.5: LSN tracked, reported at COMMIT boundaries
    - AC-6.1 through AC-6.5: Bounded channels, backpressure handling
    - AC-7.1 through AC-7.5: Slot created, reused, health metrics exposed
    - AC-8.1 through AC-8.6: All 5 WAL metrics exposed via /debug/metrics
    - AC-9.1 through AC-9.5: Automated setup idempotent, logs actions
    - AC-10.1 through AC-10.3: Manual verification workflow documented
  - **Verify**: Manual review against implementation
  - **Done when**: All 55 acceptance criteria confirmed met
  - **Commit**: None (verification only)

## Notes

### POC Shortcuts Taken
- Hardcoded replication slot name "mini_graph_slot"
- No TLS for PostgreSQL connection
- Minimal relation cache (rebuild on restart)
- No LSN persistence across restarts
- Simplified error handling (log and continue)
- No automated integration tests

### Production TODOs
- Add TLS/SSL support for PostgreSQL connection
- Persist last applied LSN to database or file
- Implement slot health monitoring (retained_bytes, lag_seconds from pg_replication_slots)
- Add configurable slot name suffix for multi-instance deployments
- Create integration tests with testcontainers-rs
- Add graceful shutdown (flush pending LSN, clean disconnect)
- Schema change detection and hot-reload (parse Relation messages)

