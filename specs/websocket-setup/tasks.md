---
spec: websocket-setup
phase: tasks
total_tasks: 22
created: 2026-01-14T22:30:00Z
---

# Implementation Tasks: websocket-setup

## Phase 1: Make It Work (POC)

### Task 1: Project Setup and Dependencies

**Do:**
1. Fix Cargo.toml edition from 2024 to 2021
2. Add all required dependencies with exact versions:
   ```toml
   [dependencies]
   tokio = { version = "1", features = ["full"] }
   axum = { version = "0.7", features = ["ws"] }
   tower-http = { version = "0.6", features = ["fs"] }
   serde = { version = "1.0", features = ["derive"] }
   serde_json = "1.0"
   uuid = { version = "1.0", features = ["v4"] }
   ```
3. Create `static/` directory in project root
4. Test that the project compiles

**Files:**
- `Cargo.toml` (modify)
- `static/` (create directory)

**Done when:**
- [x] Edition is set to 2021
- [x] All 6 dependencies added
- [x] static/ directory exists
- [x] `cargo build` succeeds with no errors

**Verify:**
```bash
cargo build
```

**Commit:**
```
Add WebSocket dependencies and fix Cargo edition to 2021

Add core dependencies required for WebSocket server implementation:
- tokio for async runtime
- axum for HTTP/WebSocket server with built-in ws support
- tower-http for static file serving
- serde/serde_json for JSON message protocol
- uuid for unique client identification

Fix edition from 2024 (invalid) to 2021 (current stable).
These dependencies form the foundation for the POC WebSocket server.
```

_Requirements: TC-1, TC-5_
_Design: Section 4_

---

### Task 2: Basic Server with Static File Serving

**Do:**
1. Replace `src/main.rs` content with Axum server setup
2. Create router with placeholder `/ws` route (returns 404 for now)
3. Add static file serving using `tower_http::services::ServeDir`
4. Bind server to `0.0.0.0:3030`
5. Add startup println showing server address
6. Use minimal code - no modules yet

**Files:**
- `src/main.rs` (modify)

**Done when:**
- [x] Server starts on port 3030
- [x] Accessing http://localhost:3030/ returns 404 (no index.html yet)
- [x] Server prints startup message with URL
- [x] No compile errors or warnings

**Verify:**
```bash
cargo run
# In another terminal:
curl http://localhost:3030/
```

**Commit:**
```
Add basic Axum server with static file serving

Bootstrap the Axum HTTP server to serve as foundation for WebSocket endpoint.
Static file serving enables us to serve the HTML client from the same server,
simplifying deployment and avoiding CORS issues during development.

Server binds to 0.0.0.0:3030 to allow connections from other devices
on the local network for multi-device testing later.
```

_Requirements: FR-1, FR-8, TC-6_
_Design: Section 1, Section 6_

---

### Task 3: Minimal HTML Client

**Do:**
1. Create `static/index.html` with basic structure
2. Add split-screen layout: left pane (40% width) and right pane (60% width)
3. Left pane: document ID input, join button, message input, send button
4. Right pane: message display area (div)
5. Add minimal inline CSS for layout
6. NO JavaScript yet - just static HTML structure
7. Use simple styling - no fancy design needed for POC

**Files:**
- `static/index.html` (create)

**Done when:**
- [x] File exists at static/index.html
- [x] Accessing http://localhost:3030/ shows the HTML page
- [x] Two-column layout renders correctly
- [x] All input elements and buttons present

**Verify:**
```bash
cargo run
# Open browser to http://localhost:3030/
# Visually verify layout appears
```

**Commit:**
```
Add basic HTML client with split-screen layout

Create split-screen UI matching the demo requirements: left pane for input
(40% width) and right pane for message stream (60% width).

This layout mirrors the Figma-like collaboration interface from the original
design doc, where users can input messages on the left and see live updates
on the right. Static HTML only at this stage - JavaScript comes next.
```

_Requirements: FR-8, FR-12_
_Design: Section 4 (static/index.html)_

---

### Task 4: Message Type Definitions

**Do:**
1. Create `src/types.rs` file
2. Define `ClientId` and `DocumentId` as type aliases (String)
3. Define `ClientMessage` enum with two variants:
   - Join { document_id: DocumentId }
   - SendMessage { text: String }
