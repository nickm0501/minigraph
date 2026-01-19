use crate::wal::{QueryHint, TupleData, Value, WalEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HintGenError {
    MissingColumn {
        table: &'static str,
        column: &'static str,
    },
}

pub fn generate_invalidation_hints(event: &WalEvent) -> Result<Vec<QueryHint>, HintGenError> {
    match event {
        WalEvent::Insert {
            relation_name,
            new_tuple,
            ..
        } => match relation_name.as_str() {
            "comments" => {
                let document_id = require_text_column("comments", new_tuple, "document_id")?;
                Ok(vec![QueryHint {
                    table: "comments".to_string(),
                    column: "document_id".to_string(),
                    value: document_id.to_string(),
                }])
            }
            "documents" => Ok(vec![]),
            _ => Ok(vec![]),
        },
        WalEvent::Update {
            relation_name,
            old_tuple,
            new_tuple,
            ..
        } => match relation_name.as_str() {
            "comments" => {
                let new_document_id = require_text_column("comments", new_tuple, "document_id")?;

                let old_document_id = old_tuple
                    .as_ref()
                    .and_then(|t| get_text_column(t, "document_id"));

                let mut hints = Vec::new();
                if old_document_id.is_some_and(|old| old != new_document_id) {
                    hints.push(QueryHint {
                        table: "comments".to_string(),
                        column: "document_id".to_string(),
                        value: old_document_id.unwrap().to_string(),
                    });
                }

                hints.push(QueryHint {
                    table: "comments".to_string(),
                    column: "document_id".to_string(),
                    value: new_document_id.to_string(),
                });

                Ok(hints)
            }
            "documents" => Ok(vec![]),
            _ => Ok(vec![]),
        },
        WalEvent::Delete {
            relation_name,
            old_tuple,
            ..
        } => match relation_name.as_str() {
            "comments" => {
                let document_id = require_text_column("comments", old_tuple, "document_id")?;
                Ok(vec![QueryHint {
                    table: "comments".to_string(),
                    column: "document_id".to_string(),
                    value: document_id.to_string(),
                }])
            }
            "documents" => {
                let id = require_text_column("documents", old_tuple, "id")?;
                Ok(vec![QueryHint {
                    table: "documents".to_string(),
                    column: "id".to_string(),
                    value: id.to_string(),
                }])
            }
            _ => Ok(vec![]),
        },
    }
}

fn get_text_column<'a>(tuple: &'a TupleData, column: &str) -> Option<&'a str> {
    tuple.columns.get(column).and_then(|value| match value {
        Value::Text(v) => Some(v.as_str()),
        _ => None,
    })
}

fn require_text_column<'a>(
    table: &'static str,
    tuple: &'a TupleData,
    column: &'static str,
) -> Result<&'a str, HintGenError> {
    get_text_column(tuple, column).ok_or(HintGenError::MissingColumn { table, column })
}
