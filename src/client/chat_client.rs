use wg_2024::network::NodeId;

use super::card::{Card, Rarity};
use super::utils::input;
use crate::application::topology::node::ApplicationType;
use crate::client::base_client::{Client, ClientBehaviour};
use crate::message::base_message::Message;
use crate::message::chat_message::{ChatRequest, ChatResponse};

pub type ChatClient = Client<ChatClientBehaviour>;

#[derive(Default)]
pub struct ChatClientBehaviour {
    username: Option<String>,
    messages: Vec<(String, String)>,
}

impl ClientBehaviour for ChatClientBehaviour {
    type RequestType = ChatRequest;
    type ResponseType = ChatResponse;
    fn cards() -> Vec<Card<Self>> {
        vec![
            Card::new(
                "Client List",
                "List all clients registered on the Chat Server",
                Rarity::Common,
                |base_client: &mut ChatClient| {
                    let destination: NodeId = input("Enter the recipient's ID".to_string());
                    let session_id = base_client.new_session_id();
                    if !base_client.send_request(Message::new(
                        base_client.id,
                        destination,
                        session_id,
                        ChatRequest::ClientList,
                    )) {
                        println!("Failed to send the request");
                        return;
                    }

                    let response = base_client.wait_for_response(|response| {
                        matches!(response.content, ChatResponse::ClientList(_))
                    });
                    match response {
                        Ok(response) => {
                            if let ChatResponse::ClientList(clients) = response.content {
                                println!("Clients: {:?}", clients);
                            }
                        }
                        Err(err) => {
                            println!("{err}");
                        }
                    }
                },
            ),
            Card::new(
                "Register",
                "Register your username",
                Rarity::Common,
                |base_client: &mut ChatClient| {
                    let destination: NodeId = input("Enter the Chat Server's ID".to_string());
                    let username: String = input("Enter your username".to_string());
                    let session_id = base_client.new_session_id();
                    if !base_client.send_request(Message::new(
                        base_client.id,
                        destination,
                        session_id,
                        ChatRequest::Register(username.clone()),
                    )) {
                        println!("Failed to send the request");
                        return;
                    }

                    base_client.behaviour.username = Some(username);

                    let response = base_client.wait_for_response(|response| {
                        matches!(response.content, ChatResponse::ClientList(_))
                    });

                    match response {
                        Ok(response) => {
                            if let ChatResponse::ClientList(clients) = response.content {
                                println!("Clients: {:?}", clients);
                            }
                        }
                        Err(err) => {
                            println!("{err}");
                        }
                    }
                },
            ),
            Card::new(
                "Send Message",
                "Send a message to another client",
                Rarity::Common,
                |base_client: &mut ChatClient| match base_client.behaviour.username.clone() {
                    Some(username) => {
                        let server_id: NodeId = input("Enter the Chat Server's ID".to_string());
                        let to: String = input("Enter the recipient's username".to_string());
                        let content: String = input("Enter the message".to_string());
                        let session_id = base_client.new_session_id();
                        if !base_client.send_request(Message::new(
                            base_client.id,
                            server_id,
                            session_id,
                            ChatRequest::SendMessage {
                                from: username,
                                to,
                                message: content,
                            },
                        )) {
                            println!("Failed to send the request");
                        }

                        // TODO: Wait for response (it could receive a NotFound response)
                    }
                    None => {
                        println!("You need to register first!");
                    }
                },
            ),
            Card::new(
                "Read Messages",
                "Read all messages received",
                Rarity::Common,
                |base_client: &mut ChatClient| {
                    for (from, message) in base_client.behaviour.messages.drain(..) {
                        println!("From: {} - Message: {}", from, message);
                    }
                },
            ),
        ]
    }

    fn on_response_received(&mut self, response: Message<ChatResponse>) {
        if let ChatResponse::MessageFrom { from, message } = response.content {
            self.messages.push((from, message));
        }
    }

    fn application_type() -> ApplicationType {
        ApplicationType::Chat
    }
}
