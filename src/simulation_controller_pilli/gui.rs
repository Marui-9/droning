use crate::application::simulation_controller_messages::{HostCommand, HostEvent};
use crate::initialization::network_initializer::{start_actual_simulation, NetworkNode};
use crate::initialization::node_creators::{
    ActualClientCreator, ActualDroneCreator, ActualServerCreator, ClientCreator, DroneCreator,
    ServerCreator,
};
use crate::simulation_controller_pilli::gui::PaneType::{ControlPane, MessagesPane, NetworkPane};
use crate::Topology;
use canvas::Program;
use crossbeam_channel::{unbounded, Receiver, Sender};
use graph::{AdjacencyVecGraph, ReferenceGraph};
use iced::advanced::image::{Handle, Image};
use iced::alignment::{Horizontal, Vertical};
use iced::event::Status;
use iced::mouse::Event as MouseEvent;
use iced::mouse::{Button, Cursor};
use iced::widget::canvas;
use iced::widget::canvas::stroke::Style;
use iced::widget::canvas::{Event, Frame, Geometry, Path, Stroke, Text};
use iced::widget::pane_grid::{Axis, State};
use iced::widget::{
    button, column, container, pane_grid, pick_list, row, scrollable, text, text_input,
};
use iced::{
    alignment, color, Color, Element, Length, Point, Rectangle, Renderer, Size, Task, Theme, Vector,
};
use rand::{random, thread_rng, Rng};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fmt::{Display, Formatter};
use std::time::{Duration, Instant};
use std::{env, thread};
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::network::NodeId;
use wg_2024::packet::{Packet, PacketType};

#[derive(Debug, Clone)]
pub struct DisplayableNode {
    position: Point,
    value: NetworkNode,
}

impl DisplayableNode {
    fn with_random_fields(value: NetworkNode) -> Self {
        let mut rng = thread_rng();
        Self::new(
            rng.gen_range(50.0..450.0),
            rng.gen_range(50.0..600.0),
            value,
        )
    }

    fn new(x: f32, y: f32, value: NetworkNode) -> Self {
        Self {
            position: Point::new(x, y),
            value,
        }
    }
}

#[derive(Default, Clone)]
pub struct Network {
    nodes: AdjacencyVecGraph<NodeId, DisplayableNode>,
    selected_node: Option<NodeId>,
    dragging_node: Option<NodeId>,
    packets: RefCell<Vec<(Instant, Packet)>>,
}

impl Program<Messages> for Network {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        event: Event,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> (Status, Option<Messages>) {
        match event {
            Event::Mouse(evt) => match evt {
                MouseEvent::CursorMoved { .. } => {
                    if self.dragging_node.is_some() {
                        if let Some(position) = cursor.position_in(bounds) {
                            (Status::Captured, Some(Messages::NodeMoved(position)))
                        } else {
                            (Status::Ignored, None)
                        }
                    } else {
                        (Status::Ignored, None)
                    }
                }
                MouseEvent::ButtonPressed(Button::Left) => {
                    if let Some(cursor_position) = cursor.position_in(bounds) {
                        let selected = self.nodes.iter().find_map(|(key, value)| {
                            if value.position.distance(cursor_position) < 25.0 {
                                Some(*key)
                            } else {
                                None
                            }
                        });
                        (Status::Captured, Some(Messages::NodeSelected(selected)))
                    } else {
                        (Status::Ignored, None)
                    }
                }
                MouseEvent::ButtonReleased(Button::Left) => {
                    (Status::Captured, Some(Messages::StopDragging))
                }
                _ => (Status::Ignored, None),
            },
            _ => (Status::Ignored, None),
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());

        frame.fill(
            &Path::rectangle(Point::default(), bounds.size()),
            Color::from_rgb8(155, 177, 191),
        );

        for (from, to) in self.nodes.edges() {
            let line = Path::line(
                self.nodes.get(from).unwrap().position,
                self.nodes.get(to).unwrap().position,
            );
            frame.stroke(
                &line,
                Stroke {
                    width: 2.0,
                    ..Stroke::default()
                },
            );
        }

