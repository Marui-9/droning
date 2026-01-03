use crate::application::assembler::{Assembler, Disassembler};
use crate::application::routing::SourceRouter;
use crate::application::simulation_controller_messages::{HostCommand, HostEvent};
use crate::application::topology::node::{ApplicationType, Node, NodeType};
use crate::initialization::network_initializer::Runnable;
use crate::message::base_message::{Message, Request, Response};
use crossbeam_channel::{Receiver, Sender};
use rand::random;
use std::collections::HashMap;
use std::fmt::Display;
use std::time::{Duration, Instant};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{
    FloodRequest, Nack, NackType, NodeType as SimpleNodeType, Packet, PacketType,
};

pub trait ServerBehaviour: Send {
    type RequestType: Request + Display;
    type ResponseType: Response + Display;
    fn handle_request(
        &mut self,
        req: Message<Self::RequestType>,
        source_id: NodeId,
    ) -> Message<Self::ResponseType>;
    fn application_type() -> ApplicationType;
}

pub struct Server<B: ServerBehaviour> {
    pub id: NodeId,
    behaviour: B,
    controller_send: Sender<HostEvent>,
    controller_recv: Receiver<HostCommand>,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    assembler: Assembler<B::RequestType>,
    disassembler: Disassembler<B::ResponseType>,
    router: SourceRouter,
    last_flood: Instant,
    last_route_update: Instant,
    active: bool,
}
impl<B: ServerBehaviour> Server<B> {
    pub fn new(
        id: NodeId,
        controller_send: Sender<HostEvent>,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        behaviour: B,
    ) -> Self {
        let router = SourceRouter::new(Node::new(id, NodeType::Server(B::application_type())));
        Server {
            id,
            controller_send,
            controller_recv,
            behaviour,
            packet_send,
            packet_recv,
            router,
            assembler: Assembler::new(),
            disassembler: Disassembler::new(),
            active: false,
            last_flood: Instant::now() - Duration::from_secs(30),
            last_route_update: Instant::now() - Duration::from_secs(25),
        }
    }
    pub fn with_default_behaviour(
        id: NodeId,
        controller_send: Sender<HostEvent>,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Self
    where
        B: Default,
    {
        Server::new(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            Default::default(),
        )
    }
    fn initiate_flood(&self) {
        let flood_id = random();
        let flood_request = Packet::new_flood_request(
            SourceRoutingHeader::empty_route(),
            random(),
            FloodRequest::initialize(flood_id, self.id, SimpleNodeType::Server),
        );
        let sent = HostEvent::FloodInitiated(self.id, flood_id);
        self.controller_send
            .send(sent)
            .expect("Failed to send flood event");
        for sender in self.packet_send.values() {
            sender.send(flood_request.clone()).expect("unable to send");
        }
    }
    fn run(&mut self) {
        self.active = true;
        println!("server {} is activated", self.id);
        while self.active {
            let flood_elapsed_seconds = self.last_flood.elapsed().as_secs();
            if flood_elapsed_seconds > 30 {
                self.last_flood = Instant::now();
                self.initiate_flood();
            }
            let route_update_elapsed_seconds = self.last_route_update.elapsed().as_secs();
            if route_update_elapsed_seconds > 30 {
                self.last_route_update = Instant::now();
                self.router.calculate_routes();
            }

            for command in self.gather_commands() {
                self.handle_command(command);
            }

            for packet in self.gather_packets() {
                self.handle_packet(packet);
            }
        }
    }

    fn gather_packets(&self) -> Vec<Packet> {
        self.packet_recv.try_iter().take(10).collect()
    }

    fn gather_commands(&self) -> Vec<HostCommand> {
        self.controller_recv.try_iter().take(10).collect()
    }

    fn stop(&mut self) {
        self.active = false;
    }

    fn handle_packet(&mut self, packet: Packet) {
        self.router.update_graph(&packet);
        let session_id = packet.session_id;
        let routing_header = packet.routing_header.clone();
        match packet.pack_type {
            PacketType::MsgFragment(frag) => {
                let fragment_index = frag.fragment_index;
                let ack =
                    Packet::new_ack(routing_header.get_reversed(), session_id, fragment_index);
                self.forward_packet(ack);
                if let Some(request_msg_frags) = self
                    .assembler
                    .insert_fragment(packet.session_id, frag.clone())
                {
                    match request_msg_frags {
                        Ok(message) => {
                            let received = HostEvent::MessageReceived(message.to_string_message());
                            self.send_event(received);
                            let response = self.behaviour.handle_request(message, self.id);
                            let sent = HostEvent::MessageSent(response.to_string_message());
                            self.send_event(sent);
                            self.send_response(response)
                        }
                        Err(_) => {
                            let packet = Packet::new_nack(
                                routing_header.get_reversed(),
                                session_id,
                                Nack {
                                    fragment_index: 0,
                                    nack_type: NackType::UnexpectedRecipient(self.id),
                                },
                            );
                            self.forward_packet(packet);
                        }
                    }
                }
            }
            PacketType::Ack(ack) => {
                self.disassembler
                    .forget_fragment(session_id, ack.fragment_index);
            }
            PacketType::Nack(nack_pack) => match nack_pack.nack_type {
                NackType::ErrorInRouting(_) => {
                    self.retransmit(session_id, nack_pack.fragment_index)
                }
                NackType::Dropped => {
                    self.retransmit(session_id, nack_pack.fragment_index);
                }
                NackType::DestinationIsDrone => {}
                NackType::UnexpectedRecipient(_) => {}
            },
            PacketType::FloodRequest(mut request) => {
                request.increment(self.id, SimpleNodeType::Server);
                let resp_packet = request.generate_response(session_id);
                self.forward_packet(resp_packet);
            }
            PacketType::FloodResponse(_) => {}
        }
    }
    fn handle_command(&mut self, command: HostCommand) {
        match command {
            HostCommand::AddConnectedDrone(id, sender) => {
                self.router.add_edge(self.id, id);
                self.packet_send.insert(id, sender);
            }
            HostCommand::RemoveConnectedDrone(id) => {
                self.router.remove_edge(self.id, id);
                self.packet_send.remove(&id);
            }
            HostCommand::Crash => {
                self.stop();
            }
        }
    }
    fn send_event(&mut self, event: HostEvent) {
        self.controller_send
            .send(event)
            .expect("unable to send events to host");
    }
    fn send_response(&mut self, response: Message<B::ResponseType>) {
        let destination = response.destination_id;
        let session = response.session_id;
        let fragments = self.disassembler.disassembly(response);
        for frag in fragments.into_iter() {
            if !self.router.can_reach(destination) {
                self.router.calculate_routes();
            }
            let packet = Packet::new_fragment(
                self.router.get_best_route(destination).unwrap(),
                session,
                frag,
            );
            self.forward_packet(packet);
        }
    }
    fn forward_packet(&self, mut packet: Packet) {
        let sender = self
            .packet_send
            .get(&packet.routing_header.next_hop().unwrap());
        packet.routing_header.increase_hop_index();
        if let Some(sender) = sender {
            sender.send(packet).expect("unable to send");
        }
    }
    fn retransmit(&mut self, session_id: u64, fragment_index: u64) {
        let frag = self
            .disassembler
            .get_fragment(session_id, fragment_index)
            .unwrap();
        let destination = self.disassembler.get_destination(session_id).unwrap();
        let to_retransmit = Packet::new_fragment(
            self.router.get_best_route(destination).unwrap(),
            session_id,
            frag,
        );
        self.forward_packet(to_retransmit);
    }
}

impl<B: ServerBehaviour> Runnable for Server<B> {
    fn run(&mut self) {
        self.run();
    }
}
