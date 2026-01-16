# MiniGraph

A toy project to implement a [LiveGraph](https://www.figma.com/blog/livegraph-real-time-data-at-scale/)-like project. Minimal, room-based Websocket server built with Rust, Axum, and Tokio.

## Run

- Start the server: `cargo run`
- Websocket endpoint: `ws://localhost:3030/ws`

## Manual Websocket Testing (wscat)

In one terminal:
- Connect: `wscat -c ws://localhost:3030/ws`
- Join a room: `{"type":"join","document_id":"doc1"}`
- Send a message: `{"type":"send_message","text":"hello"}`

In a second terminal:
- Connect: `wscat -c ws://localhost:3030/ws`
- Join the same room: `{"type":"join","document_id":"doc1"}`

Messages sent from either client should be broadcast to both clients in the same room.