        for (from, to) in self
            .packets
            .borrow()
            .iter()
            .map(|(_, pack)| {
                (
                    pack.routing_header.previous_hop().unwrap(),
                    pack.routing_header.current_hop().unwrap(),
                )
            })
            .filter(|(from, to)| self.nodes.contains_node(from) && self.nodes.contains_node(to))
        {
            let line = Path::line(
                self.nodes.get(&from).unwrap().position,
                self.nodes.get(&to).unwrap().position,
            );
            frame.stroke(
                &line,
                Stroke {
                    width: 2.0,
                    style: Style::Solid(iced::Color::from_rgba8(255, 255, 255, 0.1)),
                    ..Stroke::default()
                },
            );
        }

        for (id, node) in self.nodes.iter() {
            let path = match node.value {
                NetworkNode::Drone { .. } => "assets/pilli/Titti.png",
                NetworkNode::Client { .. } => "assets/pilli/Bugs Bunny.png",
                NetworkNode::Server { .. } => "assets/pilli/Lola Bunny.png",
            };
            const SIZE: f32 = 70.0;
            frame.draw_image(
                Rectangle::new(
                    node.position - Vector::new(SIZE / 2.0, SIZE / 2.0),
                    Size::new(SIZE, SIZE),
                ),
                Image::new(Handle::from_path(path)),
            );

            let text = Text {
                content: id.to_string(),
                position: node.position + Vector::new(0.0, 25.0),
                horizontal_alignment: Horizontal::Center,
                vertical_alignment: Vertical::Center,
                color: color!(0xb5040f),
                ..Text::default()
            };
            frame.fill_text(text);
        }

        vec![frame.into_geometry()]
    }
}

#[allow(clippy::enum_variant_names)]
enum PaneType {
    NetworkPane,
    ControlPane,
    MessagesPane,
}

#[derive(Debug, Clone)]

enum Messages {
    AddPressed,
    SelectedToAdd(NetworkNode),
    InputValue(String),
    PaneDragged(pane_grid::DragEvent),
    PaneResized(pane_grid::ResizeEvent),
    NodeSelected(Option<NodeId>),
    InputPDR(String),
    ChangePressed,
    NodeMoved(Point),
    StopDragging,
    DeleteNode,
    AddNeighbor(NodeId),
    RemoveNeighbor(NodeId),
    ConfirmAddNgh,
    ConfirmRemNgh,
    Tick,
}

pub struct Info {
    network: Network,
    to_add: Option<NetworkNode>,
    input_id: String,
    panes: State<PaneType>,
    input_pdr: String,
    to_add_ngh: Option<NodeId>,
    to_rem_ngh: Option<NodeId>,
    host_events: RefCell<VecDeque<HostEvent>>,
    drone_event_recv: Receiver<DroneEvent>,
    host_event_recv: Receiver<HostEvent>,
    packet_senders: HashMap<NodeId, Sender<Packet>>,
    drone_creator: ActualDroneCreator,
    client_creator: ActualClientCreator,
    server_creator: ActualServerCreator,
}

impl Display for HostEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            HostEvent::MessageReceived(message) => {
                write!(f, "{message}")
            }
            HostEvent::FloodInitiated(node_id, flood_id) => {
                write!(f, "{node_id} initiated flood {flood_id}")
            }
            HostEvent::MessageSent(message) => {
                write!(f, "{message}")
            }
        }
    }
}

