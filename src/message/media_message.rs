use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaRequest {
    MediaList,
    Media(String),
    ExpandList,
}

impl Display for MediaRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaRequest::MediaList => write!(f, "MediaList"),
            MediaRequest::Media(name) => write!(f, "Media({})", name),
            MediaRequest::ExpandList => write!(f, "ExpandList"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaResponse {
    MediaList(Vec<String>),
    Media(Vec<u8>),
    NotFound,
}

impl Display for MediaResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaResponse::MediaList(media_list) => write!(f, "MediaList({:?})", media_list),
            MediaResponse::Media(media) => write!(
                f,
                "Media(0x{}...)",
                media
                    .iter()
                    .take(10)
                    .fold(String::new(), |acc, b| format!("{acc}{b:02x}"))
            ),
            MediaResponse::NotFound => write!(f, "NotFound"),
        }
    }
}
