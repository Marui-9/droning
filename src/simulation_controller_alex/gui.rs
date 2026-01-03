use crate::application::simulation_controller_messages::{HostCommand, HostEvent};
use crate::initialization::network_initializer::{start_actual_simulation, NetworkNode};
use crate::initialization::node_creators::{
    ActualClientCreator, ActualDroneCreator, ActualServerCreator, ClientCreator, DroneCreator,
    ServerCreator,
};
use crate::simulation_controller_alex::gui::DroneCommandsMessage::{
    AddSenderPressed, CrashPressed, RmvSenderPressed,
};
use crate::simulation_controller_alex::gui::NodesPaneMessage::{ButtonPressed, TypeSelected};
use crate::Topology as TopologyType;
use crossbeam_channel::{unbounded, Receiver, Sender};
use graph::{AdjacencyVecGraph, ReferenceGraph};
use iced::alignment::{Horizontal, Vertical};
use iced::font::Weight;
use iced::mouse::Cursor;
use iced::widget::canvas::{Frame, Geometry, Path, Program, Stroke, Text};
use iced::widget::pane_grid::{Axis, Content, Direction, Pane, ResizeEvent, State, TitleBar};
use iced::widget::{
    button, canvas, column, container, pane_grid, pick_list, row, scrollable, slider, text,
    text_input,
};
use iced::{Color, Font, Pixels};
use iced::{Element, Fill, Point, Rectangle, Renderer, Theme};
use rand::{random, Rng};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use std::{env, thread};
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;

#[derive(Debug, Clone)]
struct Topology {
    graph: AdjacencyVecGraph<NodeId, (NetworkNode, Point)>,
    selected_node: Option<NodeId>,
}

impl Topology {
    fn new(graph: AdjacencyVecGraph<NodeId, (NetworkNode, Point)>) -> Self {
        Self {
            graph,
            selected_node: None,
        }
    }
}

type TopologyRef = Rc<RefCell<Topology>>;

#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
enum Message {
    NodesMessage(NodesPaneMessage),
    DroneCommandsMessage(DroneCommandsMessage),
    TopologyMessage(TopologyMessage),
    DroneEventsMessage(DroneEventsMessage),
    Clicked(Pane),
    Resized(ResizeEvent),
    Tick,
}

#[allow(clippy::enum_variant_names)]
enum PaneType {
    NodesPane(NodesPane),
    DroneCommandsPane(DroneCommandsPane),
    TopologyPane(TopologyPane),
    DroneEventsPane(DroneEventsPane),
}

struct Application {
    pane_state: State<PaneType>,
    active_pane: Option<Pane>,
}

impl Default for Application {
    fn default() -> Self {
        let args = env::args().collect::<Vec<String>>();
        let topology = args
                .get(2)
                .map(|arg| arg.parse().unwrap_or_else(|top| {
                    println!("Invalid topology, defaulting to DoubleChain");
                    println!("Available topologies: butterfly, double-chain, star-decagram, subnet-stars, subnet-triangles, tree");
                    top
                }))
                .unwrap_or(TopologyType::DoubleChain);
        let controller_info = start_actual_simulation(topology.to_path());

        let graph = controller_info
            .network_graph
            .map_values(|node| (node, random_point()));

        let topology = Rc::new(RefCell::new(Topology::new(graph)));

        let packet_senders = Rc::new(RefCell::new(controller_info.packet_senders));
        let drone_event_rcv = RefCell::new(controller_info.drone_event_controller_recv);
        let host_event_rcv = RefCell::new(controller_info.host_event_controller_recv);

        let (mut pane_state, pane) = State::new(PaneType::NodesPane(NodesPane::new(
            topology.clone(),
            controller_info.drone_creator,
            controller_info.client_creator,
            controller_info.server_creator,
            packet_senders.clone(),
        )));
        pane_state.split(
            Axis::Vertical,
            pane,
            PaneType::TopologyPane(TopologyPane::new(topology.clone())),
        );
        pane_state.split(
            Axis::Vertical,
            pane,
            PaneType::DroneCommandsPane(DroneCommandsPane::new(
                topology.clone(),
                packet_senders.clone(),
            )),
        );
        pane_state.split(
            Axis::Horizontal,
            pane_state.adjacent(pane, Direction::Right).unwrap(),
            PaneType::DroneEventsPane(DroneEventsPane::new(
                drone_event_rcv.clone(),
                host_event_rcv.clone(),
                packet_senders.clone(),
            )),
        );

        Self {
            pane_state,
            active_pane: None,
        }
    }
}

