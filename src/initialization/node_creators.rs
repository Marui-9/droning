use super::network_initializer::Runnable;
use crate::{
    application::{
        simulation_controller_messages::{HostCommand, HostEvent},
        turn_handler::{self, TurnHandlerArc},
    },
    client::{web_browser::WebBrowser, ChatClient},
    client_factories, drone_factories,
    server::{chat_server::ChatServer, media_server::MediaServer, text_server::TextServer},
    server_factories,
};
use crossbeam_channel::{Receiver, Sender};
use d_r_o_n_e_drone::MyDrone as D_R_O_N_E;
use dr_ones::Drone as Dr_One;
use fungi_drone::FungiDrone;
use lockheedrustin_drone::LockheedRustin;
use rolling_drone::RollingDrone;
use rust_do_it::RustDoIt;
use rustafarian_drone::RustafarianDrone;
use rusty_drones::RustyDrone;
use std::collections::HashMap;
use wg_2024::{
    controller::{DroneCommand, DroneEvent},
    drone::Drone,
    network::NodeId,
    packet::Packet,
};
use LeDron_James::Drone as LeDrone;
use RF_drone::RustAndFurious;

pub trait DroneCreatorFunction {
    fn create_drone(
        &mut self,
        id: NodeId,
        controller_send: Sender<DroneEvent>,
        controller_recv: Receiver<DroneCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        pdr: f32,
    ) -> Box<dyn Runnable>;
}

impl<F> DroneCreatorFunction for F
where
    F: FnMut(
        NodeId,
        Sender<DroneEvent>,
        Receiver<DroneCommand>,
        Receiver<Packet>,
        HashMap<NodeId, Sender<Packet>>,
        f32,
    ) -> Box<dyn Runnable>,
{
    fn create_drone(
        &mut self,
        id: NodeId,
        controller_send: Sender<DroneEvent>,
        controller_recv: Receiver<DroneCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        pdr: f32,
    ) -> Box<dyn Runnable> {
        self(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
        )
    }
}

impl DroneCreatorFunction for Box<dyn DroneCreatorFunction> {
    fn create_drone(
        &mut self,
        id: NodeId,
        controller_send: Sender<DroneEvent>,
        controller_recv: Receiver<DroneCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        pdr: f32,
    ) -> Box<dyn Runnable> {
        self.as_mut().create_drone(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
        )
    }
}

pub trait ClientCreatorFunction {
    fn create_client(
        &mut self,
        id: NodeId,
        controller_send: Sender<HostEvent>,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        turn_handler: TurnHandlerArc,
    ) -> Box<dyn Runnable>;
}

impl<F> ClientCreatorFunction for F
where
    F: FnMut(
        NodeId,
        Sender<HostEvent>,
        Receiver<HostCommand>,
        Receiver<Packet>,
        HashMap<NodeId, Sender<Packet>>,
        TurnHandlerArc,
    ) -> Box<dyn Runnable>,
{
    fn create_client(
        &mut self,
        id: NodeId,
        controller_send: Sender<HostEvent>,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        turn_handler: TurnHandlerArc,
    ) -> Box<dyn Runnable> {
        self(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            turn_handler,
        )
    }
}

impl ClientCreatorFunction for Box<dyn ClientCreatorFunction> {
    fn create_client(
        &mut self,
        id: NodeId,
        controller_send: Sender<HostEvent>,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        turn_handler: TurnHandlerArc,
    ) -> Box<dyn Runnable> {
        self.as_mut().create_client(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            turn_handler,
        )
    }
}

