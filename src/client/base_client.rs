use crate::application::assembler::{Assembler, Disassembler};
use crate::application::routing::SourceRouter;
use crate::application::simulation_controller_messages::{HostCommand, HostEvent};
use crate::application::topology::node::{ApplicationType, Node, NodeType};
use crate::application::turn_handler::TurnHandlerArc;
use crate::initialization::network_initializer::Runnable;
use crate::message::base_message::{Message, Request, Response};
use crossbeam_channel::{bounded, select, Receiver, Sender, TryRecvError};
use rand::random;
use std::collections::HashMap;
use std::fmt::Display;
use std::thread;
use std::thread::JoinHandle;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{
    FloodRequest, Nack, NackType, NodeType as SimpleNodeType, Packet, PacketType,
};
use PacketType::{Ack as Quack, Nack as Quacknt, *};

use super::card::Card;
use super::client_game::ClientGame;

pub trait ClientBehaviour: Send + Sized + 'static {
    type RequestType: Request + Display;
    type ResponseType: Response + Display;
    fn cards() -> Vec<Card<Self>>;
    fn on_response_received(&mut self, response: Message<Self::ResponseType>);
    fn application_type() -> ApplicationType;
}

pub struct Client<B: ClientBehaviour> {
    pub(crate) behaviour: B,
    pub(crate) id: NodeId,
    assembler: Assembler<B::ResponseType>,
    disassembler: Disassembler<B::RequestType>,
    router: SourceRouter,
    controller_send: Sender<HostEvent>,
    controller_recv: Receiver<HostCommand>,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    active: bool,
    card_receiver: Receiver<Card<B>>,
    cards_join_handle: Option<JoinHandle<()>>,
}

