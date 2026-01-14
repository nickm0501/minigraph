# WebSocket Setup Research

## Executive Summary

This document provides comprehensive research findings for implementing a WebSocket-based real-time communication system for the mini-graph project. The research covers Rust WebSocket libraries, implementation patterns, multi-client synchronization strategies, and potential challenges.

**Key Recommendations:**
- Use `tokio` + `axum` for the WebSocket server (Axum provides built-in WebSocket support)
- Implement room-based broadcasting with `Arc<Mutex<HashMap>>` for state management
- Use `tokio::sync::mpsc` channels for individual client communication
- Apply exponential backoff with jitter for client reconnection
- Serve static HTML files using `tower_http::services::ServeDir`

## 1. Technology Stack Assessment

### 1.1 WebSocket Libraries for Rust

#### Primary Recommendation: Axum with Built-in WebSocket Support

**Axum** (with `axum::extract::ws`) is the recommended choice for this project because:

- **Integrated WebSocket support**: Axum already supports WebSockets through `axum::extract::ws`, eliminating the need for separate WebSocket libraries
- **Tower ecosystem integration**: Seamlessly integrates with Tower services for middleware and static file serving
- **Built on tokio-tungstenite**: Uses tungstenite under the hood (a private implementation detail) while providing a cleaner API
- **Static file serving**: Easy integration with `tower_http::services::ServeDir` for serving HTML clients
- **Active development**: Well-maintained and widely adopted in the Rust community

#### Alternative: tokio-tungstenite

If you need direct access to tungstenite features:

- **Maturity**: Most widely adopted async WebSocket library in Rust
- **Performance**: Recent versions (> 0.26.2) are performant and on par with fastwebsockets
- **Tokio integration**: Provides Tokio bindings for non-blocking/asynchronous TcpStreams
- **TLS support**: Supports native-tls or rustls through feature flags
- **RFC6455 compliance**: Tested with Autobahn TestSuite for WebSocket standard compliance

**Performance benchmarks** (recent comparisons):
- wtx: 6350.31 ms (fastest)
- tokio-tungstenite: 7602.94 ms
- uWebSockets: 8393.94 ms
- fastwebsockets: 10140.58 ms

For this learning project, the performance differences are negligible. Choose based on API ergonomics and ecosystem fit.

#### Other Alternatives

**fastwebsockets:**
- Performance-focused implementation
- Different frame handling approach (gives raw frames with FIN set)
- Good for high-throughput scenarios
- Has Axum integration support

**socketioxide:**
- Socket.IO protocol server implementation
- Works as a Tower Service with Axum, Hyper, and WebSocket
- Use if Socket.IO protocol compatibility is required

**Recommendation for mini-graph**: Use **Axum's built-in WebSocket support** (`axum::extract::ws`) for simplicity and ecosystem integration. It provides everything needed for this project without additional dependencies.

### 1.2 Static File Serving

**Recommended approach**: Use `tower_http::services::ServeDir` with Axum

```rust
use tower_http::services::ServeDir;
use axum::{Router, routing::get};

let app = Router::new()
    .route("/ws", get(websocket_handler))
    .nest_service("/", ServeDir::new("static"));
```

**Key features:**
- Implements `tower::Service` for flexible integration
- Can be used with `.nest_service()` on the Axum router
- Graceful error handling
- Can serve both directory trees and individual files

**Dependencies needed:**
```toml
tower-http = { version = "0.6", features = ["fs"] }
```

### 1.3 Async Runtime

**tokio** is the de facto standard for async Rust applications:

```toml
tokio = { version = "1", features = ["full"] }
```

**Key capabilities for WebSocket applications:**
- Event loop architecture minimizing context switching
- Handle thousands of concurrent connections on a single thread
- Built-in utilities: `tokio::sync::mpsc`, `tokio::sync::broadcast`, `tokio::time::interval`
- Comprehensive testing support with `tokio::test`

## 2. WebSocket Implementation Best Practices

### 2.1 Connection Management Patterns