impl Default for Info {
    fn default() -> Self {
        let args = env::args().collect::<Vec<_>>();
        let topology = args
                .get(2)
                .map(|arg| arg.parse().unwrap_or_else(|top| {
                    println!("Invalid topology, defaulting to DoubleChain");
                    println!("Available topologies: butterfly, double-chain, star-decagram, subnet-stars, subnet-triangles, tree");
                    top
                }))
                .unwrap_or(Topology::DoubleChain);
        let info = start_actual_simulation(topology.to_path());
        let (mut pane_state, pane) = State::new(NetworkPane);
        let (new_pane, _) = pane_state.split(Axis::Vertical, pane, ControlPane).unwrap();
        pane_state
            .split(Axis::Horizontal, new_pane, MessagesPane)
            .unwrap();

        Self {
            network: Network {
                nodes: info
                    .network_graph
                    .map_values(DisplayableNode::with_random_fields),
                ..Network::default()
            },
            panes: pane_state,
            host_event_recv: info.host_event_controller_recv,
            drone_event_recv: info.drone_event_controller_recv,
            host_events: Default::default(),
            to_add: Default::default(),
            to_rem_ngh: Default::default(),
            input_id: Default::default(),
            packet_senders: info.packet_senders,
            input_pdr: Default::default(),
            to_add_ngh: Default::default(),
            drone_creator: info.drone_creator,
            client_creator: info.client_creator,
            server_creator: info.server_creator,
        }
    }
}

