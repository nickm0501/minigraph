# Implementation Plan

Roadmap and task tracking for building the mini version.

## Project Goal

Build the simplest possible HTML page that connects to WebSocket and streams PostgreSQL updates via MiniGraph services in real-time.

### Demo Application Concept

A two-pane Figma-like canvas app:

**Left Pane: Object Creator**
- Simple form to add new objects (notes, shapes, etc.)
- Fields for text content, metadata, styling
- Button to persist to database

**Right Pane: Live Canvas**
- Displays all objects in a canvas view
- Real-time updates via WebSocket from MiniGraph
- Updates as other clients (or the same client via DB → invalidation → WebSocket loop) create/modify objects
- Shows objects that were created by others

**Example Flow:**
1. User types a note and clicks "Add"
2. App sends INSERT to database
3. PostgreSQL WAL triggers invalidation
4. MiniGraph processes invalidation and detects affected queries
5. WebSocket pushes updated object list to client
6. Canvas updates in real-time to show new note

**Key Behaviors to Test:**
- Adding new objects
- Updating object text/metadata
- Deleting objects
- Multiple concurrent changes
- Client reconnection with state resync

## Phases

### Phase 1: Foundation
- Set up Docker Compose with PostgreSQL
- Implement Invalidator module (WAL consumption)
- Implement Cache module (query result storage)
- Implement Edge module (subscription management)
- Basic WebSocket server

### Phase 2: Core Features
- Define object schema and queries
- Implement query decomposition (subqueries)
- Implement inverted index for query dependencies
- Build demo HTML client
- End-to-end invalidation flow

### Phase 3: Polish
- Error handling and recovery
- Performance optimization
- Documentation and examples
