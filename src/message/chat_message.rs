use crate::message::base_message::{MessageContent, Request, Response};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatRequest {
    ClientList,
    Register(String),
    SendMessage {
        from: String,
        to: String,
        message: String,
    },
}

impl Display for ChatRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatRequest::ClientList => write!(f, "ClientList"),
            ChatRequest::Register(name) => write!(f, "Register({})", name),
            ChatRequest::SendMessage { from, to, message } => {
                write!(
                    f,
                    "SendMessage(from: {}, to: {}, message: {})",
                    from, to, message
                )
            }
        }
    }
}
impl MessageContent for ChatRequest {}
impl Request for ChatRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatResponse {
    ClientList(Vec<String>),
    MessageFrom { from: String, message: String },
    DestinationNotFound,
}

impl Display for ChatResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatResponse::ClientList(clients) => write!(f, "ClientList({:?})", clients),
            ChatResponse::MessageFrom { from, message } => {
                write!(f, "MessageFrom(from: {}, message: {})", from, message)
            }
            ChatResponse::DestinationNotFound => write!(f, "DestinationNotFound"),
        }
    }
}
impl MessageContent for ChatResponse {}
impl Response for ChatResponse {}
