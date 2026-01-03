use std::fmt::Display;

use crate::message::base_message::{MessageContent, Request, Response};
use crate::message::media_message::{MediaRequest, MediaResponse};
use crate::message::text_message::{TextRequest, TextResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentRequest {
    TextRequest(TextRequest),
    MediaRequest(MediaRequest),
}
impl Display for ContentRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentRequest::TextRequest(text_request) => write!(f, "TextRequest({})", text_request),
            ContentRequest::MediaRequest(media_request) => {
                write!(f, "MediaRequest({})", media_request)
            }
        }
    }
}
impl MessageContent for ContentRequest {}
impl Request for ContentRequest {}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentResponse {
    TextResponse(TextResponse),
    MediaResponse(MediaResponse),
    ServiceNotProvided,
}
impl Display for ContentResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentResponse::TextResponse(text_response) => {
                write!(f, "TextResponse({})", text_response)
            }
            ContentResponse::MediaResponse(media_response) => {
                write!(f, "MediaResponse({})", media_response)
            }
            ContentResponse::ServiceNotProvided => write!(f, "ServiceNotProvided"),
        }
    }
}
impl MessageContent for ContentResponse {}
impl Response for ContentResponse {}
