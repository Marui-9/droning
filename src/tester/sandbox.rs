use crate::application::simulation_controller_messages::{HostCommand, HostEvent};
use crate::initialization::network_initializer::{
    create_simulation, parse_topology_file, spawn_threads, NetworkNode, Runnable,
};
use crate::initialization::node_creators::{ClientCreator, DroneCreator, ServerCreator};
use crossbeam_channel::{unbounded, Receiver, Sender};
use graph::ReferenceGraph;
use rand::{random, thread_rng, Rng};
use std::collections::{HashMap, HashSet};
use std::mem;
use wg_2024::config::Client;
use wg_2024::controller::DroneCommand;
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;

pub trait TestFunction: Send {
    fn call(
        &mut self,
        id: NodeId,
        controller_send: Sender<HostEvent>,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    );
}

impl<F> TestFunction for F
where
    F: FnMut(
            NodeId,
            Sender<HostEvent>,
            Receiver<HostCommand>,
            Receiver<Packet>,
            HashMap<NodeId, Sender<Packet>>,
        ) + Send,
{
    fn call(
        &mut self,
        id: NodeId,
        controller_send: Sender<HostEvent>,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) {
        self(
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
        );
    }
}

pub struct TestNodeInstructions {
    id: NodeId,
    connected_drone_ids: Vec<NodeId>,
    node_behaviour: Box<dyn TestFunction>,
}

impl TestNodeInstructions {
    pub fn with_node_id(
        id: NodeId,
        connected_drone_ids: &[NodeId],
        node_behaviour: impl TestFunction + 'static,
    ) -> Self {
        TestNodeInstructions {
            id,
            connected_drone_ids: connected_drone_ids.to_vec(),
            node_behaviour: Box::new(node_behaviour),
        }
    }

    pub fn with_random_id(
        connected_drone_ids: &[NodeId],
        node_behaviour: impl TestFunction + 'static,
    ) -> Self {
        TestNodeInstructions::with_node_id(random(), connected_drone_ids, node_behaviour)
    }
}

struct TestNode {
    id: NodeId,
    controller_send: Sender<HostEvent>,
    controller_recv: Receiver<HostCommand>,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    node_behaviour: Box<dyn TestFunction + 'static>,
}

impl Runnable for TestNode {
    fn run(&mut self) {
        let mut old_self = mem::replace(
            self,
            TestNode {
                id: 0,
                controller_send: unbounded().0,
                controller_recv: unbounded().1,
                packet_recv: unbounded().1,
                packet_send: HashMap::new(),
                node_behaviour: Box::new(|_, _, _, _, _| {}),
            },
        );

        old_self.node_behaviour.call(
            old_self.id,
            old_self.controller_send,
            old_self.controller_recv,
            old_self.packet_recv,
            old_self.packet_send,
        );
    }
}

#[allow(unused)]
pub enum PDRPolicy {
    Zero,
    Gentle,
    Medium,
    Severe,
    Constant(f32),
    Uniform(f32, f32),
    Unchanged,
}

impl PDRPolicy {
    fn get_pdr(&self, original: f32) -> f32 {
        match self {
            PDRPolicy::Zero => 0.0,
            PDRPolicy::Gentle => thread_rng().gen_range(0.0..0.1),
            PDRPolicy::Medium => thread_rng().gen_range(0.1..0.5),
            PDRPolicy::Severe => thread_rng().gen_range(0.5..0.75),
            PDRPolicy::Constant(pdr) => *pdr,
            PDRPolicy::Uniform(min, max) => thread_rng().gen_range(*min..*max),
            PDRPolicy::Unchanged => original,
        }
    }
}

struct TestHostCreator<CC>
where
    CC: ClientCreator,
{
    controller_send: Sender<HostEvent>,
    test_nodes: HashMap<NodeId, TestNodeInstructions>,
    base_client_creator: CC,
}

impl<CC> TestHostCreator<CC>
where
    CC: ClientCreator,
{
    pub fn with_test_nodes(
        controller_send: Sender<HostEvent>,
        test_nodes: HashMap<NodeId, TestNodeInstructions>,
    ) -> Self {
        Self {
            controller_send: controller_send.clone(),
            test_nodes,
            base_client_creator: CC::new(controller_send),
        }
    }
}

