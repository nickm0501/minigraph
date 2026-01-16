# Architecture

## Data Flow (Actor Model)

```
                        (1) WebSocket frames
┌──────────────┐       (Text/Binary/Ping/...)
│   Client      │──────────────────────────────────────┐
└──────────────┘                                      │
                                                      ▼
                                            ┌──────────────────┐
                                            │  recv_task (per   │
                                            │  connection)      │
                                            │  - parse JSON      │
                                            │  - enforce join ack│
                                            └─────────┬────────┘
                                                      │
                                   RoomCommand (bounded│, 256)
                                                      ▼
                                            ┌──────────────────┐
                                            │ Rooms Actor       │
                                            │ (single owner of  │
                                            │ room membership)  │
                                            └─────────┬────────┘
                                                      │
                           ServerMessage (bounded, 256│) per client
                                                      ▼
                                            ┌──────────────────┐
                                            │ send_task (per    │
                                            │ connection)       │
                                            │ - serialize JSON   │
                                            │ - write to socket  │
                                            └─────────┬────────┘
                                                      │
                                                      ▼
                                            ┌──────────────────┐
                                            │   Client          │
                                            └──────────────────┘
```

Legend:
- `recv_task`, `send_task`, and `rooms_actor` are Tokio tasks spawned via `tokio::spawn`.
- `RoomCommand` and `ServerMessage` arrows are `tokio::sync::mpsc` bounded channels.

## Invariants

- Only the rooms actor owns room membership state.
- Join is only considered successful after an actor ack within 250ms.
- Rooms command channel is bounded (256); overflow drops newest commands.
- Per-client outbound queue is bounded (256); overflow drops newest messages.
- During broadcast:
  - `Full` per-client queue: drop newest message for that client.
  - `Closed` per-client queue: remove client from the room.
- On disconnect, the server best-effort sends a `Leave` based on per-session room state.

## Future: Postgres WAL Streaming

- Recommended integration shape: WAL reader task emits events into a bounded channel feeding the rooms actor (or a dedicated “events” actor), which then fans out to rooms.
