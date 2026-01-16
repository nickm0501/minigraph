use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

pub type ClientId = String;
pub type DocumentId = String;

#[derive(Debug)]
pub enum WebSocketError {
    InvalidMessage(String),
    NotInRoom,
    SendFailed,
    ConnectionError(String),
}

impl fmt::Display for WebSocketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WebSocketError::InvalidMessage(msg) => write!(f, "Invalid message: {}", msg),
            WebSocketError::NotInRoom => write!(f, "Must join a room before sending messages"),
            WebSocketError::SendFailed => write!(f, "Failed to send message to client"),
            WebSocketError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
        }
    }
}

impl std::error::Error for WebSocketError {}

#[derive(Debug)]
pub enum RoomCommandError {
    ChannelFull,
    ChannelClosed,
}

impl fmt::Display for RoomCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoomCommandError::ChannelFull => write!(f, "Room command channel is full"),
            RoomCommandError::ChannelClosed => write!(f, "Room command channel is closed"),
        }
    }
}

impl std::error::Error for RoomCommandError {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Join {
        document_id: DocumentId,
    },
    SendMessage {
        text: String,
    },
    SendMessageTo {
        document_id: DocumentId,
        text: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
