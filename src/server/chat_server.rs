use crate::application::topology::node::ApplicationType;
use crate::message::base_message::Message;
use crate::message::chat_message::{ChatRequest, ChatResponse};
use crate::server::base_server::{Server, ServerBehaviour};
use std::collections::HashMap;
use wg_2024::network::NodeId;

pub type ChatServer = Server<ChatServerBehaviour>;
#[derive(Default)]
pub struct ChatServerBehaviour {
    users: HashMap<String, NodeId>,
}
impl ServerBehaviour for ChatServerBehaviour {
    type RequestType = ChatRequest;
    type ResponseType = ChatResponse;

    fn handle_request(
        &mut self,
        req: Message<Self::RequestType>,
        id: NodeId,
    ) -> Message<Self::ResponseType> {
        match req.content {
            ChatRequest::ClientList => {
                let response =
                    ChatResponse::ClientList(self.users.keys().cloned().collect::<Vec<_>>());
                Message::new(id, req.source_id, req.session_id, response)
            }
            ChatRequest::Register(username) => {
                self.users.entry(username).or_insert(req.source_id);
                let response =
                    ChatResponse::ClientList(self.users.keys().cloned().collect::<Vec<_>>());
                Message::new(id, req.source_id, req.session_id, response)
            }
            ChatRequest::SendMessage { from, to, message } => {
                let response = Self::ResponseType::MessageFrom {
                    from: from.clone(),
                    message,
                };
                self.users.entry(from).or_insert(req.source_id);
                if let Some(destination) = self.users.get(&to) {
                    Message::new(id, *destination, req.session_id, response)
                } else {
                    let response = ChatResponse::DestinationNotFound;
                    Message::new(id, req.source_id, req.session_id, response)
                }
            }
        }
    }
    fn application_type() -> ApplicationType {
        ApplicationType::Chat
    }
}
