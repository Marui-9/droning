use std::{
    collections::{HashMap, HashSet},
    thread,
    time::Duration,
};

use crossbeam_channel::{Receiver, Sender};
use rand::{random, thread_rng, Rng};
use wg_2024::{
    controller::{DroneCommand, DroneEvent},
    network::{NodeId, SourceRoutingHeader},
    packet::{Fragment, Packet},
};

use crate::{
    application::simulation_controller_messages::{HostCommand, HostEvent},
    message::base_message::Message,
};

use super::{
    network_initializer::Runnable,
    node_creators::{ClientCreator, DroneCreator, ServerCreator},
};

struct DummyDrone {
    id: NodeId,
    _packet_recv: Receiver<Packet>,
    command_recv: Receiver<DroneCommand>,
    event_send: Sender<DroneEvent>,
    neighbors: HashSet<NodeId>,
}

impl DummyDrone {
    pub fn new(
        id: NodeId,
        packet_recv: Receiver<Packet>,
        command_recv: Receiver<DroneCommand>,
        event_send: Sender<DroneEvent>,
        neighbors: HashSet<NodeId>,
    ) -> Self {
        Self {
            id,
            _packet_recv: packet_recv,
            command_recv,
            event_send,
            neighbors,
        }
    }
}

impl Runnable for DummyDrone {
    fn run(&mut self) {
        loop {
            thread::sleep(Duration::from_millis(thread_rng().gen_range(500..5000)));

            if let Ok(command) = self.command_recv.try_recv() {
                println!("Drone {} received command {:?}", self.id, command);

                match command {
                    DroneCommand::Crash => {
                        break;
                    }
                    DroneCommand::RemoveSender(sender) => {
                        self.neighbors.remove(&sender);
                    }
                    _ => {}
                }
            }

            if self.neighbors.is_empty() {
                continue;
            }
            if self.neighbors.is_empty() {
                continue;
            }
            if thread_rng().gen_bool(0.25) {
                if thread_rng().gen_bool(0.9) {
                    let neighbor_count = self.neighbors.len();
                    let pick = thread_rng().gen_range(0..neighbor_count);
                    let random_neighbor = *self.neighbors.iter().nth(pick).unwrap();
                    let packet = Packet::new_fragment(
                        SourceRoutingHeader::with_first_hop(vec![self.id, random_neighbor]),
                        random(),
                        Fragment::from_string(random(), 1, "Hello!".to_string()),
                    );
                    self.event_send.send(DroneEvent::PacketSent(packet)).ok();
                } else {
                    let neighbor_count = self.neighbors.len();
                    let pick = thread_rng().gen_range(0..neighbor_count);
                    let random_neighbor = *self.neighbors.iter().nth(pick).unwrap();
                    let packet = Packet::new_fragment(
                        SourceRoutingHeader::initialize(vec![random_neighbor, self.id]),
                        random(),
                        Fragment::from_string(random(), 1, "Hello!".to_string()),
                    );
                    self.event_send.send(DroneEvent::PacketDropped(packet)).ok();
                }
            }
        }

        println!("Drone {} crashed", self.id);
    }
}

pub struct DummyDroneCreator {
    controller_send: Sender<DroneEvent>,
}

impl DroneCreator for DummyDroneCreator {
    fn new(controller_send: Sender<DroneEvent>) -> Self {
        Self { controller_send }
    }

    fn create_drone(
        &mut self,
        id: NodeId,
        controller_recv: Receiver<DroneCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        _pdr: f32,
    ) -> Box<dyn Runnable> {
        Box::new(DummyDrone::new(
            id,
            packet_recv,
            controller_recv,
            self.controller_send.clone(),
            packet_send.keys().copied().collect(),
        ))
    }
}

struct DummyHost {
    id: NodeId,
    _packet_recv: Receiver<Packet>,
    command_recv: Receiver<HostCommand>,
    event_send: Sender<HostEvent>,
}

impl DummyHost {
    pub fn new(
        id: NodeId,
        packet_recv: Receiver<Packet>,
        command_recv: Receiver<HostCommand>,
        event_send: Sender<HostEvent>,
    ) -> Self {
        Self {
            id,
            _packet_recv: packet_recv,
            command_recv,
            event_send,
        }
    }
}

pub struct DummyHostCreator {
    controller_send: Sender<HostEvent>,
}

impl ClientCreator for DummyHostCreator {
    fn new(controller_send: Sender<HostEvent>) -> Self {
        Self { controller_send }
    }

    fn create_client(
        &mut self,
        id: NodeId,
        controller_recv: Receiver<crate::application::simulation_controller_messages::HostCommand>,
        packet_recv: Receiver<Packet>,
        _packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Box<dyn Runnable> {
        Box::new(DummyHost::new(
            id,
            packet_recv,
            controller_recv,
            self.controller_send.clone(),
        ))
    }
}

impl ServerCreator for DummyHostCreator {
    fn new(controller_send: Sender<HostEvent>) -> Self {
        Self { controller_send }
    }

    fn create_server(
        &mut self,
        id: NodeId,
        controller_recv: Receiver<crate::application::simulation_controller_messages::HostCommand>,
        packet_recv: Receiver<Packet>,
        _packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Box<dyn Runnable> {
        Box::new(DummyHost::new(
            id,
            packet_recv,
            controller_recv,
            self.controller_send.clone(),
        ))
    }
}

impl Runnable for DummyHost {
    fn run(&mut self) {
        loop {
            thread::sleep(Duration::from_millis(thread_rng().gen_range(500..5000)));

            if let Ok(command) = self.command_recv.try_recv() {
                println!("Host {} received command {:?}", self.id, command);

                if let HostCommand::Crash = command {
                    break;
                }
            }

            if thread_rng().gen_bool(0.25) {
                if thread_rng().gen_bool(0.9) {
                    let message = Message::new(self.id, self.id, random(), "Hello!".to_string());
                    if random() {
                        self.event_send.send(HostEvent::MessageSent(message)).ok();
                    } else {
                        self.event_send
                            .send(HostEvent::MessageReceived(message))
                            .ok();
                    }
                } else {
                    self.event_send
                        .send(HostEvent::FloodInitiated(self.id, random()))
                        .ok();
                }
            }
        }

        println!("Host {} crashed", self.id);
    }
}
