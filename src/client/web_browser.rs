use std::fs;

use colored::Colorize;
use wg_2024::network::NodeId;

use super::{
    base_client::{Client, ClientBehaviour},
    card::Card,
};
use crate::{
    application::topology::node::ApplicationType,
    client::{card::Rarity, utils::input},
    message::{
        base_message::Message,
        content_message::{ContentRequest, ContentResponse},
        media_message::{MediaRequest, MediaResponse},
        text_message::{TextRequest, TextResponse},
    },
};

pub type WebBrowser = Client<WebBrowserBehaviour>;

#[derive(Default)]
pub struct WebBrowserBehaviour;

impl ClientBehaviour for WebBrowserBehaviour {
    type RequestType = ContentRequest;

    type ResponseType = ContentResponse;

    fn cards() -> Vec<Card<Self>> {
        vec![
            Card::new(
                "TextList",
                "List of text items",
                Rarity::Common,
                |base_client: &mut WebBrowser| {
                    let destination: NodeId = input("Input the recipient's ID".to_string());
                    let session_id = base_client.new_session_id();
                    if !base_client.send_request(Message::new(
                        base_client.id,
                        destination,
                        session_id,
                        ContentRequest::TextRequest(TextRequest::TextList),
                    )) {
                        println!("Failed to send the request");
                        return;
                    }

                    let response = base_client.wait_for_response(|response| {
                        matches!(
                            response.content,
                            ContentResponse::TextResponse(TextResponse::TextList(_))
                                | ContentResponse::ServiceNotProvided
                        )
                    });

                    match response {
                        Ok(response) => {
                            if let ContentResponse::TextResponse(TextResponse::TextList(list)) =
                                response.content
                            {
                                println!("The server contains the following texts:");
                                for (i, text) in list.iter().enumerate() {
                                    println!("{}. {}", i, text);
                                }
                            } else {
                                println!("The server does not provide text content");
                            }
                        }
                        Err(err) => {
                            println!("{err}");
                        }
                    }
                },
            ),
            Card::new(
                "Text Download",
                "Download a text item",
                Rarity::Rare,
                |base_client: &mut WebBrowser| {
                    let destination: NodeId = input("Input the recipient's ID".to_string());
                    let file_name: String = input("Input the file name".to_string());
                    let session_id = base_client.new_session_id();
                    if !base_client.send_request(Message::new(
                        base_client.id,
                        destination,
                        session_id,
                        ContentRequest::TextRequest(TextRequest::Text(file_name)),
                    )) {
                        println!("Failed to send the request");
                        return;
                    }

                    let response = base_client.wait_for_response(|response| {
                        matches!(
                            response.content,
                            ContentResponse::TextResponse(TextResponse::Text(_))
                                | ContentResponse::TextResponse(TextResponse::NotFound)
                                | ContentResponse::ServiceNotProvided
                        )
                    });

                    match response {
                        Ok(response) => match response.content {
                            ContentResponse::TextResponse(TextResponse::Text(text)) => {
                                println!("The server sent the following text:");

                                for line in text.lines() {
                                    for word in line.split(" ") {
                                        let colored_word = if word.starts_with("#") {
                                            word.cyan().underline()
                                        } else {
                                            word.normal()
                                        };
                                        print!("{} ", colored_word);
                                    }
                                }
                            }
                            ContentResponse::TextResponse(TextResponse::NotFound) => {
                                println!("The text item was not found");
                            }
                            ContentResponse::ServiceNotProvided => {
                                println!("The server does not provide text content");
                            }
                            _ => unreachable!(),
                        },
                        Err(err) => {
                            println!("{err}");
                        }
                    }
                },
            ),
            Card::new(
                "Media List",
                "List of media items",
                Rarity::Common,
                |base_client: &mut WebBrowser| {
                    let destination: NodeId = input("Input the recipient's ID".to_string());
                    let session_id = base_client.new_session_id();
                    if !base_client.send_request(Message::new(
                        base_client.id,
                        destination,
                        session_id,
                        ContentRequest::MediaRequest(MediaRequest::MediaList),
                    )) {
                        println!("Failed to send the request");
                        return;
                    }

                    let response = base_client.wait_for_response(|response| {
                        matches!(
                            response.content,
                            ContentResponse::MediaResponse(MediaResponse::MediaList(_))
                                | ContentResponse::ServiceNotProvided
                        )
                    });

                    match response {
                        Ok(response) => {
                            if let ContentResponse::MediaResponse(MediaResponse::MediaList(list)) =
                                response.content
                            {
                                println!("The server contains the following medias:");
                                for (i, media) in list.iter().enumerate() {
                                    println!("{}. {}", i, media);
                                }
                            } else {
                                println!("The server does not provide text content");
                            }
                        }
                        Err(err) => {
                            println!("{err}");
                        }
                    }
                },
            ),
            Card::new(
                "Download Media",
                "Download a media item",
                Rarity::Rare,
                |base_client: &mut WebBrowser| {
                    let destination: NodeId = input("Input the recipient's ID".to_string());
                    let file_name: String =
                        input("Input the file name (with # as prefix)".to_string());
                    let session_id = base_client.new_session_id();
                    if !base_client.send_request(Message::new(
                        base_client.id,
                        destination,
                        session_id,
                        ContentRequest::MediaRequest(MediaRequest::Media(file_name)),
                    )) {
                        println!("Failed to send the request");
                        return;
                    }

                    let response = base_client.wait_for_response(|response| {
                        matches!(
                            response.content,
                            ContentResponse::MediaResponse(MediaResponse::Media(_))
                                | ContentResponse::MediaResponse(MediaResponse::NotFound)
                                | ContentResponse::ServiceNotProvided
                        )
                    });

                    match response {
                        Ok(response) => match response.content {
                            ContentResponse::MediaResponse(MediaResponse::Media(media)) => {
                                fs::write("assets/temp.png", media).expect("Unable to write file");
                                open::that("assets/temp.png").expect("Unable to open file");
                            }
                            ContentResponse::MediaResponse(MediaResponse::NotFound) => {
                                println!("The media item was not found");
                            }
                            _ => {
                                println!("The server does not provide text content");
                            }
                        },
                        Err(err) => {
                            println!("{err}");
                        }
                    }
                },
            ),
            Card::new(
                "Upgrade",
                "Upgrade the catalog of media files",
                Rarity::Quacking,
                |base_client: &mut WebBrowser| {
                    let destination: NodeId = input("Input the recipient's ID".to_string());
                    let session_id = base_client.new_session_id();
                    if !base_client.send_request(Message::new(
                        base_client.id,
                        destination,
                        session_id,
                        ContentRequest::MediaRequest(MediaRequest::ExpandList),
                    )) {
                        println!("Failed to send the request");
                        return;
                    }

                    let response = base_client.wait_for_response(|response| {
                        matches!(
                            response.content,
                            ContentResponse::MediaResponse(MediaResponse::MediaList(_))
                                | ContentResponse::ServiceNotProvided
                        )
                    });

                    match response {
                        Ok(response) => {
                            if let ContentResponse::MediaResponse(MediaResponse::MediaList(list)) =
                                response.content
                            {
                                println!("The server's been upgraded!");
                                println!("The server now contains the following medias:");
                                for (i, media) in list.iter().enumerate() {
                                    println!("{}. {}", i, media);
                                }
                            } else {
                                println!("The server does not provide text content");
                            }
                        }
                        Err(err) => {
                            println!("{err}");
                        }
                    }
                },
            ),
        ]
    }

    fn on_response_received(&mut self, _response: Message<Self::ResponseType>) {}

    fn application_type() -> ApplicationType {
        ApplicationType::Content
    }
}