#### Room-Based Architecture

The recommended pattern for document-based rooms:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

type ClientId = String;
type DocumentId = String;

struct AppState {
    // document_id -> list of (client_id, sender)
    rooms: Arc<Mutex<HashMap<DocumentId, Vec<(ClientId, mpsc::UnboundedSender<String>)>>>>,
}
```

**Key design decisions:**
- **`Arc<Mutex<HashMap>>`**: Shared mutable state across async tasks
- **`mpsc::UnboundedSender`**: Individual channels for each client
- **Room isolation**: Messages only broadcast within the same document room

#### Connection Lifecycle

1. **Connection establishment**:
   - Accept WebSocket upgrade
   - Generate unique `client_id` (UUID)
   - Wait for `Join` message with `document_id`
   - Register client in room state
   - Send confirmation back to client

2. **Active connection**:
   - Spawn separate tasks for send/receive using `tokio::select!`
   - Split WebSocket stream: `let (sender, receiver) = socket.split()`
   - Concurrent read from WebSocket and write from mpsc channel

3. **Disconnection**:
   - Remove client from room state
   - Clean up empty rooms
   - Drop sender channel (automatically notifies receivers)

### 2.2 Message Broadcasting Strategies

#### Approach 1: Inverted Index (Recommended for mini-graph)

```rust
// When a message arrives:
async fn broadcast_to_room(
    rooms: &Arc<Mutex<HashMap<DocumentId, Vec<(ClientId, Sender)>>>>,
    document_id: &DocumentId,
    message: String,
) {
    let rooms = rooms.lock().await;
    if let Some(clients) = rooms.get(document_id) {
        for (_, tx) in clients {
            let _ = tx.send(message.clone()); // Ignore send errors
        }
    }
}
```

**Advantages:**
- Simple and efficient for small-to-medium scale
- Direct lookup by document_id
- No additional infrastructure needed

#### Approach 2: Broadcast Channels (Alternative)

Using `tokio::sync::broadcast`:

```rust
use tokio::sync::broadcast;

struct Room {
    tx: broadcast::Sender<String>,
}

// Each client subscribes to the broadcast channel:
let mut rx = room.tx.subscribe();
```

**Advantages:**
- One-to-many communication pattern
- Lower memory overhead for large rooms
- Built-in message buffering

**Trade-offs:**
- All clients in a room receive all messages (no filtering)
- Slower clients can cause message drops if buffer fills

**Recommendation for mini-graph**: Use the inverted index approach (Approach 1) for simplicity and fine-grained control.

### 2.3 Error Handling and Recovery

#### Server-Side Error Handling

```rust
// Handle errors during message sending:
for (client_id, tx) in clients {
    if tx.send(message.clone()).is_err() {
        // Client disconnected, will be cleaned up by disconnect handler
        continue;
    }
}
```

**Key principles:**
- Don't panic on client errors (disconnects, malformed messages)
- Log errors for debugging but continue serving other clients
- Use `Result` types and handle errors explicitly
- Implement graceful degradation

#### Connection State Management

Track WebSocket connection state:
- Handshake phase
- Open (active)
- Closing (graceful shutdown)
- Closed (connection terminated)

Handle abnormal terminations:
- Network failures
- Client process crashes
- Timeout errors

### 2.4 Reconnection Strategies

#### Client-Side Exponential Backoff

Implement reconnection with exponential backoff and jitter:

```javascript
class WebSocketClient {
    constructor(url) {
        this.url = url;
        this.baseDelay = 500; // 500ms
        this.maxDelay = 30000; // 30 seconds
        this.attempt = 0;
    }

    connect() {
        this.ws = new WebSocket(this.url);

        this.ws.onclose = () => {
            this.scheduleReconnect();
        };
    }

