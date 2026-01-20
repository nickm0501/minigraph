use std::collections::HashSet;

use crate::wal::QueryHint;

#[derive(Debug, Default)]
pub struct TransactionBuffer {
    hints: HashSet<QueryHint>,
}

impl TransactionBuffer {
    pub fn add_all(&mut self, hints: impl IntoIterator<Item = QueryHint>) {
        self.hints.extend(hints);
    }

    pub fn take(&mut self) -> HashSet<QueryHint> {
        std::mem::take(&mut self.hints)
    }

    pub fn is_empty(&self) -> bool {
        self.hints.is_empty()
    }
}