pub trait ServerCreatorFunction {
    fn create_server(
        &mut self,
        id: NodeId,
        controller_send: Sender<HostEvent>,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Box<dyn Runnable>;
}

impl<F> ServerCreatorFunction for F
where
    F: FnMut(
        NodeId,
        Sender<HostEvent>,
        Receiver<HostCommand>,
        Receiver<Packet>,
        HashMap<NodeId, Sender<Packet>>,
    ) -> Box<dyn Runnable>,
{
    fn create_server(
        &mut self,
        id: NodeId,
        controller_send: Sender<HostEvent>,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Box<dyn Runnable> {
        self(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
        )
    }
}

impl ServerCreatorFunction for Box<dyn ServerCreatorFunction> {
    fn create_server(
        &mut self,
        id: NodeId,
        controller_send: Sender<HostEvent>,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Box<dyn Runnable> {
        self.as_mut().create_server(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
        )
    }
}

pub trait DroneCreator {
    fn new(controller_send: Sender<DroneEvent>) -> Self;

    fn create_drone(
        &mut self,
        id: NodeId,
        controller_recv: Receiver<DroneCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        pdr: f32,
    ) -> Box<dyn Runnable>;

    fn create_disconnected_drone(
        &mut self,
        id: NodeId,
        controller_recv: Receiver<DroneCommand>,
        packet_recv: Receiver<Packet>,
        pdr: f32,
    ) -> Box<dyn Runnable> {
        self.create_drone(id, controller_recv, packet_recv, HashMap::new(), pdr)
    }
}

pub struct ActualDroneCreator {
    factories: Vec<Box<dyn DroneCreatorFunction>>,
    index: usize,
    controller_send: Sender<DroneEvent>,
}

impl ActualDroneCreator {
    pub fn current_factory_mut(&mut self) -> &mut Box<dyn DroneCreatorFunction> {
        &mut self.factories[self.index]
    }
}

impl DroneCreator for ActualDroneCreator {
    fn new(controller_send: Sender<DroneEvent>) -> Self {
        Self {
            factories: drone_factories!(
                RollingDrone,
                FungiDrone,
                LeDrone,
                RustDoIt,
                D_R_O_N_E,
                Dr_One,
                RustafarianDrone,
                LockheedRustin,
                RustAndFurious,
                RustyDrone
            ),
            index: 0,
            controller_send,
        }
    }
    fn create_drone(
        &mut self,
        id: NodeId,
        controller_recv: Receiver<DroneCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        pdr: f32,
    ) -> Box<dyn Runnable> {
        let controller_send = self.controller_send.clone();
        let drone = self.current_factory_mut().create_drone(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
        );
        self.index = (self.index + 1) % self.factories.len();
        drone
    }
}

pub trait ClientCreator {
    fn new(controller_send: Sender<HostEvent>) -> Self;
    fn create_client(
        &mut self,
        id: NodeId,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Box<dyn Runnable>;

    fn create_disconnected_client(
        &mut self,
        id: NodeId,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
    ) -> Box<dyn Runnable> {
        self.create_client(id, controller_recv, packet_recv, HashMap::new())
    }
}

pub struct ActualClientCreator {
    factories: Vec<Box<dyn ClientCreatorFunction>>,
    index: usize,
    controller_send: Sender<HostEvent>,
    turn_handler: TurnHandlerArc,
}

impl ActualClientCreator {
    pub fn current_factory_mut(&mut self) -> &mut Box<dyn ClientCreatorFunction> {
        &mut self.factories[self.index]
    }
}

impl ClientCreator for ActualClientCreator {
    fn new(controller_send: Sender<HostEvent>) -> Self {
        Self {
            factories: client_factories!(ChatClient, WebBrowser),
            index: 0,
            controller_send,
            turn_handler: turn_handler::create_turn_handler(),
        }
    }
    fn create_client(
        &mut self,
        id: NodeId,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Box<dyn Runnable> {
        let controller_send = self.controller_send.clone();
        let turn_handler = self.turn_handler.clone();
        let client = self.current_factory_mut().create_client(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            turn_handler,
        );
        self.index = (self.index + 1) % self.factories.len();
        client
    }
}

pub trait ServerCreator {
    fn new(controller_send: Sender<HostEvent>) -> Self;

    fn create_server(
        &mut self,
        id: NodeId,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Box<dyn Runnable>;

    fn create_disconnected_server(
        &mut self,
        id: NodeId,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
    ) -> Box<dyn Runnable> {
        self.create_server(id, controller_recv, packet_recv, HashMap::new())
    }
}

pub struct ActualServerCreator {
    factories: Vec<Box<dyn ServerCreatorFunction>>,
    index: usize,
    controller_send: Sender<HostEvent>,
}

impl ActualServerCreator {
    pub fn current_factory_mut(&mut self) -> &mut Box<dyn ServerCreatorFunction> {
        &mut self.factories[self.index]
    }
}

impl ServerCreator for ActualServerCreator {
    fn new(controller_send: Sender<HostEvent>) -> Self {
        Self {
            factories: server_factories!(ChatServer, TextServer, MediaServer),
            index: 0,
            controller_send,
        }
    }
    fn create_server(
        &mut self,
        id: NodeId,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Box<dyn Runnable> {
        let controller_send = self.controller_send.clone();
        let client = self.current_factory_mut().create_server(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
        );
        self.index = (self.index + 1) % self.factories.len();
        client
    }
}