impl Application {
    fn update(&mut self, message: Message) {
        match message {
            Message::Clicked(pane) => {
                self.active_pane = Some(pane);
            }
            Message::Resized(ResizeEvent { split, ratio }) => {
                self.pane_state.resize(split, ratio);
            }
            Message::NodesMessage(nodes_message) => {
                if let Some(PaneType::NodesPane(nodes_pane)) = self
                    .active_pane
                    .and_then(|pane| self.pane_state.get_mut(pane))
                {
                    nodes_pane.update(nodes_message);
                }
            }
            Message::DroneCommandsMessage(drone_commands_message) => {
                if let Some(PaneType::DroneCommandsPane(drone_commands_pane)) = self
                    .active_pane
                    .and_then(|pane| self.pane_state.get_mut(pane))
                {
                    drone_commands_pane.update(drone_commands_message);
                }
            }
            _ => {}
        }
    }

    fn view(&self) -> Element<Message> {
        let grid = pane_grid(
            &self.pane_state,
            |_pane, state, _is_maximized| match state {
                PaneType::NodesPane(nodes_pane) => {
                    let title_bar = TitleBar::new(text("Nodes")).padding(5);
                    Content::new(nodes_pane.view().map(Message::NodesMessage)).title_bar(title_bar)
                }
                PaneType::DroneCommandsPane(drone_commands_pane) => {
                    let title_bar = TitleBar::new(text("Drone Commands")).padding(5);
                    Content::new(
                        drone_commands_pane
                            .view()
                            .map(Message::DroneCommandsMessage),
                    )
                    .title_bar(title_bar)
                }
                PaneType::TopologyPane(topology_pane) => {
                    let title_bar = TitleBar::new(text("Network topology")).padding(5);
                    Content::new(topology_pane.view().map(Message::TopologyMessage))
                        .title_bar(title_bar)
                }
                PaneType::DroneEventsPane(drone_events_pane) => {
                    let title_bar = TitleBar::new(text("Node Events")).padding(5);
                    Content::new(drone_events_pane.view().map(Message::DroneEventsMessage))
                        .title_bar(title_bar)
                }
            },
        )
        .width(Fill)
        .height(Fill)
        .spacing(10)
        .on_click(Message::Clicked)
        .on_resize(10, Message::Resized);

        container(grid).width(Fill).height(Fill).padding(10).into()
    }
}

struct NodesPane {
    topology: TopologyRef,
    input_value: String,
    selected_type: Option<NetworkNode>,
    drone_creator: ActualDroneCreator,
    client_creator: ActualClientCreator,
    server_creator: ActualServerCreator,
    packet_senders: Rc<RefCell<HashMap<NodeId, Sender<Packet>>>>,
}

