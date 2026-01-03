use crate::application::simulation_controller_messages::{HostCommand, HostEvent};
use crossbeam_channel::{unbounded, Receiver, Sender};
use graph::{AdjacencyVecGraph, ReferenceGraph};
use std::fmt::Display;
use std::{
    collections::HashMap,
    fs,
    thread::{self, JoinHandle},
};
use wg_2024::{
    config::Config,
    controller::{DroneCommand, DroneEvent},
    drone::Drone,
    network::NodeId,
    packet::Packet,
};

use super::{
    dummies::{DummyDroneCreator, DummyHostCreator},
    node_creators::{
        ActualClientCreator, ActualDroneCreator, ActualServerCreator, ClientCreator, DroneCreator,
        ServerCreator,
    },
};

#[derive(Debug, Clone)]
pub enum NetworkNode {
    Drone {
        pdr: f32,
        command_send: Sender<DroneCommand>,
    },
    Client {
        command_send: Sender<HostCommand>,
    },
    Server {
        command_send: Sender<HostCommand>,
    },
}

impl Display for NetworkNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                NetworkNode::Drone { .. } => "Drone",
                NetworkNode::Client { .. } => "Client",
                NetworkNode::Server { .. } => "Server",
            }
        )
    }
}

impl PartialEq for NetworkNode {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (NetworkNode::Drone { .. }, NetworkNode::Drone { .. })
                | (NetworkNode::Client { .. }, NetworkNode::Client { .. })
                | (NetworkNode::Server { .. }, NetworkNode::Server { .. })
        )
    }
}

type NetworkGraph = AdjacencyVecGraph<NodeId, NetworkNode>;
type ThreadHandles = HashMap<NodeId, JoinHandle<()>>;
type Runnables = HashMap<NodeId, Box<dyn Runnable>>;

pub struct ControllerInfo<DC, CC, SC>
where
    DC: DroneCreator,
    CC: ClientCreator,
    SC: ServerCreator,
{
    pub network_graph: NetworkGraph,
    pub drone_creator: DC,
    pub client_creator: CC,
    pub server_creator: SC,
    pub host_event_controller_recv: Receiver<HostEvent>,
    pub drone_event_controller_recv: Receiver<DroneEvent>,
    pub packet_senders: HashMap<NodeId, Sender<Packet>>,
    pub handles: ThreadHandles,
}

struct ControllerChannels {
    hosts_recv_command: HashMap<NodeId, Receiver<HostCommand>>,
    host_event_controller_recv: Receiver<HostEvent>,
    drones_recv_command: HashMap<NodeId, Receiver<DroneCommand>>,
    drone_event_controller_recv: Receiver<DroneEvent>,
}

pub trait Runnable: Send {
    fn run(&mut self);
}

impl<T: Drone> Runnable for T {
    fn run(&mut self) {
        self.run();
    }
}

pub fn start_actual_simulation(
    topology_path: &str,
) -> ControllerInfo<ActualDroneCreator, ActualClientCreator, ActualServerCreator> {
    start_generic_simulation(topology_path)
}

#[allow(unused)]
pub fn start_dummy_simulation(
    topology_path: &str,
) -> ControllerInfo<DummyDroneCreator, DummyHostCreator, DummyHostCreator> {
    start_generic_simulation(topology_path)
}

fn start_generic_simulation<DC, CC, SC>(topology_path: &str) -> ControllerInfo<DC, CC, SC>
where
    DC: DroneCreator,
    CC: ClientCreator,
    SC: ServerCreator,
{
    let config = parse_topology_file(topology_path);

    let (drone_event_to_controller, drone_event_controller_recv) = unbounded();
    let (host_event_to_controller, host_event_controller_recv) = unbounded();

    let drone_creator = DC::new(drone_event_to_controller.clone());
    let client_creator = CC::new(host_event_to_controller.clone());
    let server_creator = SC::new(host_event_to_controller.clone());

    let (mut controller_info, runnables) = create_simulation(
        &config,
        drone_creator,
        client_creator,
        server_creator,
        drone_event_controller_recv,
        host_event_controller_recv,
    );

    let handles = spawn_threads(runnables);

    controller_info.handles = handles;

    controller_info
}

pub fn create_simulation<DC: DroneCreator, CC: ClientCreator, SC: ServerCreator>(
    config: &Config,
    mut drone_creator: DC,
    mut client_creator: CC,
    mut server_creator: SC,
    drone_event_controller_recv: Receiver<DroneEvent>,
    host_event_controller_recv: Receiver<HostEvent>,
) -> (ControllerInfo<DC, CC, SC>, Runnables) {
    let (network_graph, mut controller_channels) = create_topology_graph(
        config,
        drone_event_controller_recv,
        host_event_controller_recv,
    );

    let (packet_senders, packet_receivers) = create_packet_channels(&network_graph);

    let runnables = create_runnables(
        &network_graph,
        &packet_senders,
        packet_receivers,
        &mut controller_channels,
        &mut drone_creator,
        &mut client_creator,
        &mut server_creator,
    );

    (
        ControllerInfo {
            network_graph,
            drone_creator,
            client_creator,
            server_creator,
            drone_event_controller_recv: controller_channels.drone_event_controller_recv,
            host_event_controller_recv: controller_channels.host_event_controller_recv,
            packet_senders,
            handles: HashMap::new(),
        },
        runnables,
    )
}

