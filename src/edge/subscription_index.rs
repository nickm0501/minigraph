use std::collections::{HashMap, HashSet};

use crate::wal::QueryHint;

pub type SubscriptionId = String;

#[derive(Debug)]
pub enum SubscriptionIndexError {
    AlreadySubscribed(SubscriptionId),
    UnknownSubscription(SubscriptionId),
}

impl std::fmt::Display for SubscriptionIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubscriptionIndexError::AlreadySubscribed(id) => {
                write!(f, "subscription already exists: {id}")
            }
            SubscriptionIndexError::UnknownSubscription(id) => {
                write!(f, "unknown subscription: {id}")
            }
        }
    }
}

impl std::error::Error for SubscriptionIndexError {}

/// A bookkeeping helper for mapping *invalidation hints* --> *active subscriptions*.
///
/// This is the core in-memory data structure behind a LiveGraph-style "invalidation and refetch"
/// loop:
/// - Clients subscribe to a query.
/// - The Edge computes a set of `QueryHint` values that represent the query's dependencies.
/// - When the WAL invalidator emits a `QueryHint` (e.g. `comments:id:123`), the Edge can quickly
///   find which subscriptions are affected and should be refetched.
///
/// A note on terminology used throughout the Edge:
///
/// - **Base hints** are derived from the subscription's query parameters, without executing SQL.
///   Example: `CommentsByDocument { document_id: "doc1" }` produces `comments:document_id:doc1`.
///
/// - **Row-derived hints** are derived from the *current query result* after fetching from Postgres.
///   Example: if `CommentsByDocument(doc1)` currently returns comment IDs `[100, 101]`, then we also
///   register `comments:id:100` and `comments:id:101`.
///
/// Base hints are stable for the life of the subscription; row-derived hints can change on every
/// refetch and must be updated via `update_hints`.
///
/// Internally we keep two indexes:
/// - `inverted_index`: `hint -> {subscription_ids...}` for fast invalidation fanout.
/// - `subscription_hints`: `subscription_id -> {hints...}` as a reverse index.
///
/// The reverse index (`subscription_hints`) is essential for correctness: whenever a subscription is removed
/// (unsubscribe/disconnect) or a query result changes (row-derived hints added/removed), we must
/// unregister stale hint mappings to avoid memory leaks and "ghost" invalidations.
///
/// Note: this type only manages the *bookkeeping*. It does not perform refetching or I/O.
#[derive(Debug, Default)]
pub struct SubscriptionIndex {
    // hint -> subscriptions
    inverted_index: HashMap<QueryHint, HashSet<SubscriptionId>>,
    // subscription -> hints (reverse index)
    subscription_hints: HashMap<SubscriptionId, HashSet<QueryHint>>,
}

impl SubscriptionIndex {
    /// Register a new subscription and all of its hint dependencies.
    ///
    /// Call this when a client successfully subscribes (after we've computed the initial hint set).
    ///
    /// If you need to change the hint set for an existing subscription (e.g. after a refetch that
    /// changed the set of row-derived `comments:id:*` hints), use `update_hints`.
    pub fn subscribe(
        &mut self,
        subscription_id: SubscriptionId,
        hints: HashSet<QueryHint>,
    ) -> Result<(), SubscriptionIndexError> {
        // Design choice: we treat duplicate subscription IDs as an error to prevent
        // silent hint leaks. Callers should unsubscribe first or use `update_hints`.
        if self.subscription_hints.contains_key(&subscription_id) {
            return Err(SubscriptionIndexError::AlreadySubscribed(subscription_id));
        }

        for hint in &hints {
            self.inverted_index
                .entry(hint.clone())
                .or_default()
                .insert(subscription_id.clone());
        }

        self.subscription_hints.insert(subscription_id, hints);
        Ok(())
    }

    /// Unregister a subscription and remove all of its hint mappings.
    ///
    /// Call this when a client unsubscribes or disconnects.
    ///
    /// Returns the hints that were removed (useful for debugging/tests).
    pub fn unsubscribe(&mut self, subscription_id: &str) -> Option<HashSet<QueryHint>> {
        let hints = self.subscription_hints.remove(subscription_id)?;

        for hint in &hints {
            if let Some(subs) = self.inverted_index.get_mut(hint) {
                subs.remove(subscription_id);
                if subs.is_empty() {
                    self.inverted_index.remove(hint);
                }
            }
        }

        Some(hints)
    }

    /// Update the hint set for an existing subscription.
    ///
    /// This is the key operation that prevents "ghost invalidations" when we use row-derived hints.
    ///
    /// Typical usage:
    /// 1. Edge receives an invalidation.
    /// 2. Edge refetches the subscription's query result.
    /// 3. Edge derives a new hint set (base hints + row hints).
    /// 4. Edge calls `update_hints` to remove stale hint mappings and add new ones.
    pub fn update_hints(
        &mut self,
        subscription_id: &str,
        new_hints: HashSet<QueryHint>,
    ) -> Result<(), SubscriptionIndexError> {
        let Some(old_hints) = self.subscription_hints.get(subscription_id) else {
            return Err(SubscriptionIndexError::UnknownSubscription(
                subscription_id.to_string(),
            ));
        };

        let removed: Vec<QueryHint> = old_hints.difference(&new_hints).cloned().collect();
        let added: Vec<QueryHint> = new_hints.difference(old_hints).cloned().collect();

        for hint in &removed {
            if let Some(subs) = self.inverted_index.get_mut(hint) {
                subs.remove(subscription_id);
                if subs.is_empty() {
                    self.inverted_index.remove(hint);
                }
            }
        }

        for hint in &added {
            self.inverted_index
                .entry(hint.clone())
                .or_default()
                .insert(subscription_id.to_string());
        }

        self.subscription_hints
            .insert(subscription_id.to_string(), new_hints);

        Ok(())
    }

