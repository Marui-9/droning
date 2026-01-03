use crate::application::topology::node::ApplicationType;
use crate::message::base_message::Message;
use crate::message::content_message::{ContentRequest, ContentResponse};
use crate::message::text_message::{TextRequest, TextResponse};
use crate::server::base_server::{Server, ServerBehaviour};
use std::collections::HashMap;
use std::fs;
use wg_2024::network::NodeId;

pub type TextServer = Server<TextServerBehaviour>;
pub struct TextServerBehaviour {
    text_library: HashMap<String, Vec<u8>>,
}
impl Default for TextServerBehaviour {
    fn default() -> Self {
        let text_library = fs::read_dir("./assets/texts")
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                Some((
                    entry.file_name().to_str()?.to_string(),
                    fs::read(entry.path()).ok()?,
                ))
            })
            .collect();
        TextServerBehaviour { text_library }
    }
}
impl ServerBehaviour for TextServerBehaviour {
    type RequestType = ContentRequest;
    type ResponseType = ContentResponse;

    fn handle_request(
        &mut self,
        req: Message<Self::RequestType>,
        id: NodeId,
    ) -> Message<Self::ResponseType> {
        match req.content {
            ContentRequest::MediaRequest(_) => {
                req.generate_response(ContentResponse::ServiceNotProvided)
            }
            ContentRequest::TextRequest(active_request) => match active_request {
                TextRequest::TextList => {
                    let response = TextResponse::TextList(
                        self.text_library.keys().cloned().collect::<Vec<String>>(),
                    );
                    Message::new(
                        id,
                        req.source_id,
                        req.session_id,
                        ContentResponse::TextResponse(response),
                    )
                }
                TextRequest::Text(requested_id) => {
                    let response;
                    if let Some(text) = self.text_library.get(&requested_id) {
                        let txt = text.clone();
                        let txt_string: String =
                            String::from_utf8(txt).expect("couldn't convert text to string");
                        response = TextResponse::Text(txt_string)
                    } else {
                        response = TextResponse::NotFound
                    }
                    Message::new(
                        id,
                        req.source_id,
                        req.session_id,
                        ContentResponse::TextResponse(response),
                    )
                }
            },
        }
    }

    fn application_type() -> ApplicationType {
        ApplicationType::Content
    }
}
#[test]
fn test_texts() {
    let mut server = TextServerBehaviour::default();
    let message = server.handle_request(
        Message::new(0, 0, 0, ContentRequest::TextRequest(TextRequest::TextList)),
        0,
    );
    println!("{:?}", message);
}
