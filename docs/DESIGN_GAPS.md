# Design Gaps & Implementation Questions

Specific ambiguities and unknowns that will impact architecture decisions before we start implementing.

## Query Definition & Invalidation Rules

**Critical Gap:** How are queries and their invalidation rules actually specified?

- **Option A**: Queries defined in code/config, invalidation rules manually written for each query
  - Pros: Explicit, predictable, full control
  - Cons: Manual maintenance, error-prone

- **Option B**: Queries defined in config/schema, invalidation rules auto-derived from table dependencies
  - Pros: Less manual work, stays in sync with schema
  - Cons: May not capture all edge cases, magic behavior

- **Option C**: Hybrid - schema introspection + explicit overrides for complex cases

**Questions:**
- Can invalidation rules be automatically derived from query structure (table references)?
- Or do we need explicit configuration like `"query_name": ["table_1", "table_2"]`?
- How do we handle queries that reference objects indirectly (e.g., comments on a file)?

## Inverted Index Construction & Management

**How is it built?**
- At startup: scan all known query shapes and build index?
- Dynamically: as new subscriptions arrive from clients?
- Periodically rebuilt?

**How does it handle complex queries?**
- A query that references multiple tables: do we create multiple index entries?
- Queries with complex WHERE clauses: do we index the condition, or just the table?
- How granular should the index be (whole query vs individual tables)?

**What happens during schema changes?**
- If we add a column, does the inverted index need rebuilding?
- If we add a new table reference, how do we update affected queries?

## WAL Entry Parsing & Invalidation Generation

**What information do we extract from WAL entries?**
- Table name?
- Column names that changed?
- Old vs new values?
- Transaction boundaries?

**How detailed is the invalidation key?**
- Just `"table_name"` and ID like `"comments:123"`?
- Does it include column information like `"comments:text:123"`?
- Can we match it to specific queries, or is it a broadcast to Cache?

## Message Formats & Protocols

**WebSocket subscription format:**
- How do clients request updates? Raw SQL? Named queries? Structured format?
- Example: `{"subscribe": "SELECT * FROM comments WHERE file_id = 1"}`?
- Or: `{"subscribe_query": "comments_by_file", "params": {"file_id": 1}}`?

**Update message format:**
- Full result? `{"updates": [{"id": 1, "text": "new text"}]}`?
- Diff? `{"added": [...], "removed": [...], "modified": [...]}`?
- With metadata? `{"query_id": "...", "version": 123, "results": [...]}`?

**Error handling:**
- What if a query re-fetch fails? Send error message? Retry silently?
- Partial updates possible, or all-or-nothing?

## Edge Service Ephemeral State

**Inverted Index & Subscriptions:**
- On restart, do we rebuild the inverted index from scratch?
- Do we lose all active subscriptions (clients reconnect)?
- Is there a "recovery" mechanism, or is restart a clean slate?

**Query result freshness:**
- When Edge fetches from Cache, is it guaranteed fresh after invalidation?
- What if Cache is mid-invalidation when Edge requests? Race condition handling?

## Cache Service Consistency

**Between Cache and Database:**
- Cache is the source of truth, or is DB?
- What if Cache has stale data? (Ideally never, but failure modes?)

**Between Cache and Edge:**
- How does Cache know which Edge instances to notify?
- The probabilistic filter (Cuckoo filter in production) - when is this needed?
  - For mini-graph: can we just broadcast all invalidations?

## Architecture Extraction Points

**When extracting modules into separate services:**
- What does Invalidator → Cache interface look like? Message queue? RPC?
- What does Cache → Edge interface look like?
- What does Edge → Client interface look like?
- Are there consistency guarantees we need to maintain?

## Mini-Graph Simplifications

**What can we simplify for the prototype?**
- Manual query registration instead of auto-discovery?
- Hard-coded invalidation rules for our demo schema?
- All components in one process with in-memory channels?
- Simple broadcast for all invalidations (no filters)?

**What's essential to keep the architecture honest?**
- Clear module boundaries (distinct modules with defined contracts)?
- Inverted index concept (even if simple)?
- Query decomposition (even with just a few queries)?
- Real WebSocket updates?
