---
spec: websocket-setup
phase: requirements
created: 2026-01-14T21:59:00Z
---

# Requirements: websocket-setup

## User Stories

### US-1: WebSocket Server Connection
**As a** developer
**I want** a WebSocket server running on port 3030
**So that** browser clients can establish persistent bidirectional connections

**Acceptance Criteria:**
- [ ] Given the server is running when a client connects to `ws://localhost:3030/ws` then the WebSocket upgrade succeeds
- [ ] Given a connection is established when the server crashes then clients detect the disconnection
- [ ] Given multiple clients connecting when they connect simultaneously then all connections are accepted without race conditions

**Priority:** P0

### US-2: Document Room Joining
**As a** client
**I want** to join a specific document room by ID
**So that** I only receive messages relevant to my document

**Acceptance Criteria:**
- [ ] Given a WebSocket connection when I send a join message with `document_id` then I receive a confirmation with my assigned `client_id`
- [ ] Given I have joined a room when I send messages then only clients in the same room receive them
- [ ] Given I disconnect when my connection drops then I am automatically removed from the room

**Priority:** P0

### US-3: Real-Time Message Broadcasting
**As a** client in a document room
**I want** messages sent by any client to be broadcast to all clients in the same room
**So that** I can see real-time updates from other users

**Acceptance Criteria:**
- [ ] Given multiple clients in the same room when one client sends a message then all other clients receive it within 100ms
- [ ] Given clients in different rooms when a message is sent then only clients in the sender's room receive it
- [ ] Given a message is broadcast when it includes sender metadata then recipients can identify who sent it

**Priority:** P0

### US-4: Multi-Client Real-Time Streaming
**As a** user with multiple browser tabs open
**I want** to see messages from all tabs in the same document
**So that** I can test and verify multi-client synchronization

**Acceptance Criteria:**
- [ ] Given 3+ browser tabs connected to the same document when I send a message from one tab then all other tabs display it immediately
- [ ] Given tabs are connected to different documents when messages are sent then rooms remain isolated
- [ ] Given 10+ concurrent clients when messages are sent rapidly then all clients receive all messages in order

**Priority:** P0

### US-5: Connection Lifecycle Management
**As a** system administrator
**I want** proper cleanup when clients disconnect
**So that** server resources are not leaked over time

**Acceptance Criteria:**
- [ ] Given a client disconnects when the connection closes then the client is removed from room state within 1 second
- [ ] Given a room is empty when the last client leaves then the room is deleted from state
- [ ] Given the server runs for 24 hours when many clients connect and disconnect then memory usage remains stable

**Priority:** P1

### US-6: Static HTML Client Access
**As a** user
**I want** to access the WebSocket client via a web browser
**So that** I can interact with the system without installing software

**Acceptance Criteria:**
- [ ] Given the server is running when I navigate to `http://localhost:3030/` then I see the HTML client interface
- [ ] Given the HTML page loads when I interact with it then WebSocket connection establishes automatically
- [ ] Given the page is loaded when I refresh it then connection re-establishes successfully

**Priority:** P0

### US-7: Client Reconnection Handling
**As a** client
**I want** automatic reconnection with exponential backoff
**So that** temporary network issues don't require manual intervention

**Acceptance Criteria:**
- [ ] Given a connection drops when the network is restored then the client reconnects automatically
- [ ] Given multiple failed attempts when reconnecting then backoff delay increases exponentially (500ms, 1s, 2s, 4s...)
- [ ] Given many clients reconnecting when the server restarts then reconnection attempts are staggered with jitter

**Priority:** P1

### US-8: Connection Status Visibility
**As a** user
**I want** to see my connection status in the UI
**So that** I know whether my messages will be delivered

**Acceptance Criteria:**
- [ ] Given the WebSocket is connected when I view the page then status shows "Connected ✓"
- [ ] Given the connection drops when disconnected then status shows "Disconnected ✗"
- [ ] Given I join a room when confirmed then my assigned `client_id` is displayed

**Priority:** P2

## Functional Requirements

### FR-1: WebSocket Server Implementation
**Description:** Implement a WebSocket server using Axum framework with `axum::extract::ws` that listens on `0.0.0.0:3030` and accepts WebSocket upgrade requests at the `/ws` endpoint.