impl<CC> ClientCreator for TestHostCreator<CC>
where
    CC: ClientCreator,
{
    fn new(controller_send: Sender<HostEvent>) -> Self {
        Self::with_test_nodes(controller_send, HashMap::new())
    }

    fn create_client(
        &mut self,
        id: NodeId,
        controller_recv: Receiver<HostCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Box<dyn Runnable> {
        if let Some(test_node) = self.test_nodes.remove(&id) {
            Box::new(TestNode {
                id,
                controller_send: self.controller_send.clone(),
                controller_recv,
                packet_recv,
                packet_send,
                node_behaviour: test_node.node_behaviour,
            })
        } else {
            self.base_client_creator
                .create_client(id, controller_recv, packet_recv, packet_send)
        }
    }
}

pub fn create_test_environment<DC, CC, SC>(
    topology_file_path: &str,
    test_nodes: Vec<TestNodeInstructions>,
    pdr_policy: PDRPolicy,
) where
    DC: DroneCreator,
    CC: ClientCreator,
    SC: ServerCreator,
{
    let mut config = parse_topology_file(topology_file_path);
    let mut test_nodes = test_nodes
        .into_iter()
        .map(|node| (node.id, node))
        .collect::<HashMap<_, _>>();
    let test_nodes_ids = test_nodes.keys().cloned().collect::<HashSet<_>>();

    for test_node in test_nodes.values_mut() {
        let drone_ids = config.drone.iter().map(|drone| drone.id);
        let client_ids = config.client.iter().map(|client| client.id);
        let server_ids = config.server.iter().map(|server| server.id);
        let mut ids = drone_ids.chain(client_ids).chain(server_ids);
        while ids.any(|id| id == test_node.id) {
            test_node.id = rand::random();
        }
        config.client.push(Client {
            id: test_node.id,
            connected_drone_ids: test_node.connected_drone_ids.clone(),
        });
        let connected_ids = test_node.connected_drone_ids.clone();
        for drone in config.drone.iter_mut() {
            drone.pdr = pdr_policy.get_pdr(drone.pdr);
            if connected_ids.contains(&drone.id) {
                drone.connected_node_ids.push(test_node.id);
            }
        }
        for client in config.client.iter_mut() {
            if connected_ids.contains(&client.id) {
                client.connected_drone_ids.push(test_node.id);
            }
        }
        for server in config.server.iter_mut() {
            if connected_ids.contains(&server.id) {
                server.connected_drone_ids.push(test_node.id);
            }
        }
    }

    let (drone_event_to_controller, drone_event_controller_recv) = unbounded();
    let (host_event_to_controller, host_event_controller_recv) = unbounded();

    let drone_creator = DC::new(drone_event_to_controller.clone());
    let client_creator =
        TestHostCreator::<CC>::with_test_nodes(host_event_to_controller.clone(), test_nodes);
    let server_creator = SC::new(host_event_to_controller.clone());

    let (info, runnables) = create_simulation(
        &config,
        drone_creator,
        client_creator,
        server_creator,
        drone_event_controller_recv,
        host_event_controller_recv,
    );

    let mut join_handles = spawn_threads(runnables);

    for id in test_nodes_ids.into_iter() {
        if let Some(handle) = join_handles.remove(&id) {
            handle.join().ok();
        }
    }

    for (id, node) in info.network_graph.iter() {
        for adj in info.network_graph.adjacents(id) {
            match node {
                NetworkNode::Drone { command_send, .. } => {
                    command_send.send(DroneCommand::RemoveSender(*adj)).ok();
                }
                NetworkNode::Client { command_send } | NetworkNode::Server { command_send } => {
                    command_send
                        .send(HostCommand::RemoveConnectedDrone(*adj))
                        .ok();
                }
            }
        }
        match node {
            NetworkNode::Drone { command_send, .. } => {
                command_send.send(DroneCommand::Crash).ok();
            }
            NetworkNode::Client { command_send } | NetworkNode::Server { command_send } => {
                command_send.send(HostCommand::Crash).ok();
            }
        }
    }

    for handle in join_handles.into_values() {
        handle.join().ok();
    }

    println!("Test ended");
}