    scheduleReconnect() {
        const jitter = Math.random() * 1000; // 0-1000ms jitter
        const delay = Math.min(
            this.baseDelay * Math.pow(2, this.attempt) + jitter,
            this.maxDelay
        );

        setTimeout(() => {
            this.attempt++;
            this.connect();
        }, delay);
    }
}
```

**Why exponential backoff is critical:**
- Prevents "thundering herd" when server restarts
- Avoids overwhelming the server with reconnect requests
- Distributes reconnection attempts over time
- Reduces server load during recovery

**Additional best practices:**
- **Retry limits**: Set a maximum number of retries (e.g., 10 attempts)
- **Timeouts**: Set a timeout for each connection attempt
- **User feedback**: Show connection status to users
- **State restoration**: Re-subscribe to previous document/room on reconnect

#### Server-Side Connection Cleanup

```rust
// Use RAII pattern for automatic cleanup:
struct ClientGuard {
    client_id: ClientId,
    document_id: DocumentId,
    rooms: Arc<Mutex<HashMap<DocumentId, Vec<(ClientId, Sender)>>>>,
}

impl Drop for ClientGuard {
    fn drop(&mut self) {
        // Cleanup happens automatically when guard is dropped
        let rooms = self.rooms.clone();
        let client_id = self.client_id.clone();
        let document_id = self.document_id.clone();

        tokio::spawn(async move {
            let mut rooms = rooms.lock().await;
            if let Some(clients) = rooms.get_mut(&document_id) {
                clients.retain(|(id, _)| id != &client_id);
                if clients.is_empty() {
                    rooms.remove(&document_id);
                }
            }
        });
    }
}
```

## 3. Multi-Client Real-Time Patterns

### 3.1 Document/Room Isolation Patterns

**Pattern 1: HashMap-based Rooms (Recommended for mini-graph)**

```rust
HashMap<DocumentId, Vec<(ClientId, Sender)>>
```

**Characteristics:**
- Simple and efficient for moderate numbers of rooms
- Direct O(1) lookup by document_id
- Easy to implement and understand
- Works well for up to thousands of rooms

**Pattern 2: Actor Pattern**

Each room is an independent actor:
- Provides excellent isolation between rooms
- Simplifies state management
- Each room runs in its own task/goroutine
- Natural fit for message-passing architectures

**Characteristics:**
- Better isolation and fault tolerance
- Scales well to large numbers of rooms
- More complex to implement
- Consider for production systems

### 3.2 Client Identification Approaches

**UUID-based Client IDs (Recommended)**

```rust
use uuid::Uuid;

let client_id = Uuid::new_v4().to_string();
```

**Advantages:**
- Guaranteed uniqueness
- No coordination needed
- Works in distributed systems
- Easy to implement

**Alternative: Sequential IDs**

```rust
use std::sync::atomic::{AtomicU64, Ordering};

static CLIENT_COUNTER: AtomicU64 = AtomicU64::new(0);
let client_id = CLIENT_COUNTER.fetch_add(1, Ordering::SeqCst).to_string();
```

**Advantages:**
- Simpler and smaller IDs
- Easier to read in logs
- Sequential ordering

**Disadvantages:**
- Requires coordination in distributed systems
- Not suitable for sharded architectures

### 3.3 State Synchronization Strategies

#### Initial State Delivery

When a client joins a room:
1. Send acknowledgment with client_id
2. Send current room state (if applicable)
3. Begin streaming updates

#### Update Propagation

**Full State Updates (Recommended for mini-graph)**

```json
{
  "type": "state",
  "data": {
    "messages": [/* full list */]
  }
}
```

**Advantages:**
- Simple to implement and understand
- No need to handle incremental updates
- Easier to debug and reason about
- Matches mini-graph's invalidation-and-refetch pattern

**Alternative: Delta Updates (JSON Patch)**

```json
{
  "type": "patch",
  "patches": [
    {"op": "add", "path": "/messages/-", "value": {...}}
  ]
}
```

**Advantages:**
- Reduced bandwidth usage
- More efficient for large state objects
- Enables conflict resolution (CRDTs, OT)

**Trade-offs:**
- More complex implementation
- Requires careful ordering
- Harder to debug

**Recommendation for mini-graph**: Use full state updates initially. This matches the Edge module's behavior of sending full query results on invalidation.

### 3.4 Conflict Resolution

For the echo server demo, conflicts don't arise since we're just broadcasting messages. For future mini-graph features:

**Approaches:**
- **Last Write Wins (LWW)**: Simplest, timestamp-based
- **Operational Transformation (OT)**: Used by Google Docs
- **CRDTs**: Conflict-free replicated data types
- **Pessimistic locking**: Lock resources before editing

**Recommendation**: Start with LWW or no conflict resolution. Add complexity only when needed.

## 4. Architecture Patterns for Scalability

### 4.1 Single-Process Architecture (Current)

```
┌─────────────────────────────────────────┐
│         Rust Process (mini-graph)       │
│                                         │
│  ┌─────────────────────────────────┐   │
│  │  AppState (Arc<Mutex<>>)        │   │
│  │  ├── rooms: HashMap             │   │
│  │  └── clients: Vec               │   │
│  └─────────────────────────────────┘   │
│           ▲              ▲              │
│           │              │              │
│    WebSocket Handler    HTTP Handler    │
└───────────┼──────────────┼──────────────┘
            │              │
         Clients        Static Files
