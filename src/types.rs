use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub type ClientId = String;
pub type DocumentId = String;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Join { document_id: DocumentId },
    SendMessage { text: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Joined {
        client_id: ClientId,
        document_id: DocumentId,
    },
    Message {
        from: ClientId,
        text: String,
        timestamp: u64,
    },
    Error {
        message: String,
    },
}

impl ServerMessage {
    pub fn new_message(from: ClientId, text: String) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64;

        ServerMessage::Message {
            from,
            text,
            timestamp,
        }
    }
}
