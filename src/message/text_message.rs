use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextRequest {
    TextList,
    Text(String),
}

impl Display for TextRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextRequest::TextList => write!(f, "TextList"),
            TextRequest::Text(text) => write!(f, "Text({})", text),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextResponse {
    TextList(Vec<String>),
    Text(String),
    NotFound,
}

impl Display for TextResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextResponse::TextList(text_list) => write!(f, "TextList({:?})", text_list),
            TextResponse::Text(text) => write!(
                f,
                "Text({:?}...)",
                text.chars().take(10).collect::<String>()
            ),
            TextResponse::NotFound => write!(f, "NotFound"),
        }
    }
}
