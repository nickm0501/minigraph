use std::collections::HashMap;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct QueryHint {
    pub table: String,
    pub column: String,
    pub value: String,
}

impl QueryHint {
    pub fn to_key(&self) -> String {
        format!("{}:{}:{}", self.table, self.column, self.value)
    }
}

#[derive(Debug, Clone)]
pub enum WalEvent {
    Insert {
        relation_id: u32,
        relation_name: String,
        new_tuple: TupleData,
    },
    Update {
        relation_id: u32,
        relation_name: String,
        old_tuple: Option<TupleData>,
        new_tuple: TupleData,
    },
    Delete {
        relation_id: u32,
        relation_name: String,
        old_tuple: TupleData,
    },
}

#[derive(Debug, Clone, Default)]
pub struct TupleData {
    pub columns: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Null,
    Text(String),
    Int(i64),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(v) => Some(*v),
            _ => None,
        }
    }
}