4. Define `ServerMessage` enum with three variants:
   - Joined { client_id: ClientId, document_id: DocumentId }
   - Message { from: ClientId, text: String, timestamp: u64 }
   - Error { message: String }
5. Add serde derives for serialization
6. Add `impl ServerMessage::new_message()` helper that auto-generates timestamp
7. Skip custom error types for POC - use String errors

**Files:**
- `src/types.rs` (create)

**Done when:**
- [ ] File compiles without errors
- [ ] Both enums have `#[serde(tag = "type", rename_all = "snake_case")]`
- [ ] ClientMessage has Deserialize
- [ ] ServerMessage has Serialize
- [ ] new_message() helper creates timestamp

**Verify:**
```bash
cargo build
```

**Commit:**
```
Add WebSocket message protocol definitions

Define the JSON message protocol for client-server communication.
Using tagged enums with serde enables type-safe message parsing and
ensures protocol consistency between client and server.

The timestamp helper in ServerMessage::new_message() provides consistent
Unix millisecond timestamps for message ordering and display formatting.
```

_Requirements: FR-9_
_Design: Section 4 (types.rs), Section 5_

---

### Task 5: WebSocket Handler Skeleton

**Do:**
1. Create `src/websocket.rs` file
2. Add imports for Axum WebSocket types
3. Create `websocket_handler` function that accepts WebSocketUpgrade
4. Implement `handle_socket` function that:
   - Generates client_id using UUID v4
   - Splits socket into sender/receiver using `socket.split()`
   - Creates unbounded mpsc channel for now (we'll fix to bounded later)
   - Spawns send task that reads from channel and sends to WebSocket
   - Has receive loop that reads from WebSocket
   - Prints received messages for debugging
5. NO room management yet - just echo functionality
6. Add cleanup println when connection closes

**Files:**
- `src/websocket.rs` (create)

**Done when:**
- [ ] File compiles
- [ ] websocket_handler and handle_socket functions exist
- [ ] UUID v4 client_id generated
- [ ] Socket split into sender/receiver
- [ ] Send and receive tasks structured
- [ ] Prints debug messages

**Verify:**
```bash
cargo build
```

**Commit:**
```
Add WebSocket handler skeleton with connection management

Implement basic WebSocket connection lifecycle with UUID client identification.
Splitting the socket into separate send/receive streams enables concurrent
bidirectional communication without blocking.

Using unbounded channels for POC to get something working quickly - will be
replaced with bounded channels in Phase 2 to prevent backpressure issues.
```

_Requirements: FR-1, FR-3_
_Design: Section 4 (websocket.rs)_

---

### Task 6: Wire Up WebSocket Route

**Do:**
1. Update `src/main.rs` to declare `mod websocket` and `mod types`
2. Change `/ws` route to call `websocket::websocket_handler`
3. Test WebSocket upgrade using browser DevTools
4. At this point, connection should establish but not do anything useful yet

**Files:**
- `src/main.rs` (modify)

**Done when:**
- [ ] Modules declared at top of main.rs
- [ ] /ws route wired to websocket_handler
- [ ] No compile errors
- [ ] Server starts successfully

**Verify:**
```bash
cargo run
# Open browser DevTools console and run:
# ws = new WebSocket('ws://localhost:3030/ws')
# Should see connection open in Network tab
```

**Commit:**
```
Wire up WebSocket endpoint at /ws

Connect the WebSocket handler to the /ws route, completing the server setup.
This enables clients to upgrade HTTP connections to WebSocket for persistent
bidirectional communication, which is essential for real-time updates.
```

_Requirements: FR-1_
_Design: Section 4 (main.rs)_

---

### Task 7: Client WebSocket Connection Logic

**Do:**
1. Add `<script>` section to `static/index.html`
2. Create WebSocket connection to `ws://localhost:3030/ws` on page load
3. Add connection status display showing "Connecting...", "Connected ✓", "Disconnected ✗"
4. Add basic onopen, onmessage, onclose, onerror handlers
5. Log all events to console for debugging
6. Wire up "Join" button to send join message
7. Wire up "Send" button to send text message
8. Display received messages in right pane (just append text for now, no formatting)
9. NO reconnection logic yet - that comes later
10. Use vanilla JavaScript - no frameworks

**Files:**
- `static/index.html` (modify)

**Done when:**
- [ ] WebSocket connects on page load
- [ ] Status indicator updates on connection state changes
- [ ] Join button sends JSON message with document_id
- [ ] Send button sends JSON message with text
- [ ] Received messages appear in right pane
- [ ] Console shows all WebSocket events

**Verify:**
```bash
cargo run
# Open browser to http://localhost:3030/
# Open console, verify connection logs
# Try sending messages, verify in console
```

**Commit:**
```
Add WebSocket connection and basic message handling

Implement client-side WebSocket connection with automatic connection
on page load and basic message send/receive handlers.

Status indicator provides immediate feedback on connection state, which
is crucial for debugging and understanding the system behavior during
development and testing.
```

_Requirements: FR-12, US-6_
_Design: Section 4 (index.html), Section 5_

---

### Task 8: State Management Module

**Do:**
1. Create `src/state.rs` file
2. Define `AppState` struct with `Arc<Mutex<HashMap<DocumentId, Vec<(ClientId, mpsc::Sender)>>>>`
3. Implement `AppState::new()` constructor
4. Implement `join_room()` method that adds client to room
5. Implement `leave_room()` method that removes client and deletes empty rooms
6. Implement `broadcast_to_room()` method that sends message to all clients in room
7. Add println debugging statements for all operations
8. Use UNBOUNDED channels for POC (we'll fix to bounded in Phase 2)
9. Handle send failures in broadcast by collecting failed clients and cleaning them up

**Files:**
- `src/state.rs` (create)

**Done when:**
- [ ] AppState struct defined with rooms HashMap
- [ ] All three methods implemented (join_room, leave_room, broadcast_to_room)
- [ ] Empty rooms are deleted automatically
- [ ] Failed sends trigger cleanup
- [ ] Debug prints for all operations
- [ ] File compiles

**Verify:**
```bash
cargo build
```

**Commit:**
```
Add room-based state management with broadcast support

Implement the core room-based architecture using Arc<Mutex<HashMap>>
for thread-safe access across async tasks. This design enables:
- Multiple clients per document room
- Efficient O(1) room lookup by document_id
- Automatic cleanup of empty rooms to prevent memory leaks

Broadcast method handles failed sends gracefully by cleaning up
disconnected clients, preventing ghost subscriptions.
```

_Requirements: FR-2, FR-4, FR-5, FR-6, FR-7_
_Design: Section 4 (state.rs), Section 8_

---

### Task 9: Integrate State with WebSocket Handler

**Do:**
1. Update `src/main.rs` to:
   - Declare `mod state`
   - Create AppState instance
   - Add `.with_state(state)` to router
2. Update `src/websocket.rs` to:
   - Accept `State(state)` parameter in websocket_handler
   - Pass state to handle_socket
   - Track current_room as `Option<DocumentId>` in handle_socket
   - Parse incoming messages as ClientMessage
   - Handle Join message: call state.join_room(), update current_room, send Joined response
   - Handle SendMessage: call state.broadcast_to_room()
   - On disconnect: call state.leave_room()
3. Test with browser client

**Files:**
- `src/main.rs` (modify)
- `src/websocket.rs` (modify)

**Done when:**
- [ ] AppState created in main.rs and passed to router
- [ ] websocket_handler accepts State parameter
- [ ] Messages parsed as ClientMessage
- [ ] Join and SendMessage handled correctly
- [ ] Cleanup on disconnect works
- [ ] No compile errors

**Verify:**
```bash
cargo run
# Open browser, join a room, send message
# Message should echo back
```

**Commit:**
```
Integrate state management with message handlers

Wire up the WebSocket handler to use AppState for room management.
This completes the end-to-end message flow: client sends message →
server parses → broadcasts to all room members → clients receive.

Tracking current_room in handle_socket enables automatic cleanup on
disconnect, preventing clients from remaining in rooms after they leave.
```

_Requirements: FR-4, FR-5, FR-7_
_Design: Section 2, Section 8_

---

### Task 10: Multi-Client Broadcasting Test

**Do:**
1. Open 3 browser tabs to http://localhost:3030/
2. In all tabs, join the same document "demo-doc"
3. Verify each tab receives a Joined confirmation with unique client_id
4. Send a message from Tab 1
5. Verify message appears in all 3 tabs (including sender)
6. Send messages from Tab 2 and Tab 3
7. Verify all messages broadcast to all clients
8. Close Tab 1
9. Send message from Tab 2
10. Verify Tab 3 receives it but Tab 1 doesn't (because it's closed)

**Files:**
- None (manual testing)

**Done when:**
- [ ] 3 clients can join same room
- [ ] Messages broadcast to all clients in room
- [ ] Each client has unique client_id
- [ ] Sender receives their own messages
- [ ] Disconnected clients stop receiving messages

**Verify:**
```bash
cargo run
# Manual testing in browser
```

**Commit:**
No commit - this is a testing task

_Requirements: US-3, US-4, FR-5, FR-6_
_Design: Section 10 (Test Case 2)_

---

### Task 11: Room Isolation Test

**Do:**
1. Open 2 browser tabs
2. Tab 1: Join document "room-A"
3. Tab 2: Join document "room-B"
4. Tab 1: Send message "Secret A"
5. Tab 2: Send message "Secret B"
6. Verify Tab 1 only shows "Secret A"
7. Verify Tab 2 only shows "Secret B"
8. Verify NO cross-room message leakage
9. Check server logs to confirm rooms are separate

**Files:**
- None (manual testing)

**Done when:**
- [ ] Messages stay isolated to their rooms
- [ ] No cross-room leakage observed
- [ ] Server logs show two separate rooms

**Verify:**
```bash
cargo run
# Manual testing in browser
```

**Commit:**
No commit - this is a testing task

_Requirements: FR-6, NFR-5, US-3_
_Design: Section 10 (Test Case 3)_

---

## Phase 2: Refactoring

### Task 12: Extract WebSocketError Type

**Do:**
1. Add `WebSocketError` enum to `src/types.rs` with variants:
   - InvalidMessage(String)
   - NotInRoom
   - SendFailed
   - ConnectionError(String)
2. Implement Display trait for WebSocketError
3. Implement std::error::Error trait
4. Update `src/websocket.rs` to use WebSocketError instead of strings
5. Update error handling in handle_client_message to return Result<(), WebSocketError>

**Files:**
- `src/types.rs` (modify)
- `src/websocket.rs` (modify)

**Done when:**
- [ ] WebSocketError enum defined with all variants
- [ ] Display and Error traits implemented
- [ ] websocket.rs uses Result<(), WebSocketError>
- [ ] All error cases properly mapped
- [ ] Code compiles without errors

**Verify:**
```bash
cargo build
cargo run
# Test sending invalid JSON, verify error handling works
```

**Commit:**
```
Extract WebSocketError type for structured error handling

Replace string-based errors with a proper enum to enable type-safe
error handling and better error messages to clients.

This makes debugging easier and ensures all error cases are explicitly
handled, preventing unexpected panics that could crash the server.
```

_Requirements: FR-11, NFR-10_
_Design: Section 4 (types.rs), Section 7_

---

### Task 13: Replace Unbounded Channels with Bounded

**Do:**
1. In `src/websocket.rs`, change `mpsc::unbounded_channel()` to `mpsc::channel(100)`
2. Update `src/state.rs` to use `mpsc::Sender` instead of `mpsc::UnboundedSender`
3. Change sends to use `.send().await` instead of `.send()` (now async)
4. Handle send timeout using `tokio::time::timeout(Duration::from_secs(5), tx.send(msg))`
5. On timeout, treat as send failure and mark client for cleanup
6. Update type aliases in state.rs

**Files:**
- `src/websocket.rs` (modify)
- `src/state.rs` (modify)

**Done when:**
- [ ] All channels bounded to 100 messages
- [ ] Send operations handle timeouts
- [ ] Slow clients disconnected on timeout
- [ ] Type signatures updated
- [ ] Code compiles and runs

**Verify:**
```bash
cargo build
cargo run
# Test with normal clients, verify no breakage
```

**Commit:**
```
Replace unbounded channels with bounded channels for backpressure control

Switch from unbounded to bounded channels (buffer=100) to prevent memory
exhaustion when slow clients can't keep up with message volume.

Research showed this is critical for production systems - unbounded channels
can cause OOM crashes if a slow client creates an ever-growing message queue.
The 5-second timeout ensures we disconnect slow clients gracefully rather
than accumulating unbounded memory.
```

_Requirements: FR-10, NFR-3_
_Design: Section 7, Section 8_

---

### Task 14: Improve Error Handling and Logging

**Do:**
1. In `src/websocket.rs`, replace all `println!` with structured prints showing:
   - Timestamp (use `chrono` or just print duration since start)
   - Client ID
   - Event type
   - Details
2. Add error handling for all `.unwrap()` calls (if any remain)
3. In handle_client_message, send ServerMessage::Error on failures
4. Test malformed JSON input, verify error response sent to client
5. Ensure server never panics on client errors

**Files:**
- `src/websocket.rs` (modify)
- `src/state.rs` (modify)

**Done when:**
- [ ] All println! statements include client_id context
- [ ] No unwrap() calls in request handlers
- [ ] Malformed messages return error to client
- [ ] Server continues serving other clients on errors
- [ ] Manual test with bad JSON succeeds

**Verify:**
```bash
cargo run
# In browser console, send: ws.send("invalid json{")
# Verify server doesn't panic and sends error response
```

**Commit:**
```
Improve error handling and logging for robustness

Add structured logging with client_id context and ensure server never
panics on client errors, allowing it to continue serving other clients.

Malformed messages now return explicit error responses to clients,
making the system more debuggable and resilient to bad input.
```

_Requirements: FR-11, NFR-10_
_Design: Section 7_

---

### Task 15: Client Reconnection with Exponential Backoff

**Do:**
1. Update `static/index.html` JavaScript to add reconnection logic
2. Implement exponential backoff: base 500ms, max 30s
3. Add jitter: random 0-1000ms added to delay
4. Track reconnection attempts
5. On close, automatically schedule reconnect
6. On successful reconnect, reset attempt counter
7. Re-join previous room automatically on reconnect
8. Update status indicator during reconnection attempts

**Files:**
- `static/index.html` (modify)

**Done when:**
- [ ] Client reconnects automatically on disconnect
- [ ] Backoff increases exponentially (500ms, 1s, 2s, 4s...)
- [ ] Jitter randomizes reconnection timing
- [ ] Max delay capped at 30s
- [ ] Previous room automatically rejoined
- [ ] Status shows "Reconnecting..." during attempts

**Verify:**
```bash
cargo run
# Open browser, join room
# Stop server (Ctrl+C)
# Observe reconnection attempts in console
# Restart server
# Verify auto-reconnect and room rejoin
```

**Commit:**
```
Implement exponential backoff reconnection with jitter

Add automatic reconnection that increases delays exponentially (500ms → 30s)
with random jitter to prevent thundering herd when server restarts.

Without this, all disconnected clients would reconnect simultaneously,
potentially overwhelming the server. Jitter spreads reconnections over time
for graceful recovery.
```

_Requirements: FR-13, US-7, NFR-6_
_Design: Section 2, Section 10 (Test Case 4)_

---

### Task 16: Message Display Formatting

**Do:**
1. Update `static/index.html` to format messages nicely
2. For each message, display:
   - Timestamp in HH:MM:SS format (convert from Unix millis)
   - Truncated client_id (first 8 characters)
   - Message text
3. Style own messages differently (e.g., blue background)
4. Style other messages with grey background
5. Add auto-scroll: message list scrolls to bottom when new message arrives
6. Keep messages in a scrollable div with max height

**Files:**
- `static/index.html` (modify)

**Done when:**
- [ ] Messages show timestamp, client_id (truncated), and text
- [ ] Own messages visually distinct from others
- [ ] Auto-scroll to bottom on new message
- [ ] Message area scrollable with reasonable height
- [ ] Clean, readable formatting

**Verify:**
```bash
cargo run
# Open browser, send messages
# Verify formatting looks good
# Send many messages, verify auto-scroll works
```

**Commit:**
```
Add message formatting with timestamps and auto-scroll

Format messages with HH:MM:SS timestamps and truncated client IDs for
better readability. Visually distinguish own messages from others.

Auto-scroll keeps latest messages visible, important for following
real-time conversations in multi-client scenarios.
```

_Requirements: FR-14, FR-15_
_Design: Section 4 (index.html)_

---

## Phase 3: Testing

### Task 17: Unit Tests for State Management

**Do:**
1. Add `#[cfg(test)]` module to `src/state.rs`
2. Write test: `test_join_room()` - verify client added to room
3. Write test: `test_leave_room()` - verify client removed
4. Write test: `test_empty_room_deleted()` - verify empty rooms are deleted
5. Write test: `test_broadcast_to_multiple_clients()` - verify all clients receive message
6. Write test: `test_room_isolation()` - verify messages don't leak between rooms
7. Add `#[cfg(test)]` helper method to AppState: `room_client_count(&self, doc_id) -> usize`

**Files:**
- `src/state.rs` (modify)

**Done when:**
- [ ] 5 unit tests written and passing
- [ ] Tests use `#[tokio::test]` macro
- [ ] Helper method added for test introspection
- [ ] All tests pass with `cargo test`

**Verify:**
```bash
cargo test
# All tests should pass
```

**Commit:**
```
Add unit tests for room management and broadcasting

Test core state management functions to ensure rooms are created,
cleaned up, and isolated correctly.

These tests catch race conditions and memory leaks early, which are
difficult to debug in manual testing. The test helper methods enable
introspection of internal state.
```

_Requirements: NFR-8_
_Design: Section 9 (Unit Tests)_

---

### Task 18: Unit Tests for Message Parsing

**Do:**
1. Add `#[cfg(test)]` module to `src/types.rs`
2. Write test: `test_parse_join_message()` - verify JSON deserialization
3. Write test: `test_parse_send_message()` - verify JSON deserialization
4. Write test: `test_serialize_joined_message()` - verify JSON serialization
5. Write test: `test_serialize_message_broadcast()` - verify timestamp included
6. Write test: `test_invalid_json_fails()` - verify parse error on bad JSON

**Files:**
- `src/types.rs` (modify)

**Done when:**
- [ ] 5 unit tests written and passing
- [ ] Tests cover both serialization and deserialization
- [ ] Error cases tested
- [ ] All tests pass

**Verify:**
```bash
cargo test
```

**Commit:**
```
Add unit tests for message protocol serialization

Verify JSON protocol serialization/deserialization works correctly
for all message types.

These tests ensure protocol compatibility and catch breaking changes
to message formats that would cause client-server communication failures.
```

_Requirements: NFR-8_
_Design: Section 9 (Protocol Parsing Tests)_

---

### Task 19: Integration Test - Multi-Client Same Room

**Do:**
1. Create `tests/integration_test.rs` file
2. Add helper function to start server in background
3. Add helper function to connect WebSocket client
4. Write test: `test_multi_client_same_room()`
   - Start server
   - Connect 2 clients
   - Both join "test-doc"
   - Client1 sends message
   - Verify both clients receive it
   - Cleanup clients and server
5. Use tokio-tungstenite or Axum's test client

**Files:**
- `tests/integration_test.rs` (create)
- `Cargo.toml` (modify - add dev-dependencies if needed)

**Done when:**
- [ ] Integration test file exists
- [ ] Test spawns real server
- [ ] Test connects real WebSocket clients
- [ ] Test passes with `cargo test`
- [ ] Server and clients properly cleaned up

**Verify:**
```bash
cargo test test_multi_client_same_room
```

**Commit:**
```
Add multi-client same room integration test

Test real WebSocket connections with multiple clients to verify
broadcasting works correctly in same-room scenarios.

Integration tests catch issues that unit tests miss, like message
ordering, concurrent access, and actual network behavior.
```

_Requirements: NFR-8, US-3, US-4_
_Design: Section 9 (Integration Tests)_

---

### Task 20: Integration Test - Room Isolation

**Do:**
1. Add to `tests/integration_test.rs`
2. Write test: `test_room_isolation()`
   - Start server
   - Connect 2 clients
   - Client1 joins "doc1"
   - Client2 joins "doc2"
   - Client1 sends message "secret1"
   - Use timeout to verify Client2 does NOT receive it
   - Client2 sends message "secret2"
   - Verify Client1 does NOT receive it
   - Cleanup

**Files:**
- `tests/integration_test.rs` (modify)

**Done when:**
- [ ] Test verifies no cross-room leakage
- [ ] Uses tokio::time::timeout for negative assertions
- [ ] Test passes
- [ ] No flaky behavior

**Verify:**
```bash
cargo test test_room_isolation
```

**Commit:**
```
Add room isolation integration test

Verify that messages never leak between different document rooms,
which is critical for data privacy and correctness.

Uses timeouts for negative assertions to ensure messages truly don't
appear in other rooms, not just delayed delivery.
```

_Requirements: NFR-5, FR-6_
_Design: Section 9 (Integration Tests)_

---

### Task 21: Manual Testing - All 4 Demo Scenarios

**Do:**
1. Run through all 4 scenarios from requirements.md Success Criteria:
   - **Scenario 1**: Single client echo (verify message appears <100ms)
   - **Scenario 2**: Multi-client same document (3 tabs, all see all messages)
   - **Scenario 3**: Different documents isolation (2 tabs, no leakage)
   - **Scenario 4**: Reconnection handling (stop/restart server, auto-reconnect)
2. Document any failures or issues
3. Take screenshots of successful tests
4. Verify status indicator updates correctly
5. Verify client_id displayed correctly
6. Test with 10+ concurrent clients

**Files:**
- None (manual testing)

**Done when:**
- [ ] All 4 demo scenarios pass
- [ ] Status indicator works correctly
- [ ] Client IDs displayed properly
- [ ] 10+ clients tested successfully
- [ ] No errors or crashes observed

**Verify:**
```bash
cargo run
# Manual testing following scenarios
```

**Commit:**
No commit - this is a testing/validation task

_Requirements: All P0 user stories, Success Criteria_
_Design: Section 10 (Manual Testing Procedures)_

---

## Phase 4: Quality Gates

### Task 22: Code Quality and Documentation

**Do:**
1. Run `cargo clippy` and fix all warnings
2. Run `cargo fmt` to format all code
3. Add module-level doc comments to each file explaining purpose
4. Add doc comments to public functions
5. Add inline comments for non-obvious code sections
6. Review CLAUDE.md rules - ensure no unnecessary comments on obvious code
7. Verify all FR-* requirements marked complete
8. Verify all P0 and P1 requirements satisfied
9. Update project README if needed (optional)

**Files:**
- `src/main.rs` (modify)
- `src/types.rs` (modify)
- `src/state.rs` (modify)
- `src/websocket.rs` (modify)

**Done when:**
- [ ] `cargo clippy` passes with no warnings
- [ ] `cargo fmt --check` passes
- [ ] All modules have doc comments
- [ ] Public functions documented
- [ ] Code is clean and readable
- [ ] Comments explain "why" not "what"

**Verify:**
```bash
cargo clippy
cargo fmt -- --check
cargo test
cargo run
```

**Commit:**
```
Run clippy and fmt, add documentation for public APIs

Apply Rust best practices with clippy lints and consistent formatting.
Add documentation explaining why design decisions were made, not just
what the code does.

Clean, well-documented code is essential for maintainability and helps
future developers understand the architectural choices.
```

_Requirements: NFR-7, Success Criteria_
_Design: Section 10_

---

## Summary

**Total Tasks:** 22
- Phase 1 (POC): 11 tasks
- Phase 2 (Refactoring): 5 tasks
- Phase 3 (Testing): 5 tasks
- Phase 4 (Quality): 1 task

**Estimated Timeline:**
- Phase 1: 6-8 hours (get something working end-to-end)
- Phase 2: 3-4 hours (clean up and improve)
- Phase 3: 3-4 hours (comprehensive testing)
- Phase 4: 1 hour (polish)
- **Total: ~16 hours** over 2-3 days

**Key Milestones:**
- After Task 11: Basic POC demo-able
- After Task 16: Feature-complete with reconnection and formatting
- After Task 21: Fully tested and validated
- After Task 22: Production-ready code

**Requirements Coverage:**
- All P0 requirements (FR-1 through FR-9, FR-12)
- All P1 requirements (FR-10, FR-11, FR-13)
- All P2 requirements (FR-14, FR-15)
- All P0 user stories (US-1 through US-6)
- All P1 user stories (US-5, US-7)
- All NFRs addressed in implementation and testing

**Next Step:** Begin with Task 1 to set up dependencies and project structure.
