use std::collections::{HashMap, HashSet};

use crate::types::DocumentId;
use crate::wal::QueryHint;

pub struct HintRouter;

impl HintRouter {
    pub fn route(hints: HashSet<QueryHint>) -> HashMap<DocumentId, Vec<QueryHint>> {
        let mut out: HashMap<DocumentId, Vec<QueryHint>> = HashMap::new();

        for hint in hints {
            if let Some(document_id) = Self::route_key(&hint) {
                out.entry(document_id).or_default().push(hint);
            }
        }

        out
    }

    fn route_key(hint: &QueryHint) -> Option<DocumentId> {
        match (hint.table.as_str(), hint.column.as_str()) {
            ("comments", "document_id") => Some(hint.value.clone()),
            ("documents", "id") => Some(hint.value.clone()),
            _ => None,
        }
    }
}

pub fn dedupe_routed_hints(map: &mut HashMap<DocumentId, Vec<QueryHint>>) {
    for hints in map.values_mut() {
        let mut seen: HashSet<QueryHint> = HashSet::new();
        hints.retain(|h| seen.insert(h.clone()));
    }
}