impl Info {
    fn update(&mut self, messages: Messages) -> Task<Messages> {
        match messages {
            Messages::SelectedToAdd(selected) => self.to_add = Some(selected),
            Messages::InputValue(value) => {
                self.input_id = value;
            }
            Messages::AddPressed => {
                if let (Some(mut to_add), Ok(id)) = (self.to_add.clone(), self.input_id.parse()) {
                    if !self.network.nodes.contains_node(&id) {
                        match to_add {
                            NetworkNode::Drone {
                                pdr,
                                ref mut command_send,
                            } => {
                                let (cmd_send, cmd_recv) = unbounded();
                                let (pck_send, pck_recv) = unbounded();
                                *command_send = cmd_send;
                                self.packet_senders.insert(id, pck_send);
                                let mut runnable = self
                                    .drone_creator
                                    .create_disconnected_drone(id, cmd_recv, pck_recv, pdr);
                                thread::spawn(move || runnable.run());
                            }
                            NetworkNode::Client {
                                ref mut command_send,
                            } => {
                                let (cmd_send, cmd_recv) = unbounded();
                                let (pck_send, pck_recv) = unbounded();
                                *command_send = cmd_send;
                                self.packet_senders.insert(id, pck_send);
                                let mut runnable = self
                                    .client_creator
                                    .create_disconnected_client(id, cmd_recv, pck_recv);
                                thread::spawn(move || runnable.run());
                            }
                            NetworkNode::Server {
                                ref mut command_send,
                            } => {
                                let (cmd_send, cmd_recv) = unbounded();
                                let (pck_send, pck_recv) = unbounded();
                                *command_send = cmd_send;
                                self.packet_senders.insert(id, pck_send);
                                let mut runnable = self
                                    .server_creator
                                    .create_disconnected_server(id, cmd_recv, pck_recv);
                                thread::spawn(move || runnable.run());
                            }
                        }
                        self.network
                            .nodes
                            .add_node(id, DisplayableNode::with_random_fields(to_add));
                        self.input_id.clear();
                        self.to_add.take();
                    }
                }
            }
            Messages::PaneDragged(pane_grid::DragEvent::Dropped { pane, target }) => {
                self.panes.drop(pane, target)
            }
            Messages::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio)
            }
            Messages::PaneDragged(_) => {}
            Messages::NodeSelected(node) => {
                if node != self.network.selected_node {
                    self.to_add_ngh = None;
                    self.to_rem_ngh = None;
                }
                if node.is_some() {
                    self.network.dragging_node = node;
                }
                self.network.selected_node = node;
            }
            Messages::InputPDR(val) => self.input_pdr = val,
            Messages::ChangePressed => {
                if let Ok(new_pdr) = self.input_pdr.parse() {
                    if (0.0..=1.0).contains(&new_pdr) {
                        let node = self
                            .network
                            .nodes
                            .get_mut(&self.network.selected_node.unwrap())
                            .unwrap();

                        match node.value {
                            NetworkNode::Drone {
                                ref mut pdr,
                                ref command_send,
                            } => {
                                *pdr = new_pdr;
                                command_send
                                    .send(DroneCommand::SetPacketDropRate(new_pdr))
                                    .ok();
                            }
                            _ => unreachable!(),
                        }

                        self.input_pdr.clear();
                    }
                }
            }
            Messages::DeleteNode => {
                let to_delete = self.network.selected_node.unwrap();
                let mut new_graph = self.network.nodes.clone();
                new_graph.remove_node(&to_delete);

                if new_graph.is_connected_undirected()
                    && !self
                        .network
                        .nodes
                        .adjacents(&self.network.selected_node.unwrap())
                        .any(|node| {
                            if let NetworkNode::Server { .. } = self.network.nodes[node].value {
                                self.network.nodes.adjacents(node).count() <= 2
                            } else {
                                false
                            }
                        })
                {
                    match self.network.nodes.get(&to_delete).unwrap().value {
                        NetworkNode::Drone {
                            ref command_send, ..
                        } => {
                            command_send.send(DroneCommand::Crash).ok();
                        }
                        NetworkNode::Server { ref command_send }
                        | NetworkNode::Client { ref command_send } => {
                            command_send.send(HostCommand::Crash).ok();
                        }
                    }
                    for ngh in self.network.nodes.adjacents(&to_delete) {
                        match self.network.nodes[ngh].value {
                            NetworkNode::Drone {
                                ref command_send, ..
                            } => {
                                command_send
                                    .send(DroneCommand::RemoveSender(to_delete))
                                    .ok();
                            }
                            NetworkNode::Server { ref command_send }
                            | NetworkNode::Client { ref command_send } => {
                                command_send
                                    .send(HostCommand::RemoveConnectedDrone(to_delete))
                                    .ok();
                            }
                        }
                    }
                    self.network.nodes.remove_node(&to_delete);
                    self.network.selected_node = None;
                }
            }
            Messages::NodeMoved(position) => {
                if let Some(dragging) = self.network.dragging_node {
                    self.network.nodes.get_mut(&dragging).unwrap().position = position;
                }
            }
            Messages::StopDragging => {
                self.network.dragging_node = None;
            }
            Messages::AddNeighbor(value) => {
                self.to_add_ngh = Some(value);
            }
            Messages::RemoveNeighbor(value) => {
                self.to_rem_ngh = Some(value);
            }
            Messages::ConfirmAddNgh => {
                if let (Some(to_add_ngh), Some(selected)) =
                    (self.to_add_ngh.take(), self.network.selected_node)
                {
                    match self.network.nodes[&selected].value {
                        NetworkNode::Drone {
                            ref command_send, ..
                        } => {
                            command_send
                                .send(DroneCommand::AddSender(
                                    to_add_ngh,
                                    self.packet_senders[&to_add_ngh].clone(),
                                ))
                                .ok();
                        }
                        NetworkNode::Client { ref command_send }
                        | NetworkNode::Server { ref command_send } => {
                            command_send
                                .send(HostCommand::AddConnectedDrone(
                                    to_add_ngh,
                                    self.packet_senders[&to_add_ngh].clone(),
                                ))
                                .ok();
                        }
                    }

                    match self.network.nodes[&to_add_ngh].value {
                        NetworkNode::Drone {
                            ref command_send, ..
                        } => {
                            command_send
                                .send(DroneCommand::AddSender(
                                    selected,
                                    self.packet_senders[&selected].clone(),
                                ))
                                .ok();
                        }
                        NetworkNode::Client { ref command_send }
                        | NetworkNode::Server { ref command_send } => {
                            command_send
                                .send(HostCommand::AddConnectedDrone(
                                    selected,
                                    self.packet_senders[&selected].clone(),
                                ))
                                .ok();
                        }
                    }
                    self.network
                        .nodes
                        .add_undirected_edge(self.network.selected_node.unwrap(), to_add_ngh);
                }
            }
            Messages::ConfirmRemNgh => {
                if let (Some(to_rem_ngh), Some(selected)) =
                    (self.to_rem_ngh.take(), self.network.selected_node)
                {
                    match self.network.nodes[&selected].value {
                        NetworkNode::Drone {
                            ref command_send, ..
                        } => {
                            command_send
                                .send(DroneCommand::RemoveSender(to_rem_ngh))
                                .ok();
                        }
                        NetworkNode::Server { ref command_send }
                        | NetworkNode::Client { ref command_send } => {
                            command_send
                                .send(HostCommand::RemoveConnectedDrone(to_rem_ngh))
                                .ok();
                        }
                    }
                    match self.network.nodes[&to_rem_ngh].value {
                        NetworkNode::Drone {
                            ref command_send, ..
                        } => {
                            command_send.send(DroneCommand::RemoveSender(selected)).ok();
                        }
                        NetworkNode::Server { ref command_send }
                        | NetworkNode::Client { ref command_send } => {
                            command_send
                                .send(HostCommand::RemoveConnectedDrone(selected))
                                .ok();
                        }
                    }
                    self.network
                        .nodes
                        .remove_undirected_edge(&self.network.selected_node.unwrap(), &to_rem_ngh);
                }
            }
            Messages::Tick => {}
        }

        Task::none()
    }
    fn view(&self) -> Element<'_, Messages> {
        let mut network_packets = self.network.packets.borrow_mut();
        network_packets.retain(|(instant, _)| instant.elapsed().as_millis() < 500);
        let now = Instant::now();
        for event in self.drone_event_recv.try_iter() {
            match event {
                DroneEvent::PacketSent(packet) => {
                    if let PacketType::MsgFragment(_) = packet.pack_type {
                        network_packets.push((now, packet));
                    }
                }
                DroneEvent::ControllerShortcut(packet) => {
                    self.packet_senders[&packet.routing_header.destination().unwrap()]
                        .send(packet)
                        .ok();
                }
                _ => {}
            }
        }

        let mut host_events = self.host_events.borrow_mut();
        for event in self.host_event_recv.try_iter() {
            host_events.push_front(event);
        }

        drop(host_events);

        let all = [
            NetworkNode::Drone {
                pdr: random(),
                command_send: unbounded().0,
            },
            NetworkNode::Client {
                command_send: unbounded().0,
            },
            NetworkNode::Server {
                command_send: unbounded().0,
            },
        ];

        pane_grid(&self.panes, |_pane, state, _is_maximized| {
            pane_grid::Content::new(match state {
                PaneType::NetworkPane => container(column![
                    row![text("SELECT A NODE".to_string())
                        .size(25)
                        .color(color!(0x9c0b0b))
                        .align_x(alignment::Horizontal::Left)
                        .align_y(alignment::Vertical::Top)]
                    .padding(10),
                    canvas(&self.network)
                        .width(Length::Fill)
                        .height(Length::Fill),
                ]),
                PaneType::ControlPane => match self.network.selected_node {
                    Some(id) => container(
                        column![
                            text(format!("{} {id}", self.network.nodes[&id].value))
                                .size(25)
                                .color(color!(0x9c0b0b)),
                            match self.network.nodes.get(&id).unwrap().value {
                                NetworkNode::Drone { pdr: value, .. } => {
                                    container(scrollable(
                                        column![
                                            column![text("PDR (nr between 0 and 1)"), text(value),]
                                                .spacing(20),
                                            row![
                                                text_input("Insert new PDR:", &self.input_pdr)
                                                    .on_input(Messages::InputPDR),
                                                button("Change PDR")
                                                    .on_press(Messages::ChangePressed),
                                            ]
                                            .spacing(10),
                                            button("Crash Drone").on_press(Messages::DeleteNode),
                                            text("Add Neighbor"),
                                            row![
                                                pick_list(
                                                    self.network
                                                        .nodes
                                                        .keys()
                                                        .copied()
                                                        .filter(|id| {
                                                            self.network.selected_node != Some(*id)
                                                                && !self
                                                                    .network
                                                                    .nodes
                                                                    .is_adjacent_to(
                                                                        id,
                                                                        &self
                                                                            .network
                                                                            .selected_node
                                                                            .unwrap(),
                                                                    )
                                                        })
                                                        .filter(|id| {
                                                            if let NetworkNode::Client { .. } =
                                                                self.network.nodes[id].value
                                                            {
                                                                self.network
                                                                    .nodes
                                                                    .adjacents(id)
                                                                    .count()
                                                                    < 2
                                                            } else {
                                                                true
                                                            }
                                                        })
                                                        .collect::<Vec<_>>(),
                                                    self.to_add_ngh.as_ref(),
                                                    Messages::AddNeighbor
                                                )
                                                .placeholder("Select a Node"),
                                                button("Confirm").on_press(Messages::ConfirmAddNgh),
                                            ]
                                            .spacing(10),
                                            text("Remove Neighbor"),
                                            row![
                                                pick_list(
                                                    self.network
                                                        .nodes
                                                        .adjacents(
                                                            &self.network.selected_node.unwrap()
                                                        )
                                                        .copied()
                                                        .collect::<Vec<_>>(),
                                                    self.to_rem_ngh.as_ref(),
                                                    Messages::RemoveNeighbor
                                                )
                                                .placeholder("Select a Node"),
                                                button("Confirm").on_press(Messages::ConfirmRemNgh),
                                            ]
                                            .spacing(10),
                                        ]
                                        .spacing(20),
                                    ))
                                }
                                NetworkNode::Server { .. } => {
                                    let mut elements = Vec::new();
                                    elements.push(
                                        container(
                                            button("Delete Server").on_press(Messages::DeleteNode),
                                        )
                                        .into(),
                                    );
                                    elements.push(container(text("Add Neighbor")).into());
                                    elements.push(
                                        container(
                                            row![
                                                pick_list(
                                                    self.network
                                                        .nodes
                                                        .iter()
                                                        .filter_map(|(id, node)| {
                                                            if self.network.selected_node
                                                                != Some(*id)
                                                                && !self
                                                                    .network
                                                                    .nodes
                                                                    .is_adjacent_to(
                                                                        &self
                                                                            .network
                                                                            .selected_node
                                                                            .unwrap(),
                                                                        id,
                                                                    )
                                                                && matches!(
                                                                    node.value,
                                                                    NetworkNode::Drone { .. }
                                                                )
                                                            {
                                                                Some(id)
                                                            } else {
                                                                None
                                                            }
                                                        })
                                                        .copied()
                                                        .collect::<Vec<_>>(),
                                                    self.to_add_ngh.as_ref(),
                                                    Messages::AddNeighbor
                                                )
                                                .placeholder("Select a Node"),
                                                button("Confirm").on_press(Messages::ConfirmAddNgh),
                                            ]
                                            .spacing(10),
                                        )
                                        .into(),
                                    );

                                    if self
                                        .network
                                        .nodes
                                        .adjacents(&self.network.selected_node.unwrap())
                                        .count()
                                        > 2
                                    {
                                        elements.push(container(text("Remove Neighbor")).into());
                                        elements.push(
                                            container(
                                                row![
                                                    pick_list(
                                                        self.network
                                                            .nodes
                                                            .adjacents(
                                                                &self
                                                                    .network
                                                                    .selected_node
                                                                    .unwrap()
                                                            )
                                                            .copied()
                                                            .collect::<Vec<_>>(),
                                                        self.to_rem_ngh.as_ref(),
                                                        Messages::RemoveNeighbor
                                                    )
                                                    .placeholder("Select a Node"),
                                                    button("Confirm")
                                                        .on_press(Messages::ConfirmRemNgh),
                                                ]
                                                .spacing(10),
                                            )
                                            .into(),
                                        );
                                    }

                                    container(scrollable(column(elements).spacing(15)))
                                }
                                NetworkNode::Client { .. } => {
                                    let mut elements = Vec::new();
                                    elements.push(
                                        container(
                                            button("Delete Client").on_press(Messages::DeleteNode),
                                        )
                                        .into(),
                                    );
                                    if self
                                        .network
                                        .nodes
                                        .adjacents(&self.network.selected_node.unwrap())
                                        .count()
                                        < 2
                                        && self
                                            .network
                                            .nodes
                                            .adjacents(&self.network.selected_node.unwrap())
                                            .count()
                                            > 0
                                    {
                                        elements.push(container(text("Add Neighbor")).into());
                                        elements.push(
                                            container(
                                                row![
                                                    pick_list(
                                                        self.network
                                                            .nodes
                                                            .iter()
                                                            .filter_map(|(id, node)| {
                                                                if self.network.selected_node
                                                                    != Some(*id)
                                                                    && !self
                                                                        .network
                                                                        .nodes
                                                                        .is_adjacent_to(
                                                                            &self
                                                                                .network
                                                                                .selected_node
                                                                                .unwrap(),
                                                                            id,
                                                                        )
                                                                    && matches!(
                                                                        node.value,
                                                                        NetworkNode::Drone { .. }
                                                                    )
                                                                {
                                                                    Some(id)
                                                                } else {
                                                                    None
                                                                }
                                                            })
                                                            .copied()
                                                            .collect::<Vec<_>>(),
                                                        self.to_add_ngh.as_ref(),
                                                        Messages::AddNeighbor
                                                    )
                                                    .placeholder("Select a Node"),
                                                    button("Confirm")
                                                        .on_press(Messages::ConfirmAddNgh),
                                                ]
                                                .spacing(10),
                                            )
                                            .into(),
                                        );
                                    }
                                    if self
                                        .network
                                        .nodes
                                        .adjacents(&self.network.selected_node.unwrap())
                                        .count()
                                        > 1
                                    {
                                        elements.push(container(text("Remove Neighbor")).into());

                                        elements.push(
                                            container(
                                                row![
                                                    pick_list(
                                                        self.network
                                                            .nodes
                                                            .adjacents(
                                                                &self
                                                                    .network
                                                                    .selected_node
                                                                    .unwrap()
                                                            )
                                                            .copied()
                                                            .collect::<Vec<_>>(),
                                                        self.to_rem_ngh.as_ref(),
                                                        Messages::RemoveNeighbor
                                                    )
                                                    .placeholder("Select a Node"),
                                                    button("Confirm")
                                                        .on_press(Messages::ConfirmRemNgh),
                                                ]
                                                .spacing(10),
                                            )
                                            .into(),
                                        );
                                    }
                                    container(scrollable(column(elements).spacing(15))).padding(10)
                                }
                            }
                        ]
                        .spacing(10),
                    )
                    .padding(10),
                    None => container(
                        column![
                            container(text("ADD NODE").size(20).color(color!(0x9c0b0b))),
                            container(scrollable(
                                column![
                                    pick_list(
                                        all.clone(),
                                        self.to_add.as_ref(),
                                        Messages::SelectedToAdd
                                    )
                                    .placeholder("What to add"),
                                    text_input("Insert ID: ", &self.input_id)
                                        .on_input(Messages::InputValue),
                                    button("ADD").on_press(Messages::AddPressed),
                                ]
                                .spacing(10)
                            ))
                        ]
                        .spacing(10),
                    )
                    .padding(10),
                },
                MessagesPane => container(column![
                    container(text("MESSAGES").size(25).color(color!(0x9c0b0b))),
                    container(
                        scrollable(
                            column(
                                self.host_events
                                    .borrow()
                                    .iter()
                                    .map(|element| format!("{element}"))
                                    .map(text)
                                    .map(Into::into)
                            )
                            .spacing(25)
                        )
                        .width(Length::Fill)
                    ),
                ])
                .padding(10),
            })
        })
        .on_drag(Messages::PaneDragged)
        .on_resize(10, Messages::PaneResized)
        .into()
    }
}

pub fn main() -> iced::Result {
    iced::application("Bagel Bomber", Info::update, Info::view)
        .subscription(|_state| {
            iced::time::every(Duration::from_millis(100)).map(|_| Messages::Tick)
        })
        .run()
}