pub fn parse_topology_file(path: &str) -> Config {
    let config_data = fs::read_to_string(path).expect("Unable to read config file");
    let config: Config = toml::from_str(&config_data).expect("Unable to parse TOML");
    config
}

fn create_topology_graph(
    config: &Config,
    drone_event_controller_recv: Receiver<DroneEvent>,
    host_event_controller_recv: Receiver<HostEvent>,
) -> (NetworkGraph, ControllerChannels) {
    let mut graph = AdjacencyVecGraph::new();

    let (hosts_recv_command, drones_recv_command) = add_nodes(&mut graph, config);

    add_edges(&mut graph, config);

    (
        graph,
        ControllerChannels {
            host_event_controller_recv,
            drone_event_controller_recv,
            hosts_recv_command,
            drones_recv_command,
        },
    )
}

fn add_nodes(
    graph: &mut AdjacencyVecGraph<NodeId, NetworkNode>,
    config: &Config,
) -> (
    HashMap<NodeId, Receiver<HostCommand>>,
    HashMap<NodeId, Receiver<DroneCommand>>,
) {
    let mut host_receivers = HashMap::new();
    let mut drone_receivers = HashMap::new();

    for drone in config.drone.iter() {
        let (drone_command_send, drone_command_recv) = unbounded::<DroneCommand>();
        drone_receivers.insert(drone.id, drone_command_recv);
        graph.add_node(
            drone.id,
            NetworkNode::Drone {
                pdr: drone.pdr,
                command_send: drone_command_send,
            },
        );
    }

    for client in config.client.iter() {
        let (client_command_send, client_command_recv) = unbounded::<HostCommand>();
        host_receivers.insert(client.id, client_command_recv);
        graph.add_node(
            client.id,
            NetworkNode::Client {
                command_send: client_command_send,
            },
        );
    }

    for server in config.server.iter() {
        let (server_command_send, server_command_recv) = unbounded::<HostCommand>();
        host_receivers.insert(server.id, server_command_recv);
        graph.add_node(
            server.id,
            NetworkNode::Server {
                command_send: server_command_send,
            },
        );
    }

    (host_receivers, drone_receivers)
}

fn add_edges<T>(graph: &mut AdjacencyVecGraph<NodeId, T>, config: &Config) {
    for drone in config.drone.iter() {
        for neighbor_id in drone.connected_node_ids.iter() {
            graph.add_directed_edge(drone.id, *neighbor_id);
        }
    }

    for client in config.client.iter() {
        for neighbor_id in client.connected_drone_ids.iter() {
            graph.add_directed_edge(client.id, *neighbor_id);
        }
    }

    for server in config.server.iter() {
        for neighbor_id in server.connected_drone_ids.iter() {
            graph.add_directed_edge(server.id, *neighbor_id);
        }
    }
}

fn create_packet_channels<T>(
    graph: &AdjacencyVecGraph<NodeId, T>,
) -> (
    HashMap<NodeId, Sender<Packet>>,
    HashMap<NodeId, Receiver<Packet>>,
) {
    graph
        .keys()
        .map(|id| {
            let (snd, rcv) = unbounded::<Packet>();
            ((*id, snd), (*id, rcv))
        })
        .unzip()
}

fn create_runnables(
    graph: &NetworkGraph,
    packet_senders: &HashMap<NodeId, Sender<Packet>>,
    mut packet_receivers: HashMap<NodeId, Receiver<Packet>>,
    controller_channels: &mut ControllerChannels,
    drone_creator: &mut impl DroneCreator,
    client_creator: &mut impl ClientCreator,
    server_creator: &mut impl ServerCreator,
) -> Runnables {
    let mut runnables = HashMap::new();

    for (node_id, node_value) in graph.iter() {
        let packet_recv = packet_receivers.remove(node_id).unwrap();
        let packet_send = find_packet_send(graph.adjacents(node_id), packet_senders);

        let runnable = match node_value {
            NetworkNode::Drone { pdr, .. } => drone_creator.create_drone(
                *node_id,
                controller_channels
                    .drones_recv_command
                    .remove(node_id)
                    .unwrap(),
                packet_recv,
                packet_send,
                *pdr,
            ),
            NetworkNode::Client { .. } => client_creator.create_client(
                *node_id,
                controller_channels
                    .hosts_recv_command
                    .remove(node_id)
                    .unwrap(),
                packet_recv,
                packet_send,
            ),
            NetworkNode::Server { .. } => server_creator.create_server(
                *node_id,
                controller_channels
                    .hosts_recv_command
                    .remove(node_id)
                    .unwrap(),
                packet_recv,
                packet_send,
            ),
        };

        runnables.insert(*node_id, runnable);
    }

    runnables
}

pub fn find_packet_send<'a>(
    connected_node_ids: impl Iterator<Item = &'a NodeId>,
    packet_senders: &HashMap<NodeId, Sender<Packet>>,
) -> HashMap<NodeId, Sender<Packet>> {
    let mut packet_send = HashMap::with_capacity(5);
    for neighbor_id in connected_node_ids {
        if let Some(snd) = packet_senders.get(neighbor_id) {
            packet_send.insert(*neighbor_id, snd.clone());
        }
    }
    packet_send
}

pub fn spawn_threads(nodes: Runnables) -> HashMap<NodeId, JoinHandle<()>> {
    let mut handles = HashMap::new();
    for (id, mut node) in nodes {
        let spawn = thread::spawn(move || {
            node.run();
        });
        let handle = spawn;
        handles.insert(id, handle);
    }
    handles
}