```

**Characteristics:**
- All state in-process memory
- Simple deployment
- Lost on restart (acceptable for demo)
- Limited to single machine resources

### 4.2 Future: Distributed Architecture

When scaling beyond a single process:

```
┌──────────┐    ┌──────────┐
│ Server 1 │    │ Server 2 │
│  (Axum)  │    │  (Axum)  │
└────┬─────┘    └────┬─────┘
     │               │
     └───────┬───────┘
             │
      ┌──────▼──────┐
      │   Redis     │
      │  (Pub/Sub)  │
      └─────────────┘
```

**Required changes:**
- Move room state to Redis
- Use Redis pub/sub for cross-server broadcasting
- Implement sticky sessions or session store
- Add load balancer

**Abstraction for future compatibility:**

```rust
#[async_trait]
trait RoomManager: Send + Sync {
    async fn join_room(&self, doc_id: &str, client_id: &str, tx: Sender);
    async fn leave_room(&self, doc_id: &str, client_id: &str);
    async fn broadcast(&self, doc_id: &str, message: String);
}

// In-memory implementation now:
struct InMemoryRoomManager { /* ... */ }

// Redis implementation later:
struct RedisRoomManager { /* ... */ }
```

## 5. Testing Strategies

### 5.1 Unit Testing

**Testing Connection Handlers**

Use `tokio_test::io::Builder` for mocking:

```rust
#[cfg(test)]
mod tests {
    use tokio_test::io::Builder;

