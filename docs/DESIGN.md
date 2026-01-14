# Design

Design decisions and architectural choices for mini-graph, a simplified implementation of Figma's LiveGraph.

## Overview

Mini-graph is a learning-focused implementation of a real-time data synchronization system inspired by Figma's LiveGraph. It provides:

- **Real-time subscriptions**: Clients subscribe to queries and receive updates when underlying data changes
- **WAL-based invalidation**: Changes detected via Postgres logical replication trigger cache invalidation
- **Query decomposition**: Complex views broken into simple single-table subqueries

### Data Flow

```
┌──────────────────────────────────────────────────────────────────────────┐
│                            mini-graph process                            │
│                                                                          │
│  ┌─────────┐       ┌─────────┐       ┌─────────────┐                     │
│  │  Edge   │◄─────►│  Cache  │◄──────│ Invalidator │                     │
│  └────┬────┘       └────┬────┘       └──────┬──────┘                     │
│       │                 │                   │                            │
└───────┼─────────────────┼───────────────────┼────────────────────────────┘
        │                 │                   │
        ▼                 ▼                   ▼
   ┌─────────┐       ┌─────────┐       ┌─────────────┐
   │ Clients │       │Postgres │       │  WAL Stream │
   │  (WS)   │       │   DB    │       │  (logical)  │
   └─────────┘       └─────────┘       └─────────────┘
```

### Request Flow

1. Client connects via WebSocket to **Edge**
2. Client sends subscription request: `{"subscribe": "comments_by_file", "params": {"file_id": 123}}`
3. Edge registers subscription in the **inverted index**
4. Edge requests query result from **Cache**
5. Cache checks local store or fetches from Postgres
6. Edge sends initial result to client
7. Later: row changes in Postgres → WAL entry → **Invalidator** → invalidation message → Cache invalidates → Edge re-fetches → client receives update

## Core Components

### Edge Module

**Responsibility**: Client connection management, subscription lifecycle, result delivery.

```rust
// Conceptual structure
struct Edge {
    subscriptions: HashMap<SubscriptionId, Subscription>,
    inverted_index: InvertedIndex,
    cache_tx: mpsc::Sender<CacheRequest>,
    invalidation_rx: broadcast::Receiver<Invalidation>,
}

struct Subscription {
    id: SubscriptionId,
    client_id: ClientId,
    query: Query,
    hints: Vec<QueryHint>,  // e.g., ["comments:file_id:123"]
}
```

**Key behaviors**:
- Accepts WebSocket connections (tokio-tungstenite)
- Parses subscription requests into `Query` structs
- Derives `QueryHint` keys from query parameters
- Maintains inverted index: `hint → [subscription_ids]`
- Listens for invalidations, matches via inverted index, triggers re-fetch
- Sends full query results to clients on initial subscribe and on invalidation

### Cache Module

**Responsibility**: Query result storage, database fetching, invalidation handling.

```rust
struct Cache {
    store: HashMap<QueryHash, CachedResult>,
    db_pool: PgPool,
    invalidation_rx: mpsc::Receiver<InvalidationBatch>,
}

struct CachedResult {
    data: serde_json::Value,
    version: u64,
    cached_at: Instant,
}
```

**Key behaviors**:
- Read-through cache: if miss, fetch from Postgres
- On invalidation: remove entry from store (lazy re-fetch on next request)
- Query results keyed by hash of (query_name, params)

### Invalidator Module

**Responsibility**: WAL consumption, invalidation generation.

```rust
struct Invalidator {
    wal_stream: LogicalReplicationStream,
    cache_tx: mpsc::Sender<InvalidationBatch>,
    edge_tx: broadcast::Sender<Invalidation>,
}

struct WalEntry {
    table: String,
    operation: Operation,  // Insert, Update, Delete
    columns: HashMap<String, Value>,
}

struct Invalidation {
    hints: Vec<QueryHint>,  // e.g., ["comments:file_id:123", "comments:id:456"]
}
```

**Key behaviors**:
- Connects to Postgres logical replication slot
- Parses WAL entries (pgoutput format)
- Generates query hints from row data based on registered hint patterns
- Broadcasts invalidations to Cache and Edge
- **Stateless**: no persistent state, can restart cleanly

### Inverted Index

