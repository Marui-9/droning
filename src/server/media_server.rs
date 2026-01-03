use crate::application::topology::node::ApplicationType;
use crate::message::base_message::Message;
use crate::message::content_message::{ContentRequest, ContentResponse};
use crate::message::media_message::{MediaRequest, MediaResponse};
use crate::server::base_server::{Server, ServerBehaviour};
use std::collections::HashMap;
use std::fs;
use wg_2024::network::NodeId;

pub type MediaServer = Server<crate::server::media_server::MediaServerBehaviour>;
#[derive(Debug)]
pub struct MediaServerBehaviour {
    media_library: HashMap<String, Vec<u8>>,
}
impl Default for MediaServerBehaviour {
    fn default() -> Self {
        let media_library = fs::read_dir("./assets/medias")
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.ok()?;

                let hashtag_filename = format!(
                    "#{}",
                    entry.file_name().into_string().unwrap().replace(".png", "")
                );
                Some((hashtag_filename, fs::read(entry.path()).ok()?))
            })
            .collect();
        MediaServerBehaviour { media_library }
    }
}
impl ServerBehaviour for MediaServerBehaviour {
    type RequestType = ContentRequest;
    type ResponseType = ContentResponse;

    fn handle_request(
        &mut self,
        req: Message<Self::RequestType>,
        id: NodeId,
    ) -> Message<Self::ResponseType> {
        match req.content {
            ContentRequest::TextRequest(_) => {
                req.generate_response(ContentResponse::ServiceNotProvided)
            }
            ContentRequest::MediaRequest(active_request) => match active_request {
                MediaRequest::MediaList => {
                    let response = MediaResponse::MediaList(
                        self.media_library.keys().cloned().collect::<Vec<String>>(),
                    );
                    Message::new(
                        id,
                        req.source_id,
                        req.session_id,
                        ContentResponse::MediaResponse(response),
                    )
                }
                MediaRequest::Media(requested_id) => {
                    let response = if let Some(media) = self.media_library.get(&requested_id) {
                        MediaResponse::Media(media.clone())
                    } else {
                        MediaResponse::NotFound
                    };
                    Message::new(
                        id,
                        req.source_id,
                        req.session_id,
                        ContentResponse::MediaResponse(response),
                    )
                }
                MediaRequest::ExpandList => {
                    let mut scraper2 = crate::server::scraper::Scraper::new();
                    let html = scraper2.get_html("https://scrapeme.live/shop/").unwrap();
                    let urls = scraper2.get_urls(html);
                    scraper2.insert_urls(urls);
                    for (name, png) in scraper2.data {
                        let newkey = format!("#{}", name.clone().to_ascii_lowercase());
                        self.media_library.insert(newkey, png);
                    }
                    let response = MediaResponse::MediaList(
                        self.media_library.keys().cloned().collect::<Vec<String>>(),
                    );
                    Message::new(
                        id,
                        req.source_id,
                        req.session_id,
                        ContentResponse::MediaResponse(response),
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
fn test_medias() {
    let mut server = MediaServerBehaviour::default();
    let message = server.handle_request(
        Message::new(
            0,
            0,
            0,
            ContentRequest::MediaRequest(MediaRequest::MediaList),
        ),
        0,
    );
    println!("{:?}", message);
}