    #[tokio::test]
    async fn test_message_handler() {
        let mock_stream = Builder::new()
            .read(b"test message")
            .build();

        // Test handler with mock stream
    }
}
```

**Testing State Management**

```rust
#[tokio::test]
async fn test_room_join_leave() {
    let state = AppState::new();
    let (tx, _rx) = mpsc::unbounded_channel();

    state.join_room("doc1", "client1", tx).await;
    assert_eq!(state.room_count("doc1").await, 1);

    state.leave_room("doc1", "client1").await;
    assert_eq!(state.room_count("doc1").await, 0);
}
```

### 5.2 Integration Testing

**Multi-Client Scenarios**

```rust
#[tokio::test]
async fn test_multi_client_broadcast() {
    // Start server
    let server = spawn_server();

    // Connect multiple clients
    let client1 = connect_websocket("ws://localhost:3030/ws").await;
    let client2 = connect_websocket("ws://localhost:3030/ws").await;

    // Both join same room
    client1.send(r#"{"type":"join","document_id":"doc1"}"#).await;
    client2.send(r#"{"type":"join","document_id":"doc1"}"#).await;

    // Client1 sends message
    client1.send(r#"{"type":"send_message","text":"hello"}"#).await;

    // Client2 should receive it
    let msg = client2.recv().await;
    assert!(msg.contains("hello"));
}
```

### 5.3 Manual Testing

**Test cases from WEBSOCKET_DEMO_PLAN.md:**

1. **Single client**: Open browser, connect, send message, verify echo
2. **Multiple clients, same document**: Open 2+ tabs, verify cross-client broadcasting
3. **Multiple clients, different documents**: Verify room isolation
4. **Reconnection**: Stop/restart server, verify client reconnect

**Tools:**
- Browser DevTools (Network tab for WebSocket frames)
- Postman or WebSocket client extensions
- Custom HTML client with detailed logging

### 5.4 Load Testing

For future performance testing:

**Tools:**
- **artillery**: Load testing with WebSocket support
- **k6**: Modern load testing tool
- **Custom Rust client**: Use tokio-tungstenite to spawn many clients

**Metrics to track:**
- Concurrent connections supported
- Message latency (send to receive)
- Memory usage per connection
- CPU usage under load
- Connection establishment rate

## 6. Potential Challenges and Mitigation

### 6.1 Backpressure and Memory Management

#### The Problem

**Backpressure**: The WebSocket API does not support backpressure. Calling `send()` repeatedly without considering the consumer's state can lead to memory leaks and instability.

**Symptoms:**
- Unbounded memory growth
- Server crashes with OOM errors
- Severe performance degradation
- Message queues growing indefinitely

**Root cause**: Slow consumers (clients) can't keep up with fast producers (server), causing messages to queue up in buffers.

#### Mitigation Strategies

**1. Bounded Channels (Recommended)**

```rust
// Instead of:
let (tx, rx) = mpsc::unbounded_channel();

// Use:
let (tx, rx) = mpsc::channel(100); // Buffer max 100 messages
```

**When buffer is full:**
- `send()` returns error or blocks
- Server can decide: drop message, close slow connection, or implement policy

**2. Close Slow Clients**

```rust
async fn send_with_timeout(tx: &Sender, msg: String) -> Result<()> {
    match timeout(Duration::from_secs(5), tx.send(msg)).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) | Err(_) => {
            // Client too slow or disconnected, close connection
            Err(anyhow!("Client too slow"))
        }
    }
}
```

**3. Monitor Buffer Depth**

```rust
if tx.capacity() - tx.len() < 10 {
    warn!("Client {} buffer nearly full", client_id);
}
```

**4. Compression Settings**

Each WebSocket connection consumes memory:
- Default compression: ~64 KiB per connection
- Disable compression for lower memory footprint
- Trade-off: higher bandwidth usage

**Recommendation for mini-graph**: Use bounded channels with a reasonable limit (e.g., 100 messages). Close clients that can't keep up.

### 6.2 Connection Cleanup and Resource Leaks

#### The Problem

**Memory leaks** can occur when:
- Client state isn't removed on disconnect
- Channels aren't properly closed
- Circular references prevent garbage collection
- Event listeners aren't unregistered

#### Mitigation Strategies

**1. RAII Pattern for Cleanup**

Use Rust's `Drop` trait for guaranteed cleanup:

```rust
struct ClientConnection {
    client_id: ClientId,
    document_id: DocumentId,
    state: Arc<Mutex<AppState>>,
}

impl Drop for ClientConnection {
    fn drop(&mut self) {
        // Cleanup happens automatically
        let state = self.state.clone();
        let client_id = self.client_id.clone();
        tokio::spawn(async move {
            state.remove_client(&client_id).await;
        });
    }
}
```

**2. Explicit Cleanup on Disconnect**

```rust
// In connection handler:
let result = handle_websocket(socket, state.clone()).await;

// Always cleanup, even on error:
state.remove_client(&client_id).await;
```

**3. Periodic Cleanup Task**

```rust
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        state.remove_stale_clients().await;
    }
});
```

**4. Track Idle Connections**

Close connections that haven't sent messages recently:

```rust
struct Client {
    last_activity: Instant,
    // ...
}