**Priority:** P0
**Dependencies:** None

### FR-2: Room-Based State Management
**Description:** Maintain an in-memory state structure `Arc<Mutex<HashMap<DocumentId, Vec<(ClientId, mpsc::Sender<String>)>>>>` to track which clients are in which document rooms.

**Priority:** P0
**Dependencies:** FR-1

### FR-3: Client ID Generation
**Description:** Generate unique client identifiers using UUID v4 for each WebSocket connection upon establishment.

**Priority:** P0
**Dependencies:** FR-1

### FR-4: Join Message Handling
**Description:** Process join messages from clients containing `document_id`, add the client to the specified room, and send back a confirmation message with the assigned `client_id`.

**Priority:** P0
**Dependencies:** FR-1, FR-2, FR-3

### FR-5: Message Broadcasting
**Description:** When a client sends a message, broadcast it to all other clients in the same document room with metadata including sender `client_id` and timestamp.

**Priority:** P0
**Dependencies:** FR-2, FR-4

### FR-6: Room Isolation
**Description:** Ensure that messages sent in one document room are never delivered to clients in other document rooms.

**Priority:** P0
**Dependencies:** FR-2, FR-5

### FR-7: Connection Cleanup
**Description:** Implement automatic cleanup when clients disconnect, removing them from room state and deleting empty rooms using RAII patterns (Drop trait).

**Priority:** P0
**Dependencies:** FR-2

### FR-8: Static File Serving
**Description:** Serve static HTML files from the `static/` directory using `tower_http::services::ServeDir`, making the client interface available at `http://localhost:3030/`.

**Priority:** P0
**Dependencies:** None

### FR-9: JSON Protocol Serialization
**Description:** Define and implement JSON message protocol for client-server communication with message types: `join`, `send_message` (client→server) and `joined`, `message`, `error` (server→client).

**Priority:** P0
**Dependencies:** FR-1

### FR-10: Bounded Channel Communication
**Description:** Use bounded `mpsc::channel` (not unbounded) with a reasonable buffer size (e.g., 100 messages) for client communication to prevent backpressure and memory leaks.

**Priority:** P1
**Dependencies:** FR-2

### FR-11: Error Handling
**Description:** Implement graceful error handling for malformed messages, failed sends, and disconnections without panicking. Log errors for debugging while continuing to serve other clients.

**Priority:** P1
**Dependencies:** FR-1

### FR-12: Split-Screen HTML Client
**Description:** Create an HTML client with a split-screen layout: left pane for message input (40% width), right pane for message stream display (60% width).

**Priority:** P0
**Dependencies:** FR-8

### FR-13: Client Reconnection Logic
**Description:** Implement client-side exponential backoff reconnection with jitter (base 500ms, max 30s) when WebSocket connection drops.

**Priority:** P1
**Dependencies:** FR-12

### FR-14: Message Display Formatting
**Description:** Display messages in the right pane with timestamp (HH:MM:SS), truncated client ID (first 8 chars), and message text. Differentiate own messages from others visually.

**Priority:** P2
**Dependencies:** FR-12

### FR-15: Auto-Scroll Behavior
**Description:** Automatically scroll the message list to the bottom when new messages arrive to keep the latest messages visible.

**Priority:** P2
**Dependencies:** FR-12, FR-14

## Non-Functional Requirements

### NFR-1: Concurrent Connection Capacity
**Description:** The server must support at least 100 concurrent WebSocket connections without degradation in message delivery latency.

**Metric:** 100+ simultaneous connections with <100ms message broadcast latency

### NFR-2: Message Broadcast Latency
**Description:** Messages must be broadcast to all clients in a room within 100ms of the server receiving them under normal load.

**Metric:** P95 latency <100ms for rooms with <20 clients

### NFR-3: Memory Stability
**Description:** Server memory usage must remain stable over 24+ hours of operation with continuous client connects/disconnects.

**Metric:** Memory growth <5% over 24 hours with 1000+ connection cycles

### NFR-4: Connection Cleanup Reliability
**Description:** Client state must be cleaned up reliably within 1 second of disconnection to prevent resource leaks.