impl NodesPane {
    fn new(
        topology: TopologyRef,
        drone_creator: ActualDroneCreator,
        client_creator: ActualClientCreator,
        server_creator: ActualServerCreator,
        packet_senders: Rc<RefCell<HashMap<NodeId, Sender<Packet>>>>,
    ) -> Self {
        Self {
            topology,
            input_value: "".to_string(),
            selected_type: None,
            drone_creator,
            client_creator,
            server_creator,
            packet_senders,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum NodesPaneMessage {
    ButtonPressed,
    InputChanged(String),
    TypeSelected(NetworkNode),
    NodeSelected(NodeId),
}

impl NodesPane {
    fn update(&mut self, message: NodesPaneMessage) {
        match message {
            ButtonPressed => {
                let node_id = self.input_value.parse::<u8>();
                if let Ok(id) = node_id {
                    if let Some(mut node_type) = self.selected_type.take() {
                        if !self.topology.borrow().graph.contains_node(&id) {
                            match &mut node_type {
                                NetworkNode::Drone { pdr, command_send } => {
                                    *pdr = random();
                                    let (cmd_send, cmd_receive) = unbounded();
                                    *command_send = cmd_send;
                                    let (pckt_send, pckt_receive) = unbounded();
                                    self.packet_senders.borrow_mut().insert(id, pckt_send);
                                    let mut runnable =
                                        self.drone_creator.create_disconnected_drone(
                                            id,
                                            cmd_receive,
                                            pckt_receive,
                                            *pdr,
                                        );
                                    thread::spawn(move || runnable.run());
                                }
                                NetworkNode::Client { command_send } => {
                                    let (cmd_send, cmd_receive) = unbounded();
                                    *command_send = cmd_send;
                                    let (pckt_send, pckt_receive) = unbounded();
                                    self.packet_senders.borrow_mut().insert(id, pckt_send);
                                    let mut runnable = self
                                        .client_creator
                                        .create_disconnected_client(id, cmd_receive, pckt_receive);
                                    thread::spawn(move || runnable.run());
                                }
                                NetworkNode::Server { command_send } => {
                                    let (cmd_send, cmd_receive) = unbounded();
                                    *command_send = cmd_send;
                                    let (pckt_send, pckt_receive) = unbounded();
                                    self.packet_senders.borrow_mut().insert(id, pckt_send);
                                    let mut runnable = self
                                        .server_creator
                                        .create_disconnected_server(id, cmd_receive, pckt_receive);
                                    thread::spawn(move || runnable.run());
                                }
                            }
                            self.topology
                                .borrow_mut()
                                .graph
                                .add_node(id, (node_type, random_point()));
                        }
                    }
                }
                self.input_value = "".to_string();
            }
            NodesPaneMessage::InputChanged(input) => {
                self.input_value = input.clone();
            }
            TypeSelected(node_type) => {
                self.selected_type = Some(node_type);
            }
            NodesPaneMessage::NodeSelected(id) => {
                self.topology.borrow_mut().selected_node = Some(id);
            }
        }
    }

    fn view(&self) -> Element<NodesPaneMessage> {
        let input = self.input_value.clone();

        let button = button("+").on_press(ButtonPressed);

        let text_input = text_input("NodeId", &input)
            .on_input(NodesPaneMessage::InputChanged)
            .width(100);

        let node_types: [NetworkNode; 3] = [
            NetworkNode::Drone {
                pdr: 0f32,
                command_send: unbounded().0,
            },
            NetworkNode::Client {
                command_send: unbounded().0,
            },
            NetworkNode::Server {
                command_send: unbounded().0,
            },
        ];

        let spawn = container(
            column![
                pick_list(node_types, self.selected_type.clone(), TypeSelected)
                    .placeholder("NodeType"),
                row![text_input, button,].spacing(10)
            ]
            .spacing(10),
        )
        .height(80);

        container(
            column![spawn, scrollable(container(self.view_nodes())).height(Fill),].spacing(20),
        )
        .height(Fill)
        .width(Fill)
        .padding(20)
        .into()
    }

    fn view_nodes(&self) -> Element<NodesPaneMessage> {
        column(
            self.topology
                .borrow()
                .graph
                .iter()
                .map(|(id, (node_type, _position))| {
                    button(text(format!(
                        "{}, ID: {}",
                        match node_type {
                            NetworkNode::Drone { .. } => "Drone",
                            NetworkNode::Server { .. } => "Server",
                            NetworkNode::Client { .. } => "Client",
                        },
                        id
                    )))
                    .on_press(NodesPaneMessage::NodeSelected(*id))
                    .into()
                }),
        )
        .spacing(10)
        .width(Fill)
        .into()
    }
}

fn random_point() -> Point {
    let mut rng = rand::thread_rng();
    let x: f32 = rng.gen_range(30.0..630f32);
    let y: f32 = rng.gen_range(30.0..480f32);
    Point::new(x, y)
}

struct DroneCommandsPane {
    topology: TopologyRef,
    slider_value: f32,
    slider_input_content: String,
    pick_list_add_selected: Option<NodeId>,
    pick_list_rmv_selected: Option<NodeId>,
    packet_senders: Rc<RefCell<HashMap<NodeId, Sender<Packet>>>>,
}

#[derive(Debug, Clone)]
enum DroneCommandsMessage {
    SliderChanged(f32),
    SliderInputChanged(String),
    SliderInputSubmitted(String),
    CrashPressed(Option<NodeId>),
    AddNodeSelected(NodeId),
    RmvNodeSelected(NodeId),
    AddSenderPressed(Option<NodeId>),
    RmvSenderPressed(Option<NodeId>),
}

impl DroneCommandsPane {
    fn new(
        topology: TopologyRef,
        packet_senders: Rc<RefCell<HashMap<NodeId, Sender<Packet>>>>,
    ) -> Self {
        Self {
            topology,
            slider_value: 0.0,
            slider_input_content: "".to_string(),
            pick_list_add_selected: None,
            pick_list_rmv_selected: None,
            packet_senders,
        }
    }
    fn update(&mut self, message: DroneCommandsMessage) {
        match message {
            DroneCommandsMessage::SliderChanged(slider_value) => {
                let mut topology = self.topology.borrow_mut();
                if let Some(id) = topology.selected_node {
                    if let NetworkNode::Drone { pdr, command_send } = &mut topology.graph[&id].0 {
                        self.slider_value = change_pdr(slider_value);
                        *pdr = self.slider_value;
                        command_send
                            .send(DroneCommand::SetPacketDropRate(*pdr))
                            .expect("error sending SetPacketDropRate");
                    }
                }
            }
            DroneCommandsMessage::SliderInputChanged(input_value) => {
                self.slider_input_content = input_value.deref().to_owned();
            }
            DroneCommandsMessage::SliderInputSubmitted(input_value) => {
                let mut topology = self.topology.borrow_mut();
                if let Some(id) = topology.selected_node {
                    if let NetworkNode::Drone { pdr, command_send } = &mut topology.graph[&id].0 {
                        let new_value = input_value.parse::<f32>();
                        if let Ok(value) = new_value {
                            if value >= 1f32 {
                                self.slider_value = 1f32;
                            } else if value <= 0f32 {
                                self.slider_value = 0f32;
                            } else {
                                self.slider_value = change_pdr(value);
                            }
                            *pdr = self.slider_value;
                            command_send
                                .send(DroneCommand::SetPacketDropRate(*pdr))
                                .expect("error sending SetPacketDropRate");
                            self.slider_input_content = "".to_string();
                        }
                    }
                }
            }
            CrashPressed(option_id) => {
                if let Some(id) = option_id {
                    let mut topology = self.topology.borrow_mut();
                    let mut new_graph = topology.graph.clone();
                    new_graph.remove_node(&id);
                    if new_graph.is_connected_undirected() && is_topology_valid(&new_graph) {
                        if let NetworkNode::Drone { command_send, .. } = &topology.graph[&id].0 {
                            for node in topology
                                .graph
                                .adjacents(&id)
                                .map(|adj| &topology.graph[adj].0)
                            {
                                match node {
                                    NetworkNode::Drone { command_send, .. } => {
                                        command_send
                                            .send(DroneCommand::RemoveSender(id))
                                            .expect("error sending RemoveSender");
                                    }
                                    NetworkNode::Client { command_send }
                                    | NetworkNode::Server { command_send } => {
                                        command_send
                                            .send(HostCommand::RemoveConnectedDrone(id))
                                            .expect("error sending RemoveConnectedDrone");
                                    }
                                }
                            }
                            command_send
                                .send(DroneCommand::Crash)
                                .expect("error sending Crash");
                            topology.graph.remove_node(&id);
                        }
                        topology.selected_node = None;
                    }
                }
            }
            DroneCommandsMessage::AddNodeSelected(node) => self.pick_list_add_selected = Some(node),
            AddSenderPressed(node_option) => {
                let mut topology = self.topology.borrow_mut();
                if let Some(selected_node) = topology.selected_node {
                    if let Some(node) = node_option {
                        let node_type = &topology.graph[&node].0;
                        let selected_node_type = &topology.graph[&selected_node].0;
                        let can_connect = !matches!(
                            (node_type, selected_node_type),
                            (NetworkNode::Client { .. }, NetworkNode::Client { .. })
                                | (NetworkNode::Server { .. }, NetworkNode::Server { .. })
                                | (NetworkNode::Server { .. }, NetworkNode::Client { .. })
                                | (NetworkNode::Client { .. }, NetworkNode::Server { .. })
                        );
                        let mut new_graph = topology.graph.clone();
                        new_graph.add_undirected_edge(selected_node, node);
                        if can_connect && is_topology_valid(&new_graph) {
                            match node_type {
                                NetworkNode::Drone { command_send, .. } => {
                                    command_send
                                        .send(DroneCommand::AddSender(
                                            selected_node,
                                            self.packet_senders.borrow()[&selected_node].clone(),
                                        ))
                                        .expect("error sending AddSender");
                                }
                                NetworkNode::Client { command_send }
                                | NetworkNode::Server { command_send } => {
                                    command_send
                                        .send(HostCommand::AddConnectedDrone(
                                            selected_node,
                                            self.packet_senders.borrow()[&selected_node].clone(),
                                        ))
                                        .expect("error sending AddConnectedDrone");
                                }
                            }
                            match selected_node_type {
                                NetworkNode::Drone { command_send, .. } => {
                                    command_send
                                        .send(DroneCommand::AddSender(
                                            node,
                                            self.packet_senders.borrow()[&node].clone(),
                                        ))
                                        .expect("error sending AddSender");
                                }
                                NetworkNode::Client { command_send }
                                | NetworkNode::Server { command_send } => {
                                    command_send
                                        .send(HostCommand::AddConnectedDrone(
                                            node,
                                            self.packet_senders.borrow()[&node].clone(),
                                        ))
                                        .expect("error sending AddConnectedDrone");
                                }
                            }
                            topology.graph.add_undirected_edge(selected_node, node);
                        }
                        self.pick_list_add_selected = None;
                    }
                }
            }
            DroneCommandsMessage::RmvNodeSelected(node) => self.pick_list_rmv_selected = Some(node),
            RmvSenderPressed(node_option) => {
                let mut topology = self.topology.borrow_mut();
                if let Some(id) = topology.selected_node {
                    if let Some(node) = node_option {
                        let mut new_graph = topology.graph.clone();
                        new_graph.remove_undirected_edge(&id, &node);
                        if new_graph.is_connected_undirected() && is_topology_valid(&new_graph) {
                            match &topology.graph[&node].0 {
                                NetworkNode::Drone { command_send, .. } => {
                                    command_send
                                        .send(DroneCommand::RemoveSender(id))
                                        .expect("error sending RemoveSender");
                                }
                                NetworkNode::Client { command_send }
                                | NetworkNode::Server { command_send } => {
                                    command_send
                                        .send(HostCommand::RemoveConnectedDrone(id))
                                        .expect("error sending RemoveConnectedDrone");
                                }
                            }
                            topology.graph.remove_undirected_edge(&id, &node);
                        }
                        self.pick_list_rmv_selected = None;
                    }
                }
            }
        }
    }

    fn view(&self) -> Element<DroneCommandsMessage> {
        let topology = self.topology.borrow();

        let add_sender =
            button("Add sender").on_press(AddSenderPressed(self.pick_list_add_selected));

        let pick_list_add = pick_list(
            topology
                .graph
                .keys()
                .copied()
                .filter(|id| {
                    if topology.selected_node.is_some() {
                        topology.selected_node != Some(*id)
                            && !topology
                                .graph
                                .is_adjacent_to(&topology.selected_node.unwrap(), id)
                    } else {
                        false
                    }
                })
                .collect::<Vec<_>>(),
            self.pick_list_add_selected.as_ref(),
            DroneCommandsMessage::AddNodeSelected,
        )
        .placeholder("Select node");

        let rmv_sender =
            button("Remove sender").on_press(RmvSenderPressed(self.pick_list_rmv_selected));

        let pick_list_rmv = pick_list(
            topology
                .graph
                .keys()
                .copied()
                .filter(|id| {
                    if topology.selected_node.is_some() {
                        topology.selected_node != Some(*id)
                            && topology
                                .graph
                                .is_adjacent_to(&topology.selected_node.unwrap(), id)
                    } else {
                        false
                    }
                })
                .collect::<Vec<_>>(),
            self.pick_list_rmv_selected.as_ref(),
            DroneCommandsMessage::RmvNodeSelected,
        )
        .placeholder("Select node");

        if let Some(id) = topology.selected_node {
            match topology.graph[&id].0 {
                NetworkNode::Drone { .. } => {
                    let slider_value = match topology.selected_node {
                        Some(id) => match topology.graph[&id].0 {
                            NetworkNode::Drone { pdr, .. } => (pdr * 100f32).round() / 100f32,
                            _ => 0f32,
                        },
                        _ => 0.0,
                    };

                    let slider = container(
                        slider(
                            0f32..=1f32,
                            slider_value,
                            DroneCommandsMessage::SliderChanged,
                        )
                        .default(0f32)
                        .step(0.01),
                    )
                    .width(150);

                    let text_input = text_input("...", &self.slider_input_content)
                        .on_input(DroneCommandsMessage::SliderInputChanged)
                        .on_submit(DroneCommandsMessage::SliderInputSubmitted(
                            self.slider_input_content.clone(),
                        )); // ugly clones

                    let slider = container(
                        row![
                            "PDR:",
                            column![slider, text(slider_value).center(),],
                            text_input.width(50),
                        ]
                        .spacing(15),
                    )
                    .width(Fill)
                    .padding(10);

                    let crash = button("Crash").on_press(CrashPressed(topology.selected_node));

                    container(
                        column![
                            crash,
                            slider,
                            container(row![add_sender, pick_list_add].spacing(20)),
                            container(row![rmv_sender, pick_list_rmv].spacing(20))
                        ]
                        .spacing(20),
                    )
                    .width(Fill)
                    .padding(20)
                    .into()
                }
                _ => container(
                    column![
                        container(row![add_sender, pick_list_add].spacing(20)),
                        container(row![rmv_sender, pick_list_rmv].spacing(20))
                    ]
                    .spacing(20),
                )
                .width(Fill)
                .padding(20)
                .into(),
            }
        } else {
            container(text("Please select a node")).padding(20).into()
        }
    }
}

fn is_topology_valid(topology: &AdjacencyVecGraph<NodeId, (NetworkNode, Point)>) -> bool {
    let client_drones = topology
        .iter()
        .filter(|(_id, (node_type, _position))| matches!(node_type, NetworkNode::Client { .. }))
        .all(|(id, (_node_type, _position))| {
            topology.adjacents(id).count() == 1 || topology.adjacents(id).count() == 2
        });

    let server_drones = topology
        .iter()
        .filter(|(_id, (node_type, _position))| matches!(node_type, NetworkNode::Server { .. }))
        .all(|(id, (_node_type, _position))| topology.adjacents(id).count() >= 2);

    client_drones && server_drones
}

fn change_pdr(val: f32) -> f32 {
    (val * 100f32).round() / 100f32
}

struct TopologyPane {
    topology: TopologyRef,
}

#[derive(Debug)]
enum TopologyMessage {}

impl TopologyPane {
    fn new(topology: TopologyRef) -> Self {
        Self { topology }
    }

    fn view(&self) -> Element<TopologyMessage> {
        let canvas = canvas(self).width(Fill).height(Fill);
        container(canvas).padding(20).into()
    }
}

impl Program<TopologyMessage> for TopologyPane {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());
        let topology = self.topology.borrow();
        for (from, to) in topology
            .graph
            .edges()
            .map(|(from, to)| (topology.graph[from].1, topology.graph[to].1))
        {
            frame.stroke(
                &Path::line(from, to),
                Stroke {
                    width: 5.0,
                    ..Default::default()
                },
            );
        }
        for (id, (node_type, position)) in topology.graph.iter() {
            frame.fill(
                &Path::circle(*position, 20f32),
                if topology.selected_node == Some(*id) {
                    Color::from_rgb8(255, 0, 0)
                } else {
                    match node_type {
                        NetworkNode::Drone { .. } => Color::WHITE,
                        NetworkNode::Server { .. } => Color::from_rgb8(128, 0, 128),
                        NetworkNode::Client { .. } => Color::from_rgb8(0, 255, 0),
                    }
                },
            );
            frame.fill_text(Text {
                content: format!("{}", *id),
                position: *position,
                size: Pixels(18f32),
                font: Font {
                    weight: Weight::Bold,
                    ..Default::default()
                },
                horizontal_alignment: Horizontal::Center,
                vertical_alignment: Vertical::Center,
                ..Default::default()
            });
        }

        vec![frame.into_geometry()]
    }
}

struct DroneEventsPane {
    drone_event_rcv: RefCell<Receiver<DroneEvent>>,
    host_event_rcv: RefCell<Receiver<HostEvent>>,
    packet_senders: Rc<RefCell<HashMap<NodeId, Sender<Packet>>>>,
}

#[derive(Debug)]
enum DroneEventsMessage {}

impl DroneEventsPane {
    fn new(
        drone_event_rcv: RefCell<Receiver<DroneEvent>>,
        host_event_rcv: RefCell<Receiver<HostEvent>>,
        packet_senders: Rc<RefCell<HashMap<NodeId, Sender<Packet>>>>,
    ) -> Self {
        Self {
            drone_event_rcv,
            host_event_rcv,
            packet_senders,
        }
    }