// In cleanup task:
if client.last_activity.elapsed() > Duration::from_secs(300) {
    client.close().await;
}
```

### 6.3 Race Conditions and State Consistency

#### The Problem

**Race conditions** in multi-threaded environments:
- Client joins room while another client is leaving
- Message broadcast happens during room cleanup
- Multiple threads modify shared state simultaneously

#### Mitigation Strategies

**1. Fine-Grained Locking**

```rust
// Instead of locking entire state:
struct AppState {
    rooms: Arc<Mutex<HashMap<DocumentId, Room>>>,
}

// Lock per-room:
struct AppState {
    rooms: HashMap<DocumentId, Arc<Mutex<Room>>>,
}
```

**2. Message Passing Over Shared State**

```rust
enum RoomCommand {
    Join { client_id: ClientId, tx: Sender },
    Leave { client_id: ClientId },
    Broadcast { message: String },
}

// Room runs in dedicated task:
async fn room_actor(mut rx: mpsc::Receiver<RoomCommand>) {
    let mut clients = Vec::new();

    while let Some(cmd) = rx.recv().await {
        match cmd {
            RoomCommand::Join { client_id, tx } => {
                clients.push((client_id, tx));
            },
            // ... handle other commands
        }
    }
}
```

**3. Use Atomic Operations**

For simple counters and flags:

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

struct Metrics {
    connection_count: AtomicUsize,
}

// Thread-safe increment:
metrics.connection_count.fetch_add(1, Ordering::SeqCst);
```

### 6.4 Testing Multi-Client Scenarios

#### Challenges

- Hard to reproduce timing-sensitive bugs
- Need to simulate concurrent connections
- Difficult to test at scale locally

#### Strategies

**1. Deterministic Testing with Tokio**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_concurrent_joins() {
    let state = Arc::new(AppState::new());

    // Spawn many clients concurrently
    let mut handles = vec![];
    for i in 0..100 {
        let state = state.clone();
        handles.push(tokio::spawn(async move {
            state.join_room("doc1", &format!("client{}", i), tx).await;
        }));
    }

    // Wait for all
    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(state.client_count("doc1").await, 100);
}
```

**2. Property-Based Testing**

Use `proptest` or `quickcheck`:

```rust
proptest! {
    #[test]
    fn test_room_invariants(ops: Vec<RoomOp>) {
        // Apply random sequence of operations
        // Check invariants hold
    }
}
```

**3. Load Testing with Custom Clients**

```rust
#[tokio::test]
#[ignore] // Run separately: cargo test --ignored
async fn load_test_100_clients() {
    let mut clients = vec![];

    for i in 0..100 {
        let client = WebSocketClient::connect("ws://localhost:3030").await;
        clients.push(client);
    }

    // Measure latency, throughput, etc.
}
```

### 6.5 WebSocket-Specific Gotchas

#### Message Ordering

**Problem**: Messages from different clients may arrive in different orders at different clients.

**Solution**:
- Add timestamps to messages
- Use sequence numbers if ordering is critical
- Accept eventual consistency for mini-graph

#### Frame Fragmentation

**Problem**: Large messages may be split into multiple frames.

**Solution**:
- tungstenite handles frame reassembly automatically
- Be aware of maximum message size limits
- Consider splitting large payloads application-side

#### Ping/Pong Heartbeats

**Problem**: Connections may appear alive but be dead (network partition).

**Solution**:
```rust
// Server sends periodic pings:
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        if socket.send(Message::Ping(vec![])).await.is_err() {
            break; // Connection dead
        }
    }
});
```

#### Close Handshake

**Problem**: Abrupt closes can leave state inconsistent.

**Solution**:
- Always send proper close frames
- Handle both clean and abrupt disconnects
- Use RAII for guaranteed cleanup regardless of close type

## 7. Existing Codebase Analysis

### 7.1 Current State

**Project structure:**
```
mini-graph/
├── Cargo.toml           # Edition 2024 (should be 2021), no dependencies
├── src/
│   └── main.rs          # Hello World only
├── docs/
│   ├── DESIGN.md        # Full architecture documented
│   ├── PLAN.md          # Project phases
│   ├── WEBSOCKET_DEMO_PLAN.md  # This implementation plan
│   └── RESEARCH.md      # General research
└── static/              # (to be created)
    └── index.html       # (to be created)
