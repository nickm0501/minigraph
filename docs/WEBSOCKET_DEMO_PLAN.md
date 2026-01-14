# WebSocket Echo Server Implementation Plan

## Goal

Build a simple HTML page with WebSocket connection that demonstrates real-time multi-client streaming using document-based rooms.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Rust WebSocket Server                    │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Document Rooms: HashMap<DocumentId, Vec<ClientId>>   │ │
│  └────────────────────────────────────────────────────────┘ │
│                           │                                  │
│     ┌─────────────────────┼─────────────────────┐           │
│     ▼                     ▼                     ▼           │
│  Client A             Client B             Client C          │
│  (doc: "demo")        (doc: "demo")        (doc: "other")   │
└─────────────────────────────────────────────────────────────┘
         │                     │                     │
         ▼                     ▼                     ▼
    Browser Tab 1        Browser Tab 2        Browser Tab 3
```

## Component 1: Rust WebSocket Server

**File**: `src/main.rs`

**Dependencies to add**:
```toml
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.24"
futures-util = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.0", features = ["v4"] }
```

**Core data structures**:
```rust
// Client connection identifier
type ClientId = String;

// Document/room identifier
type DocumentId = String;

// Message types
enum ClientMessage {
    Join { document_id: DocumentId },
    SendMessage { text: String },
}

enum ServerMessage {
    Joined { document_id: DocumentId, client_id: ClientId },
    Message { client_id: ClientId, text: String, timestamp: u64 },
    Error { message: String },
}

// Shared state
struct AppState {
    // document_id -> list of client senders
    rooms: HashMap<DocumentId, Vec<(ClientId, mpsc::UnboundedSender<String>)>>,
}
```

**Server behavior**:
1. Listen on `0.0.0.0:3030` for WebSocket connections
2. On connection:
   - Generate random `client_id` (UUID)
   - Wait for `Join` message with `document_id`
   - Add client to the specified room
   - Send `Joined` confirmation back
3. On message from client:
   - Broadcast to all clients in the same document room
   - Include sender `client_id` and timestamp
4. On disconnect:
   - Remove client from their room
   - Clean up empty rooms

**Server also serves static files**:
- Serve `static/index.html` on `http://localhost:3030/`
- Simple HTTP server alongside WebSocket endpoint

## Component 2: HTML Client

**File**: `static/index.html`

**Layout structure**:
```
┌─────────────────────────────────────────────────────────┐
│ MiniGraph Echo Demo - Document: [demo-doc]   [Status]  │
├──────────────────────┬──────────────────────────────────┤
│                      │                                  │
│   Send Messages      │    Live Stream                   │
│   (Left Pane)        │    (Right Pane)                  │
│                      │                                  │
│  ┌────────────────┐  │  • [12:34:56] client-abc: Hi    │
│  │                │  │  • [12:34:58] client-xyz: Hello │
│  │  Text Input    │  │  • [12:35:01] client-abc: Test  │
│  │  (textarea)    │  │                                  │
│  │                │  │  (auto-scrolls to bottom)        │
│  └────────────────┘  │                                  │
│  [Send Button]       │                                  │
│                      │                                  │
└──────────────────────┴──────────────────────────────────┘
```

**UI Elements**:

**Top Bar**:
- Input field for `document_id` (defaults to "demo-doc", can be changed)
- Connection status indicator (Connected ✓ / Disconnected ✗)
- Shows assigned client_id once connected

**Left Pane (40% width)**:
- Textarea for message input
- "Send" button (also supports Enter key)
- Clear input after sending

**Right Pane (60% width)**:
- Scrolling message list (newest at bottom)
- Each message shows:
  - Timestamp (HH:MM:SS)
  - Client ID (first 8 chars for readability)
  - Message text
- Different styling for own messages vs. others
- Auto-scroll to bottom on new message

**WebSocket Protocol** (JSON over WebSocket):

Client → Server:
```json
// On connection, join document
{"type": "join", "document_id": "demo-doc"}

// Send a message
{"type": "send_message", "text": "Hello world"}
```

Server → Client:
```json
// Confirmation of join
{"type": "joined", "document_id": "demo-doc", "client_id": "abc-123-def"}

// Broadcast message
{"type": "message", "client_id": "xyz-789", "text": "Hello world", "timestamp": 1704067200}

// Error
{"type": "error", "message": "Invalid document_id"}
```

**JavaScript behavior**:
1. On page load:
   - Read `document_id` from input (or use default "demo-doc")
   - Connect to `ws://localhost:3030/ws`
   - Send `join` message immediately after connection opens
2. On successful join:
   - Update status to "Connected ✓"
   - Display assigned `client_id`
3. On "Send" button click:
   - Send `send_message` with textarea content
   - Clear textarea
   - Focus back to textarea
4. On message received:
   - Append to right pane message list
   - Highlight own messages differently
   - Auto-scroll to bottom

## Testing Plan

**Manual test cases**:
1. **Single client**:
   - Open browser → should connect
   - Type message → should see own message appear

2. **Multiple clients, same document**:
   - Open 2+ tabs with same `document_id`
   - Type in one → all others should see it instantly
   - Messages should show different `client_id`s

3. **Multiple clients, different documents**:
   - Open tab with `document_id="doc1"`
   - Open tab with `document_id="doc2"`
   - Messages in doc1 should NOT appear in doc2

4. **Reconnection**:
   - Stop server while client connected
   - Restart server
   - Client should show disconnect, allow manual reconnect

## File Structure

```
mini-graph/
├── Cargo.toml
├── src/
│   └── main.rs          # WebSocket server + static file serving
└── static/
    └── index.html       # Split-screen client interface
```

## Implementation Order

1. Add dependencies to `Cargo.toml`
2. Implement WebSocket server core (connection handling, room management)
3. Create HTML client with split-screen layout
4. Wire up WebSocket protocol (join, send, receive)
5. Test with multiple browser windows

## Success Criteria

- [ ] Server starts and listens on port 3030
- [ ] Browser can load HTML page from http://localhost:3030/
- [ ] WebSocket connection establishes successfully
- [ ] Single client can send and receive messages
- [ ] Multiple clients in same document see each other's messages in real-time
- [ ] Clients in different documents are isolated
- [ ] Message display shows timestamp and sender client_id
- [ ] Own messages are visually distinct from others