**Metric:** 100% of disconnections result in cleanup within 1s

### NFR-5: Room Isolation Guarantee
**Description:** Messages must never leak between document rooms, ensuring complete isolation of room-based communication.

**Metric:** 0 cross-room message leaks in 10,000+ message test

### NFR-6: Client Reconnection Resilience
**Description:** Clients must successfully reconnect within 30 seconds of server recovery after a crash with exponential backoff preventing thundering herd.

**Metric:** 95% of clients reconnect within 30s of server restart

### NFR-7: Code Maintainability
**Description:** Code must be organized into logical modules (websocket.rs, state.rs, main.rs) with clear separation of concerns and comprehensive inline documentation.

**Metric:** Code review approval, <500 lines per module

### NFR-8: Test Coverage
**Description:** Core functionality must have unit and integration tests covering state management, room logic, and multi-client scenarios.

**Metric:** >80% line coverage for core modules, integration tests for all user stories

### NFR-9: Browser Compatibility
**Description:** HTML client must work in modern browsers (Chrome 90+, Firefox 88+, Safari 14+, Edge 90+).

**Metric:** Manual testing on 3+ browsers shows full functionality

### NFR-10: Error Recovery
**Description:** System must gracefully handle and recover from individual client errors (malformed messages, slow clients) without affecting other clients.

**Metric:** Single client error does not impact other clients in 100 test runs

## Technical Constraints

### TC-1: Programming Language and Framework
- **Language:** Rust (edition 2021)
- **Web Framework:** Axum 0.7+ with WebSocket support (`axum::extract::ws`)
- **Async Runtime:** Tokio 1.x with "full" features

### TC-2: Single-Process Architecture
- All state maintained in-process memory using `Arc<Mutex<HashMap>>`
- No distributed state management (Redis, etc.) in this phase
- State lost on server restart (acceptable for Phase 1)

### TC-3: No Database Persistence
- No PostgreSQL or SQLite integration in this phase
- All data ephemeral in-memory only
- Database integration deferred to Phase 2

### TC-4: Browser-Based Client
- Client implemented as HTML/JavaScript/CSS served as static files
- No native desktop or mobile applications
- Must work without build tools (vanilla JS, no webpack/vite)

### TC-5: Dependencies
Required Rust crates:
- `tokio` = { version = "1", features = ["full"] }
- `axum` = { version = "0.7", features = ["ws"] }
- `tower-http` = { version = "0.6", features = ["fs"] }
- `serde` = { version = "1.0", features = ["derive"] }
- `serde_json` = "1.0"
- `uuid` = { version = "1.0", features = ["v4"] }

### TC-6: Network Configuration
- Server binds to `0.0.0.0:3030`
- WebSocket endpoint at `/ws`
- Static files served from root `/`
- No TLS/HTTPS in Phase 1 (plain WebSocket)

## Success Criteria

### Functional Completeness
- [ ] All P0 functional requirements implemented and tested
- [ ] All P0 user stories have passing acceptance criteria
- [ ] Demo scenarios from WEBSOCKET_DEMO_PLAN.md work flawlessly

### Demo Scenarios
1. **Single Client Echo:**
   - Open browser to `http://localhost:3030/`
   - Send message "Hello World"
   - See message appear in right pane with own client_id
   - **Success:** Message echoes back within 100ms

2. **Multi-Client Same Document:**
   - Open 3 browser tabs with `document_id="demo-doc"`
   - Send message from Tab 1: "Message from Tab 1"
   - **Success:** All 3 tabs show the message with sender's client_id

3. **Multi-Client Different Documents:**
   - Open 2 tabs: Tab 1 with `document_id="doc1"`, Tab 2 with `document_id="doc2"`
   - Send message in Tab 1: "Should not appear in doc2"
   - **Success:** Tab 2 shows no messages, Tab 1 shows the message

4. **Reconnection Handling:**
   - Open browser tab, connect successfully
   - Stop server with Ctrl+C
   - Observe "Disconnected ✗" status
   - Restart server
   - **Success:** Client reconnects automatically within 5 seconds

### Performance Metrics
- [ ] 10+ concurrent clients in same room with <100ms latency
- [ ] 100+ messages broadcast in 1 minute without errors
- [ ] Server memory stable after 1000+ connect/disconnect cycles