**Responsibility**: Efficient lookup of subscriptions affected by an invalidation.

```rust
struct InvertedIndex {
    // hint_key → set of subscription IDs
    index: HashMap<QueryHint, HashSet<SubscriptionId>>,
}

impl InvertedIndex {
    fn register(&mut self, hint: QueryHint, sub_id: SubscriptionId);
    fn unregister(&mut self, hint: QueryHint, sub_id: SubscriptionId);
    fn lookup(&self, hint: &QueryHint) -> HashSet<SubscriptionId>;
}
```

**Query hints** follow the pattern: `table:column:value`
- Example: `comments:file_id:123`
- When a `comments` row with `file_id=123` changes, we look up this hint

## Query System

### Query Definition

Queries are defined as named templates with typed parameters:

```rust
enum QueryDef {
    CommentsByFile { file_id: i64 },
    CommentById { id: i64 },
    UserById { id: i64 },
    FileById { id: i64 },
}
```

Each query definition specifies:
1. The SQL to execute (parameterized)
2. The hint patterns it matches (for invalidation)

```rust
impl QueryDef {
    fn sql(&self) -> &str { ... }
    fn hints(&self) -> Vec<QueryHint> { ... }
}

// Example:
// CommentsByFile { file_id: 123 }
//   sql: "SELECT * FROM comments WHERE file_id = $1"
//   hints: ["comments:file_id:123"]
```

### Hint Generation from WAL

When we see a WAL entry:
```
table: comments
operation: INSERT
data: { id: 456, file_id: 123, text: "Hello", user_id: 789 }
```

We generate hints for columns that queries might filter on:
- `comments:id:456`
- `comments:file_id:123`
- `comments:user_id:789`

The inverted index matches these against registered subscriptions.

### Invalidation Strategy

We use **invalidation-and-refetch** (not IVM):

1. WAL entry arrives
2. Invalidator generates hints
3. Cache removes affected entries
4. Edge receives invalidation, looks up affected subscriptions
5. Edge re-fetches full results for affected subscriptions
6. Edge sends new results to clients

This is simpler than IVM and avoids complex in-memory state management.

## Wire Protocol

### Client → Edge

**Subscribe**:
```json
{
  "type": "subscribe",
  "id": "sub_1",
  "query": "comments_by_file",
  "params": { "file_id": 123 }
}
```

**Unsubscribe**:
```json
{
  "type": "unsubscribe",
  "id": "sub_1"
}
```

### Edge → Client

**Initial result / Update**:
```json
{
  "type": "data",
  "subscription_id": "sub_1",
  "version": 1,
  "data": [
    { "id": 1, "text": "First comment", "user_id": 10 },
    { "id": 2, "text": "Second comment", "user_id": 11 }
  ]
}
```

**Error**:
```json
{
  "type": "error",
  "subscription_id": "sub_1",
  "message": "Query failed: connection error"
}
```

We send **full results** on every update, not diffs. Simpler to implement and reason about.

## Key Differences from Production LiveGraph

### What We're Implementing

| Feature | Production | Mini-graph |
|---------|------------|------------|
| Core architecture | Edge/Cache/Invalidator | ✅ Same pattern |
| WAL-based invalidation | Yes | ✅ Yes |
| Inverted index | Yes | ✅ Yes |
| Query decomposition | Yes | ✅ Simplified |
| WebSocket subscriptions | Yes | ✅ Yes |
| Invalidation-and-refetch | Yes | ✅ Yes |

### What We're Leaving Out

| Feature | Why Excluded | Could Layer On? |
|---------|--------------|-----------------|
| **Horizontal sharding** | Complexity; single-node sufficient for learning | ✅ Yes - cache keyed by hash, just route to shards |
| **Database sharding** | Requires multi-DB setup | ✅ Yes - Invalidator maps to DB shards |
| **Kafka for WAL distribution** | Overkill; direct PG replication sufficient | ✅ Yes - swap channel for Kafka consumer |
| **Cuckoo filters** | Probabilistic fan-out limiting; not needed at small scale | ⚠️ Maybe - adds complexity |
| **IVM (Incremental View Maintenance)** | Complex; invalidation-and-refetch simpler | ❌ No - fundamental design choice |
| **Multiple service instances** | Single process sufficient | ✅ Yes - extract to separate binaries |
| **GraphQL interface** | Complexity; simple JSON protocol sufficient | ✅ Yes - add resolver layer |
| **Authentication/authorization** | Orthogonal to core sync logic | ✅ Yes - middleware layer |
| **Metrics/distributed tracing** | Production concern | ✅ Yes - add tracing crate |
| **Connection pooling/retries** | Production hardening | ✅ Yes - add resilience layer |