```

**Findings:**
- Clean slate: No existing WebSocket code
- Well-documented architecture in docs/
- No dependencies configured yet
- Ready for implementation

### 7.2 Dependencies to Add

Based on research and plan:

```toml
[package]
name = "mini-graph"
version = "0.1.0"
edition = "2021"  # Fix: 2024 is not valid

[dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# Web framework with WebSocket support
axum = { version = "0.7", features = ["ws"] }

# Static file serving
tower-http = { version = "0.6", features = ["fs"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Utilities
uuid = { version = "1.0", features = ["v4"] }
```

**Optional for future phases:**
- `sqlx` - PostgreSQL async driver
- `tokio-postgres` - PostgreSQL client
- `anyhow` / `thiserror` - Error handling

## 8. Implementation Recommendations

### 8.1 Phase 1: Basic Echo Server (Current)

**Goal**: WebSocket server with room-based broadcasting

**Steps:**
1. Add dependencies to Cargo.toml
2. Create AppState with room HashMap
3. Implement WebSocket upgrade handler
4. Implement join/send_message handlers
5. Add static file serving
6. Create HTML client with split-screen UI

**Success criteria:**
- Multiple clients can connect
- Messages broadcast within same room
- Rooms are isolated
- Clean disconnect handling

### 8.2 Phase 2: Integration with Cache (Future)

**Goal**: Connect WebSocket edge to Cache module

**Steps:**
1. Define Query types
2. Implement subscription management
3. Add inverted index
4. Connect to Cache via channels
5. Handle invalidations

**Changes to WebSocket setup:**
- Add subscription state to AppState
- Replace simple echo with query result delivery
- Add invalidation listeners

### 8.3 Best Practices Checklist

- [ ] Use bounded channels to prevent backpressure issues
- [ ] Implement RAII for automatic client cleanup
- [ ] Add proper error handling (don't panic on client errors)
- [ ] Use exponential backoff for client reconnection
- [ ] Separate concerns: connection management, room logic, message handling
- [ ] Write unit tests for state management
- [ ] Write integration tests for multi-client scenarios
- [ ] Monitor memory usage and connection counts
- [ ] Log errors and connection events
- [ ] Document WebSocket protocol in comments

### 8.4 Code Organization

Suggested module structure:

```rust
// src/main.rs
mod websocket;
mod state;

// src/websocket.rs
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    // Handle WebSocket upgrade
}

// src/state.rs
pub struct AppState {
    rooms: Arc<Mutex<HashMap<DocumentId, Vec<Client>>>>,
}

impl AppState {
    pub async fn join_room(&self, doc_id: &str, client: Client) { ... }
    pub async fn leave_room(&self, doc_id: &str, client_id: &str) { ... }
    pub async fn broadcast(&self, doc_id: &str, msg: String) { ... }
}
```

## 9. Documentation and Resources

### 9.1 Official Documentation

- **Axum WebSocket example**: https://github.com/tokio-rs/axum/blob/main/examples/websockets/src/main.rs
- **tokio-tungstenite GitHub**: https://github.com/snapview/tokio-tungstenite
- **Tokio async book**: https://tokio.rs/
- **tower-http documentation**: https://docs.rs/tower-http/

### 9.2 Learning Resources

- **Comprehensive Rust WebSocket Tutorial**: https://google.github.io/comprehensive-rust/concurrency/async-exercises/chat-app.html
- **WebSocket Implementation in Rust Guide**: https://websocket.org/guides/languages/rust/
- **Rust WebSocket Server Tutorial**: https://codezup.com/rust-websocket-server-tutorial/
- **Serving Static Files with Axum**: https://benw.is/posts/serving-static-files-with-axum

### 9.3 Best Practices Articles

- **Robust WebSocket Reconnection Strategies**: https://dev.to/hexshift/robust-websocket-reconnection-strategies-in-javascript-with-exponential-backoff-40n1
- **WebSocket Architecture Best Practices**: https://ably.com/topic/websocket-architecture-best-practices
- **Backpressure in WebSocket Streams**: https://skylinecodes.substack.com/p/backpressure-in-websocket-streams
- **Scaling WebSockets**: https://medium.com/@taycode/websockets-scaling-over-a-distributed-system-ea567d8372e5

### 9.4 Similar Projects

- **Figma LiveGraph**: https://www.figma.com/blog/livegraph-real-time-data-fetching-at-figma/
- **Real-time sync patterns**: https://cetra3.github.io/blog/synchronising-with-websocket/

## 10. Conclusion

### Key Takeaways

1. **Use Axum for everything**: Built-in WebSocket support, static file serving, and Tower ecosystem integration make it the clear choice for mini-graph.

2. **Start simple, architect for growth**: In-memory state now, but design with abstraction boundaries that allow extracting to Redis/distributed later.

3. **Backpressure is real**: Use bounded channels and close slow clients to prevent memory leaks.

4. **Exponential backoff is mandatory**: Prevent thundering herd problems during reconnection.

5. **Testing is critical**: Write tests for multi-client scenarios early, before subtle race conditions emerge.

### Implementation Priority

**High priority** (Phase 1):
- WebSocket server with room broadcasting
- Static file serving
- Client with reconnection logic
- Basic error handling and cleanup

**Medium priority** (Phase 2):
- Integration with Cache module
- Subscription management
- Inverted index

**Low priority** (Future):
- Redis-backed state
- Load balancing
- Metrics and monitoring
- Advanced conflict resolution

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Memory leaks from slow clients | Use bounded channels, implement timeouts |
| Race conditions in room state | Use Mutex carefully, consider message-passing |
| Complex multi-client bugs | Write comprehensive tests early |
| Difficulty debugging WebSocket issues | Add detailed logging, use browser DevTools |
| Performance problems at scale | Monitor early, load test periodically |

### Next Steps

1. Implement basic WebSocket echo server following WEBSOCKET_DEMO_PLAN.md
2. Test with multiple browser windows
3. Measure baseline performance
4. Iterate based on findings
5. Proceed to Phase 2 integration with Cache module

---

## Sources

- [tokio-tungstenite GitHub Repository](https://github.com/snapview/tokio-tungstenite)
- [WebSocket Implementation in Rust | WebSocket.org](https://websocket.org/guides/languages/rust/)
- [Comprehensive Rust Broadcast Chat Application](https://google.github.io/comprehensive-rust/concurrency/async-exercises/chat-app.html)
- [Axum WebSocket Example](https://github.com/tokio-rs/axum/blob/main/examples/websockets/src/main.rs)
- [Serving Static Files With Axum](https://benw.is/posts/serving-static-files-with-axum)
- [Robust WebSocket Reconnection Strategies with Exponential Backoff](https://dev.to/hexshift/robust-websocket-reconnection-strategies-in-javascript-with-exponential-backoff-40n1)
- [WebSocket Architecture Best Practices](https://ably.com/topic/websocket-architecture-best-practices)
- [Synchronizing state with WebSockets and JSON Patch](https://cetra3.github.io/blog/synchronising-with-websocket/)
- [Backpressure in WebSocket Streams](https://skylinecodes.substack.com/p/backpressure-in-websocket-streams)
- [WebSocket Memory and Buffers Documentation](https://websockets.readthedocs.io/en/stable/topics/memory.html)
- [Tokio Unit Testing Guide](https://tokio.rs/tokio/topics/testing)
- [The Fastest WebSocket Implementation](https://c410-f3r.github.io/thoughts/the-fastest-websocket-implementation/)
- [Scaling WebSockets Over a Distributed System](https://medium.com/@taycode/websockets-scaling-over-a-distributed-system-ea567d8372e5)
