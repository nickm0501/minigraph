# Research

Links and findings about the real project.

## References

- Project Link:
- Documentation:

## Key Findings

### Architecture Overview

A cache invalidation system that propagates changes from database writes through multiple layers to push real-time updates to clients.

**Data Flow:**
1. Database Write → WAL Entry
2. Invalidator generates invalidation keys (e.g., "comments:file_id:1")
3. Cache Service receives invalidation:
   - Evicts cached results for the key
   - Forwards invalidation to Edge Service
4. Edge Service processes the invalidation:
   - Looks up inverted index to find affected queries (Query A, Query B, etc.)
   - Re-fetches queries from Cache (which may need to hit DB)
   - Pushes updates to connected clients via WebSocket

### Change Detection Pipeline

Detailed flow from database mutation to client update:

1. **Database Write**: Application writes to PostgreSQL
2. **WAL Entry**: PostgreSQL writes to its Write-Ahead Log (replication stream)
3. **Kafka Distribution** (Production) / **Direct Consumption** (Mini-Graph): WAL entries distributed via Kafka to LiveGraph servers (or consumed directly in our mini version)
4. **Invalidation Generation**: Invalidator processes WAL entries and generates invalidation keys
5. **Cache Invalidation**: Relevant cache shards evict affected entries
6. **Edge Notification**: Edge receives invalidation via probabilistic filter (or all invalidations in mini version)
7. **Re-query**: Edge re-fetches affected queries from cache (which hits database if needed)
8. **Client Update**: Updated results sent to client via WebSocket

### Query Decomposition

Views are decomposed into a tree of subqueries for efficient caching and incremental updates:

**Subquery Properties:**
- Fetches a single type of object
- Takes the form: `SELECT columns FROM table WHERE condition`
- No joins at the subquery level
- Can be shared across multiple client views (deduplication)

**Benefits:**
- Granular cache entries enable efficient invalidation
- Multiple views can depend on the same subquery (cache hit opportunity)
- Changes to a single object only invalidate specific subqueries
- Allows targeted updates to clients without fetching entire views

### State Management

| Component | State | Persistent? | Notes |
|-----------|-------|-------------|-------|
| Invalidator | Query shapes only (static) | No | No runtime state |
| Cache | Cached query results + Edge filters | In-memory | Survives Edge deploys |
| Edge | Inverted index + active subscriptions | In-memory | Rebuilt on restart |

### Important Concepts

- **Invalidation Keys**: Hierarchical keys that identify what data changed (e.g., resource_type:resource_id)
- **Inverted Index**: Maps data changes to the queries that depend on them (at Edge level)
- **Query Subscriptions**: Long-lived client connections that receive updates via WebSocket
- **Cache Filters**: Rules stored in Cache that Edge uses to determine which queries are affected by a change
- **WAL Entry**: Write-Ahead Log entry triggers the invalidation cascade

### Notable Patterns

- **Layered invalidation**: Changes flow through distinct services (Cache → Edge) with clear responsibilities
- **Inverted dependency tracking**: Edge knows which queries depend on which data, enabling targeted updates
- **In-memory vs persistent state**: Query metadata is cached/persisted; subscriptions/index are ephemeral

## Mini-Graph Implementation Strategy

We'll build a simplified, single-process version that maintains the same architectural shape and module boundaries as LiveGraph 100x, but without distributed systems complexity.

### Inspired By
- **Original Architecture (pre-2024)**: Single server with in-memory cache, WebSocket clients
- **100x Architecture**: Three-service approach with clear separation: Invalidator → Cache → Edge

### Mini-Graph Approach
- **Single Rust binary** with distinct modules (Invalidator, Cache, Edge) that could be extracted into services
- **Local Postgres with WAL** (instead of Kafka): Direct logical decoding from database
- **In-memory state**: All caching and subscriptions in-process
- **WebSocket support**: Rust native (tokio ecosystem has excellent support)
- **Docker Compose** for easy local Postgres setup

### Key Simplifications vs LiveGraph 100x
- No distributed sharding (single database, single cache, single edge)
- No Kafka; direct WAL consumption locally
- No probabilistic filters needed (single cache instance)
- No hot standby complexity
- All state lost on process restart (acceptable for local development)

## Open Questions & Considerations

### Schema & Migrations
- **Query Shape Definition**: How do we define and version query shapes? Static configuration file? Code generation? Schema introspection?
- **Schema Changes**: When the database schema evolves, how should invalidation rules adapt? What's the contract?
- **Migration Coupling**: Is the system inherently tied to Postgres WAL, or can invalidation rules be defined independently?
- **Schema Versioning**: How do we handle multiple versions of queries in flight during schema transitions?
- **Generalizability**: Can invalidation rules be database-agnostic? Could this work with MySQL, SQLite, or other databases?
- **Open Source Potential**: What would need to be abstracted/generalized to make this reusable across different applications?

### Query & Subscription Design
- **Query Format**: How are queries defined? SQL strings? GraphQL? Custom DSL? What metadata do they need?
- **View Expansion**: How does Edge expand client view requests into specific query shapes?
- **Invalidation Rule Format**: How do we specify which queries depend on which data changes? (e.g., "when table X changes, invalidate query Y")
- **Subscription Protocol**: What's the wire format for client subscriptions? How do clients request views vs individual queries?

### Operational & Reliability
- **Failure Handling**: What happens if a query re-fetch fails during invalidation? Partial updates? Retry logic?
- **Client Reconnection**: How do we handle clients reconnecting after network partitions?
- **Hot Query Tracking**: How do we know which queries are actively subscribed (for optimization)?
- **Testing**: How do we test this without a real database? Mock WAL? In-memory database?
- **Key Generation**: Are invalidation keys hardcoded per table, or can they be derived from schema?

### Architecture Boundaries
- **Module Contracts**: What are the exact inputs/outputs between Invalidator → Cache → Edge?
- **Configuration**: How much should be configurable vs hardcoded in the mini version?
- **Extractability**: What changes would be needed to extract modules into separate services?