    /// Look up which subscriptions are affected by a specific invalidation hint.
    ///
    /// Call this when the invalidator emits a hint and we want to determine which subscriptions to
    /// refetch.
    pub fn subscriptions_for_hint(&self, hint: &QueryHint) -> HashSet<SubscriptionId> {
        self.inverted_index.get(hint).cloned().unwrap_or_default()
    }

    pub fn hints_for_subscription(&self, subscription_id: &str) -> Option<&HashSet<QueryHint>> {
        self.subscription_hints.get(subscription_id)
    }

    pub fn is_empty(&self) -> bool {
        self.inverted_index.is_empty() && self.subscription_hints.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hint(table: &str, column: &str, value: &str) -> QueryHint {
        QueryHint {
            table: table.to_string(),
            column: column.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn subscribe_registers_hints_in_both_indexes() {
        let mut idx = SubscriptionIndex::default();

        let sub_id = "sub_1".to_string();
        let hints = HashSet::from([
            hint("comments", "id", "123"),
            hint("comments", "document_id", "doc1"),
        ]);

        idx.subscribe(sub_id.clone(), hints.clone()).unwrap();

        assert_eq!(idx.hints_for_subscription(&sub_id), Some(&hints));
        let comment_hint = hint("comments", "id", "123");
        let doc_hint = hint("comments", "document_id", "doc1");

        assert_eq!(
            idx.subscriptions_for_hint(&comment_hint),
            HashSet::from([sub_id.clone()])
        );
        assert_eq!(
            idx.subscriptions_for_hint(&doc_hint),
            HashSet::from([sub_id])
        );
    }

    #[test]
    fn unsubscribe_removes_subscription_from_all_hints() {
        let mut idx = SubscriptionIndex::default();

        let comment_hint = hint("comments", "id", "123");
        let doc1_hint = hint("comments", "document_id", "doc1");
        let doc2_hint = hint("comments", "document_id", "doc2");

        idx.subscribe(
            "sub_1".to_string(),
            HashSet::from([comment_hint.clone(), doc1_hint.clone()]),
        )
        .unwrap();

        idx.subscribe(
            "sub_2".to_string(),
            HashSet::from([comment_hint.clone(), doc2_hint.clone()]),
        )
        .unwrap();

        let removed = idx.unsubscribe("sub_1").expect("sub_1 should exist");
        assert!(removed.contains(&comment_hint));

        // `comments:id:123` should still exist due to sub_2.
        assert_eq!(
            idx.subscriptions_for_hint(&comment_hint),
            HashSet::from(["sub_2".to_string()])
        );

        // `comments:document_id:doc1` should be fully removed.
        assert!(idx.subscriptions_for_hint(&doc1_hint).is_empty());
        assert!(idx.hints_for_subscription("sub_1").is_none());

        // `comments:document_id:doc2` should be present for sub_2
        assert_eq!(
            idx.subscriptions_for_hint(&doc2_hint),
            HashSet::from(["sub_2".to_string()])
        );
        assert_eq!(
            idx.hints_for_subscription("sub_2"),
            Some(&HashSet::from([doc2_hint, comment_hint]))
        );
    }

    #[test]
    fn update_hints_unregisters_removed_and_registers_added() {
        let mut idx = SubscriptionIndex::default();

        idx.subscribe(
            "sub_1".to_string(),
            HashSet::from([
                hint("comments", "id", "100"),
                hint("comments", "id", "101"),
                hint("comments", "document_id", "doc1"),
            ]),
        )
        .unwrap();

        idx.update_hints(
            "sub_1",
            HashSet::from([
                hint("comments", "id", "101"),
                hint("comments", "id", "102"),
                hint("comments", "document_id", "doc1"),
            ]),
        )
        .unwrap();

        assert!(idx
            .subscriptions_for_hint(&hint("comments", "id", "100"))
            .is_empty());
        assert_eq!(
            idx.subscriptions_for_hint(&hint("comments", "id", "101")),
            HashSet::from(["sub_1".to_string()])
        );
        assert_eq!(
            idx.subscriptions_for_hint(&hint("comments", "id", "102")),
            HashSet::from(["sub_1".to_string()])
        );
    }

    #[test]
    fn duplicate_subscribe_errors() {
        let mut idx = SubscriptionIndex::default();

        idx.subscribe(
            "sub_1".to_string(),
            HashSet::from([hint("comments", "id", "123")]),
        )
        .unwrap();

        let err = idx
            .subscribe(
                "sub_1".to_string(),
                HashSet::from([hint("comments", "id", "456")]),
            )
            .unwrap_err();

        assert!(matches!(err, SubscriptionIndexError::AlreadySubscribed(_)));
    }

    #[test]
    fn unsubscribe_unknown_is_none() {
        let mut idx = SubscriptionIndex::default();
        assert!(idx.unsubscribe("nope").is_none());
        assert!(idx.is_empty());
    }

    #[test]
    fn updae_hints_unknown_errors() {
        let mut idx = SubscriptionIndex::default();
        let err = idx
            .update_hints("sub_1", HashSet::from([hint("comments", "id", "123")]))
            .unwrap_err();

        assert!(matches!(
            err,
            SubscriptionIndexError::UnknownSubscription(_)
        ));
    }
}