    fn view(&self) -> Element<DroneEventsMessage> {
        container(
            column![
                container(
                    column!(text("Drone Events:"), scrollable(self.drone_listener()),).spacing(15)
                ),
                container(
                    column!(text("Host Events:"), scrollable(self.host_listener()),).spacing(15)
                ),
            ]
            .spacing(40),
        )
        .padding(20)
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn drone_listener(&self) -> Element<DroneEventsMessage> {
        column(
            self.drone_event_rcv
                .borrow()
                .try_iter()
                .filter_map(|event| {
                    if let DroneEvent::ControllerShortcut(packet) = event {
                        let destination_sender = &self.packet_senders.borrow()
                            [&packet.routing_header.destination().unwrap()];
                        destination_sender
                            .send(packet)
                            .expect("error sending packet");
                        None
                    } else {
                        Some(text(format!("{:?}", event)).into())
                    }
                }),
        )
        .spacing(30)
        .height(100)
        .width(Fill)
        .into()
    }
    fn host_listener(&self) -> Element<DroneEventsMessage> {
        column(
            self.host_event_rcv
                .borrow()
                .try_iter()
                .map(|event| text(format!("{:?}", event)).into()),
        )
        .spacing(30)
        .height(100)
        .width(Fill)
        .into()
    }
}

pub fn main() -> iced::Result {
    iced::application(
        "Simulation Controller",
        Application::update,
        Application::view,
    )
    .centered()
    .run()
}