### Sharding-Ready Design

We design with clear boundaries so sharding can be added later:

```
Current (single process):
  Edge ←→ Cache ←→ Invalidator
       (mpsc channels)

Future (distributed):
  Edge₁ ←→ Cache₁ ←→ Invalidator₁
  Edge₂ ←→ Cache₂ ←→ Invalidator₂
       (gRPC / message queue)
```

**Key abstractions that enable this:**

1. **Cache is keyed by query hash** - sharding = route hash to shard
2. **Invalidator is stateless** - can run multiple instances, one per DB shard
3. **Edge is stateless per-subscription** - can run multiple behind load balancer
4. **All inter-component communication via message passing** - swap channels for network

We use trait abstractions for communication:
```rust
#[async_trait]
trait CacheClient {
    async fn get(&self, query: &Query) -> Result<CachedResult>;
    async fn invalidate(&self, hints: &[QueryHint]);
}

// In-process implementation now, gRPC implementation later
```

## Trade-offs

### Simplicity vs. Performance

- **Full result updates**: Simpler than diffs, but more bandwidth. Acceptable for learning.
- **Single-threaded event loop**: Simpler than work-stealing, but limits throughput.
- **In-memory cache only**: No persistence, lost on restart. Acceptable for demo.

### Correctness vs. Complexity

- **Read-invalidation rendezvous**: We implement the pattern where Edge subscribes to invalidations *before* requesting from Cache. This ensures we don't miss invalidations during in-flight reads.

- **No ordering guarantees**: We don't guarantee clients see updates in exact causal order. For mini-graph, eventual consistency is acceptable.

### Flexibility vs. YAGNI

- **Fixed query definitions**: Queries hardcoded in Rust, not dynamic SQL. Simpler, safer.
- **Single table subqueries**: No JOIN support in queries. Matches LiveGraph's decomposition model.
- **Explicit hint patterns**: We manually specify which columns trigger invalidation rather than introspecting all columns.

## Demo Schema

For demonstration, we use a simple schema:

```sql
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT NOT NULL
);

CREATE TABLE files (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    owner_id INTEGER REFERENCES users(id)
);

CREATE TABLE comments (
    id SERIAL PRIMARY KEY,
    file_id INTEGER REFERENCES files(id),
    user_id INTEGER REFERENCES users(id),
    text TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT NOW()
);
```

**Supported queries:**
- `comments_by_file(file_id)` - All comments on a file
- `comment_by_id(id)` - Single comment
- `user_by_id(id)` - Single user
- `file_by_id(id)` - Single file

This mirrors a simplified Figma-like model: files have comments, comments have authors.

## Implementation Phases

### Phase 1: Foundation
- Project structure with Edge/Cache/Invalidator modules
- Basic types: Query, Subscription, Invalidation, QueryHint
- In-memory inverted index
- Channel-based inter-component communication

### Phase 2: Database & WAL
- Postgres connection with sqlx
- Logical replication setup
- WAL stream parsing (pgoutput)
- Invalidation generation from WAL entries

### Phase 3: Cache & Query Execution
- In-memory cache store
- Query execution against Postgres
- Cache invalidation flow
- Read-invalidation rendezvous handling

### Phase 4: WebSocket & Client Protocol
- WebSocket server (tokio-tungstenite)
- JSON message parsing
- Subscription management
- Result delivery to clients

### Phase 5: Integration & Demo
- End-to-end flow testing
- Demo client (could be simple HTML/JS)
- Documentation and examples

## References

- [LiveGraph: real-time data fetching at Figma](https://www.figma.com/blog/livegraph-real-time-data-fetching-at-figma/)
- [Keeping It 100(x) With Real-time Data At Scale](https://www.figma.com/blog/livegraph-real-time-data-at-scale/)
- [The Hard Things About Sync](https://expertofobsolescence.substack.com/p/the-hard-things-about-sync)
