use std::fmt::Display;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Error;
use wg_2024::network::NodeId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message<M: MessageContent> {
    pub source_id: NodeId,
    pub destination_id: NodeId,
    pub session_id: u64,
    pub content: M,
}

impl<M: MessageContent + Display> Display for Message<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            " {{ source_id: {}, destination_id: {}, session_id: {}, content: {} }}",
            self.source_id, self.destination_id, self.session_id, self.content
        )
    }
}

impl<M: MessageContent + Serialize> Message<M> {
    pub fn serialize(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

impl<M: MessageContent + DeserializeOwned> Message<M> {
    pub fn deserialize(serialized: String) -> Result<Self, Error> {
        serde_json::from_str(serialized.as_str())
    }
}

impl<M: MessageContent> Message<M> {
    pub fn new(source_id: NodeId, destination_id: NodeId, session_id: u64, content: M) -> Self {
        Message {
            source_id,
            destination_id,
            session_id,
            content,
        }
    }

    pub fn generate_response<R: Response>(&self, content: R) -> Message<R>
    where
        M: Request,
    {
        Message {
            source_id: self.destination_id,
            destination_id: self.source_id,
            session_id: self.session_id,
            content,
        }
    }

    pub fn to_string_message(&self) -> Message<String>
    where
        M: Display,
    {
        Message::new(
            self.source_id,
            self.destination_id,
            self.session_id,
            self.content.to_string(),
        )
    }
}

impl MessageContent for String {}

pub trait MessageContent {}

pub trait Request: Send + MessageContent + Serialize + DeserializeOwned {}
pub trait Response: Send + MessageContent + Serialize + DeserializeOwned {}
