use std::collections::HashMap;

use mini_graph::wal::{
    generate_invalidation_hints, HintGenError, QueryHint, TupleData, Value, WalEvent,
};

fn tuple(columns: impl IntoIterator<Item = (&'static str, Value)>) -> TupleData {
    TupleData {
        columns: columns
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect::<HashMap<_, _>>(),
    }
}

#[test]
fn wal_hint_query_hint_to_key_formats_table_column_value() {
    let hint = QueryHint {
        table: "comments".to_string(),
        column: "document_id".to_string(),
        value: "doc123".to_string(),
    };

    assert_eq!(hint.to_key(), "comments:document_id:doc123");
}

#[test]
fn wal_hint_comment_insert_generates_document_hint() {
    let event = WalEvent::Insert {
        relation_id: 1,
        relation_name: "comments".to_string(),
        new_tuple: tuple([("document_id", Value::Text("doc123".to_string()))]),
    };

    let hints = generate_invalidation_hints(&event).unwrap();
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].to_key(), "comments:document_id:doc123");
}

#[test]
fn wal_hint_comment_update_generates_new_document_hint() {
    let event = WalEvent::Update {
        relation_id: 1,
        relation_name: "comments".to_string(),
        old_tuple: None,
        new_tuple: tuple([("document_id", Value::Text("doc456".to_string()))]),
    };

    let hints = generate_invalidation_hints(&event).unwrap();
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].to_key(), "comments:document_id:doc456");
}

#[test]
fn wal_hint_comment_delete_generates_document_hint_from_old_tuple() {
    let event = WalEvent::Delete {
        relation_id: 1,
        relation_name: "comments".to_string(),
        old_tuple: tuple([("document_id", Value::Text("doc789".to_string()))]),
    };

    let hints = generate_invalidation_hints(&event).unwrap();
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].to_key(), "comments:document_id:doc789");
}

#[test]
fn wal_hint_comment_update_changing_document_id_generates_both_hints() {
    let event = WalEvent::Update {
        relation_id: 1,
        relation_name: "comments".to_string(),
        old_tuple: Some(tuple([("document_id", Value::Text("doc_a".to_string()))])),
        new_tuple: tuple([("document_id", Value::Text("doc_b".to_string()))]),
    };

    let mut hints = generate_invalidation_hints(&event)
        .unwrap()
        .into_iter()
        .map(|h| h.to_key())
        .collect::<Vec<_>>();
    hints.sort();

    assert_eq!(
        hints,
        vec!["comments:document_id:doc_a", "comments:document_id:doc_b"]
    );
}

#[test]
fn wal_hint_document_delete_generates_id_hint() {
    let event = WalEvent::Delete {
        relation_id: 2,
        relation_name: "documents".to_string(),
        old_tuple: tuple([("id", Value::Text("doc999".to_string()))]),
    };

    let hints = generate_invalidation_hints(&event).unwrap();
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].to_key(), "documents:id:doc999");
}

#[test]
fn wal_hint_document_insert_generates_no_hints_in_phase_1() {
    let event = WalEvent::Insert {
        relation_id: 2,
        relation_name: "documents".to_string(),
        new_tuple: tuple([("id", Value::Text("doc111".to_string()))]),
    };

    let hints = generate_invalidation_hints(&event).unwrap();
    assert_eq!(hints, Vec::<QueryHint>::new());
}

#[test]
fn wal_hint_unknown_table_generates_no_hints() {
    let event = WalEvent::Insert {
        relation_id: 3,
        relation_name: "users".to_string(),
        new_tuple: tuple([]),
    };

    let hints = generate_invalidation_hints(&event).unwrap();
    assert_eq!(hints, Vec::<QueryHint>::new());
}

#[test]
fn wal_hint_missing_document_id_returns_error() {
    let event = WalEvent::Insert {
        relation_id: 1,
        relation_name: "comments".to_string(),
        new_tuple: tuple([("text", Value::Text("hello".to_string()))]),
    };

    let err = generate_invalidation_hints(&event).unwrap_err();
    assert_eq!(
        err,
        HintGenError::MissingColumn {
            table: "comments",
            column: "document_id",
        }
    );
}

#[test]
fn wal_hint_transaction_buffer_dedupes() {
    let mut tx = mini_graph::wal::TransactionBuffer::default();

    let hint = QueryHint {
        table: "comments".to_string(),
        column: "document_id".to_string(),
        value: "doc1".to_string(),
    };

    tx.add_all([hint.clone(), hint]);

    let taken = tx.take();
    assert_eq!(taken.len(), 1);
    assert!(tx.is_empty());
}

#[test]
fn wal_hint_router_routes_document_scoped_hints() {
    use std::collections::HashSet;

    let hints: HashSet<QueryHint> = [
        QueryHint {
            table: "comments".to_string(),
            column: "document_id".to_string(),
            value: "doc1".to_string(),
        },
        QueryHint {
            table: "documents".to_string(),
            column: "id".to_string(),
            value: "doc2".to_string(),
        },
    ]
    .into_iter()
    .collect();

    let mut routed = mini_graph::wal::HintRouter::route(hints);

    let doc1 = routed.remove("doc1").unwrap();
    let doc2 = routed.remove("doc2").unwrap();

    assert!(routed.is_empty());

    assert_eq!(doc1.len(), 1);
    assert_eq!(doc1[0].to_key(), "comments:document_id:doc1");

    assert_eq!(doc2.len(), 1);
    assert_eq!(doc2[0].to_key(), "documents:id:doc2");
}