### Code Quality
- [ ] Passes `cargo clippy` with no warnings
- [ ] Formatted with `cargo fmt`
- [ ] Core modules have unit tests
- [ ] At least one integration test per user story
- [ ] No panics or crashes during normal operation

## Out of Scope

### Phase 2: Deferred to Cache Integration
- **Query subscription management:** Tracking which clients subscribe to which queries
- **Cache module integration:** Connecting WebSocket to the Cache invalidation system
- **Inverted index:** Mapping cache keys to subscribed clients
- **Query result delivery:** Sending computed graph results instead of simple text messages

### Future Phases: Advanced Features
- **Database persistence:** Storing messages or connection history in PostgreSQL
- **Authentication:** User login, JWT tokens, access control
- **Redis pub/sub:** Distributed state management for horizontal scaling
- **Load balancing:** Multiple server instances with sticky sessions
- **TLS/HTTPS:** Secure WebSocket (wss://) connections
- **Compression:** Per-message deflate for bandwidth reduction
- **Rate limiting:** Preventing message spam or DoS attacks
- **Metrics/Monitoring:** Prometheus metrics, distributed tracing
- **Advanced conflict resolution:** Operational Transform (OT) or CRDTs
- **Presence tracking:** Online/offline status, typing indicators
- **Message history:** Retrieving past messages on reconnect
- **Binary protocols:** Protocol Buffers or MessagePack instead of JSON

### Explicitly Not Included
- **Mobile native apps:** iOS/Android clients (browser only for now)
- **File/image uploads:** Text messages only
- **Voice/video:** WebRTC integration
- **End-to-end encryption:** Client-side encryption
- **Desktop application:** Electron or Tauri wrapper

## Glossary

### document_id
A string identifier representing a logical document or room. Clients with the same `document_id` can communicate with each other. Example: `"demo-doc"`, `"project-123"`.

### client_id
A unique identifier (UUID v4) assigned to each WebSocket connection by the server. Used to identify message senders. Example: `"a1b2c3d4-e5f6-7890-1234-567890abcdef"`.

### room
A logical grouping of clients based on `document_id`. All clients in a room receive broadcasts sent by any member of that room.

### broadcast
The act of sending a message from one client to all other clients in the same room.

### WebSocket upgrade
The HTTP protocol handshake that transitions a connection from HTTP to the WebSocket protocol (RFC 6455).

### backpressure
The phenomenon where a slow consumer (client) cannot keep up with a fast producer (server), causing messages to queue and potentially exhaust memory.

### bounded channel
A message queue with a fixed maximum size. Once full, attempts to send fail or block, preventing unbounded memory growth.

### exponential backoff
A reconnection strategy where retry delays double after each failure (500ms, 1s, 2s, 4s...) to prevent overwhelming the server.

### jitter
Random delay added to backoff intervals to prevent synchronized reconnection attempts (thundering herd).

### RAII (Resource Acquisition Is Initialization)
A Rust pattern using the `Drop` trait to guarantee cleanup code runs when values go out of scope, preventing resource leaks.

### split-screen UI
The HTML client layout with two panes: left for input (40% width), right for message stream (60% width).

### P0/P1/P2 Priority
- **P0:** Critical, must-have for Phase 1 completion
- **P1:** Important, should-have for production readiness
- **P2:** Nice-to-have, enhances user experience

## Dependencies and Risks

### Dependencies

#### External Dependencies
- **Rust toolchain:** Requires Rust 1.70+ for edition 2021 and async features
- **Tokio runtime:** Foundation for async operations; mature and stable
- **Axum framework:** Active development, v0.7+ API stable
- **Browser WebSocket API:** Standard across modern browsers, no polyfills needed

#### Internal Dependencies
- **File structure:** Must create `static/` directory for HTML client
- **Port availability:** Port 3030 must be available on localhost
- **Network access:** Clients must be able to reach server (same machine for demo)

#### Logical Dependencies
- FR-4 (Join Handling) depends on FR-2 (State) and FR-3 (Client ID)
- FR-5 (Broadcasting) depends on FR-2 (State) and FR-4 (Join)
- US-3 (Broadcasting) depends on US-2 (Room Joining)
- All client features depend on FR-8 (Static File Serving)

### Risks

#### Risk 1: Backpressure Memory Leaks
**Description:** Using unbounded channels could cause memory exhaustion if slow clients can't keep up with message volume.

**Impact:** High - Server crashes with OOM errors

**Mitigation:**
- Use bounded channels with 100-message buffer (FR-10)
- Implement send timeouts and close slow clients
- Monitor channel depth in logs
- Test with deliberately slow clients

**Status:** Mitigated by research recommendations

#### Risk 2: Race Conditions in Room State
**Description:** Concurrent join/leave operations could cause inconsistent state, duplicate entries, or missed cleanup.

**Impact:** Medium - Clients don't receive messages or rooms leak memory

**Mitigation:**
- Use `Arc<Mutex<>>` for synchronized access
- Minimize time holding locks
- Comprehensive unit tests for concurrent operations
- Integration tests with 10+ simultaneous clients

**Status:** Addressable with careful implementation

#### Risk 3: WebSocket Browser Compatibility
**Description:** Older browsers or restrictive environments may not support WebSocket API.

**Impact:** Low - Demo targets modern browsers only

**Mitigation:**
- Document minimum browser versions (NFR-9)
- Test on Chrome, Firefox, Safari during development
- Display clear error if WebSocket unavailable

**Status:** Low risk for Phase 1 demo

#### Risk 4: Connection Cleanup Failures
**Description:** Clients may not be removed from state on disconnect, causing memory leaks and ghost subscriptions.

**Impact:** High - Long-running server becomes unstable

**Mitigation:**
- RAII pattern with Drop trait (FR-7)
- Explicit cleanup in disconnect handlers
- Integration tests for 1000+ connect/disconnect cycles
- Monitor memory usage metrics

**Status:** Mitigated by Rust's ownership model

#### Risk 5: Testing Difficulty for Multi-Client Scenarios
**Description:** Hard to reproduce timing-dependent bugs and race conditions in tests.

**Impact:** Medium - Bugs may only appear in production

**Mitigation:**
- Use `tokio::test` with multi-threaded runtime
- Property-based testing with random operation sequences
- Load testing with 100+ concurrent clients
- Detailed logging for post-mortem analysis

**Status:** Requires proactive test development

#### Risk 6: Thundering Herd on Server Restart
**Description:** If all clients reconnect simultaneously after server restart, the spike in connection requests could overwhelm the server.

**Impact:** Medium - Server slow to recover or crashes again

**Mitigation:**
- Implement exponential backoff with jitter in client (FR-13)
- Test restart scenario with 50+ connected clients
- Document expected recovery time

**Status:** Mitigated by FR-13 requirement

#### Risk 7: JSON Serialization Performance
**Description:** JSON parsing overhead may impact latency at high message volumes.

**Impact:** Low - Not expected to be bottleneck for Phase 1

**Mitigation:**
- Benchmark message serialization/deserialization
- Consider binary protocols (MessagePack, Protocol Buffers) in Phase 2 if needed
- Profile server under load to identify bottlenecks

**Status:** Accept risk for Phase 1, monitor performance

### Risk Matrix

| Risk | Probability | Impact | Mitigation Status | Priority |
|------|-------------|--------|-------------------|----------|
| Backpressure Memory Leaks | Medium | High | Mitigated | P0 |
| Race Conditions | Medium | Medium | Addressable | P0 |
| Browser Compatibility | Low | Low | Accepted | P2 |
| Cleanup Failures | Low | High | Mitigated | P0 |
| Testing Difficulty | High | Medium | Requires Work | P1 |
| Thundering Herd | Medium | Medium | Mitigated | P1 |
| JSON Performance | Low | Low | Accepted | P2 |

---

## Next Phase: Design

After requirements approval, proceed to technical design phase to specify:
- Detailed module architecture and APIs
- Data structure definitions with lifetimes and traits
- WebSocket message protocol schemas
- State management synchronization patterns
- Error handling strategy and error types
- Testing plan with specific test cases
- Deployment and operational procedures

See: `./specs/websocket-setup/design.md` (to be created)