impl<B> Client<B>
where
    B: ClientBehaviour,
{
    pub fn new(
        id: NodeId,
        controller_send: Sender<HostEvent>,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        turn_handler: TurnHandlerArc,
        behaviour: B,
    ) -> Self {
        let (sender, receiver) = bounded(0);
        Self {
            behaviour,
            id,
            assembler: Assembler::new(),
            disassembler: Disassembler::new(),
            router: SourceRouter::new(Node::new(id, NodeType::Client(B::application_type()))),
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            card_receiver: receiver,
            active: false,
            cards_join_handle: Some(ClientGame::start_thread(id, sender, turn_handler)),
        }
    }

    pub fn with_default_behaviour(
        id: NodeId,
        controller_send: Sender<HostEvent>,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        turn_handler: TurnHandlerArc,
    ) -> Self
    where
        B: Default,
    {
        Self::new(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            turn_handler,
            B::default(),
        )
    }

    pub(crate) fn send_request(&mut self, request: Message<B::RequestType>) -> bool {
        let session_id = request.session_id;
        let destination_id = request.destination_id;
        if !self.router.can_reach(destination_id) {
            return false;
        }
        self.controller_send
            .send(HostEvent::MessageSent(request.to_string_message()))
            .unwrap();
        let fragments = self.disassembler.disassembly(request);
        let packets = fragments
            .into_iter()
            .map(|frag| Packet {
                session_id,
                routing_header: self.router.get_best_route(destination_id).unwrap(),
                pack_type: PacketType::MsgFragment(frag),
            })
            .collect::<Vec<Packet>>();
        packets.into_iter().for_each(|packet| self.forward(packet));

        true
    }

    pub(crate) fn initiate_flood(&mut self) {
        let flood_id = random();
        self.controller_send
            .send(HostEvent::FloodInitiated(self.id, flood_id))
            .unwrap();
        let flood_request = Packet::new_flood_request(
            SourceRoutingHeader::empty_route(),
            self.new_session_id(),
            FloodRequest::initialize(flood_id, self.id, SimpleNodeType::Client),
        );
        for sender in self.packet_send.values() {
            self.send_flood_request(sender, flood_request.clone());
        }
    }

    fn send_flood_request(&self, sender: &Sender<Packet>, flood_request: Packet) {
        sender.send(flood_request).unwrap();
    }

    fn forward(&mut self, mut packet: Packet) {
        if let Some(next_hop) = packet.routing_header.next_hop() {
            if let Some(sender) = self.packet_send.get(&next_hop) {
                packet.routing_header.increase_hop_index();
                sender.send(packet).unwrap();
            }
        }
    }

    pub(crate) fn forget_topology(&mut self) {
        self.router.forget_topology()
    }

    pub(crate) fn print_reachable_servers(&self) {
        self.router.print_reachable_servers();
    }

    pub fn new_session_id(&mut self) -> u64 {
        Disassembler::<B::RequestType>::transform_session_id(
            self.disassembler.new_session_id(),
            self.id,
        )
    }

    pub fn run(&mut self) {
        self.active = true;

        println!("Client {} started", self.id);

        while self.active {
            select! {
                recv(self.controller_recv) -> command => {
                    if let Ok(command) = command {
                        self.handle_command(command);
                    }
                }
                recv(self.packet_recv) -> packet => {
                    if let Ok(packet) = packet {
                        self.handle_packet_normal(packet);
                    }
                }
                recv(self.card_receiver) -> card => {
                    if let Ok(card) = card {
                        card.activate(self);
                        self.card_receiver.recv().unwrap();
                    }
                }
            }

            thread::yield_now();
        }

        if let Some(handle) = self.cards_join_handle.take() {
            handle.join().unwrap()
        }

        println!("Client {} stopped", self.id);
    }

    pub(crate) fn try_recv_packet(&self) -> Result<Packet, TryRecvError> {
        self.packet_recv.try_recv()
    }

    pub fn handle_command(&mut self, command: HostCommand) {
        match command {
            HostCommand::Crash => {
                self.stop();
            }
            HostCommand::AddConnectedDrone(id, sender) => {
                self.router.add_edge(self.id, id);
                self.packet_send.insert(id, sender);
            }
            HostCommand::RemoveConnectedDrone(id) => {
                self.router.remove_edge(self.id, id);
                self.packet_send.remove(&id);
            }
        }
    }

    pub fn handle_packet_normal(&mut self, packet: Packet) {
        self.router.update_graph(&packet);
        let session_id = packet.session_id;
        match packet.pack_type {
            MsgFragment(frag) => {
                let quack = Packet::new_ack(
                    self.router
                        .get_best_route(packet.routing_header.source().unwrap())
                        .unwrap(),
                    session_id,
                    frag.fragment_index,
                );
                self.forward(quack);
                if let Some(message_result) = self.assembler.insert_fragment(session_id, frag) {
                    match message_result {
                        Ok(message) => {
                            self.controller_send
                                .send(HostEvent::MessageReceived(message.to_string_message()))
                                .unwrap();
                            self.assembler.forget(session_id);
                            self.behaviour.on_response_received(message);
                        }
                        Err(_) => {
                            todo!("Send UnexpectedRecipient Quacknt");
                        }
                    }
                }
            }
            Quack(quack) => {
                self.disassembler
                    .forget_fragment(session_id, quack.fragment_index);
            }
            Quacknt(quacknt) => match quacknt.nack_type {
                NackType::ErrorInRouting(_) => {
                    self.retransmit(session_id, quacknt.fragment_index);
                }
                NackType::DestinationIsDrone => {}
                NackType::Dropped => {
                    self.retransmit(session_id, quacknt.fragment_index);
                }
                NackType::UnexpectedRecipient(id) => {
                    self.unwanted_node(&id);
                }
            },
            FloodRequest(mut request) => {
                request.increment(self.id, SimpleNodeType::Client);
                let response = request.generate_response(session_id);
                self.forward(response);
            }
            _ => {}
        }
    }

    pub fn unwanted_node(&mut self, node_id: &NodeId) {
        self.router.unwanted_node(node_id);
    }

    pub fn retransmit(&mut self, session_id: u64, fragment_index: u64) {
        if let Some(fragment) = self.disassembler.get_fragment(session_id, fragment_index) {
            let routing_header = self
                .router
                .get_best_route(self.disassembler.get_destination(session_id).unwrap())
                .unwrap();

            let packet = Packet::new_fragment(routing_header, session_id, fragment);

            self.forward(packet);
        }
    }

    pub fn stop(&mut self) {
        self.active = false;
    }

    pub(crate) fn calculate_routes(&mut self) -> usize {
        self.router.calculate_routes()
    }

    pub fn wait_for_response(
        &mut self,
        mut predicate: impl FnMut(&Message<B::ResponseType>) -> bool,
    ) -> Result<Message<B::ResponseType>, String> {
        loop {
            select! {
                recv(self.packet_recv) -> packet => {
                    if let Ok(packet) = packet {
                        match packet.pack_type {
                            PacketType::MsgFragment(frag) => {
                                self.router.update_graph(&(&packet.routing_header, &frag));
                                if let Some(Ok(message)) = self.assembler.insert_fragment(packet.session_id, frag) {
                                    self.controller_send.send(HostEvent::MessageReceived(message.to_string_message())).unwrap();
                                    self.assembler.forget(packet.session_id);
                                    if predicate(&message) {
                                        break Ok(message);
                                    }
                                    self.behaviour.on_response_received(message);
                                }
                            }
                            Quacknt(Nack {
                                nack_type: NackType::UnexpectedRecipient(_),
                                ..
                            }) => {
                                self.handle_packet_normal(packet);
                                break Err("This is the wrong kind of Server".to_string());
                            }
                            _ => {
                                self.handle_packet_normal(packet);
                            }
                        }
                    }
                }
            }
        }
    }
}

impl<B: ClientBehaviour> Runnable for Client<B> {
    fn run(&mut self) {
        self.run();
    }
}
