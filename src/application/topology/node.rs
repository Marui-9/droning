use std::collections::VecDeque;
use std::fmt::{Debug, Display, Formatter, Result};
use wg_2024::network::NodeId;
use wg_2024::packet::NodeType as SimpleNodeType;

const MEMORY_SIZE: usize = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationType {
    Chat,
    Content,
    Unknown,
    Unwanted,
}

impl ApplicationType {
    fn compatible(&self, other: &Self) -> bool {
        match (self, other) {
            (ApplicationType::Unknown, _) => true,
            (_, ApplicationType::Unknown) => true,
            (ApplicationType::Unwanted, _) => false,
            (_, ApplicationType::Unwanted) => false,
            _ => self == other,
        }
    }
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum FragmentDelivery {
    Forwarded,
    Dropped,
}

#[derive(Clone)]
pub struct Drone {
    latest_deliveries: VecDeque<FragmentDelivery>,
}

impl Debug for Drone {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "Drone: PDR = {:.2}%", self.calculate_pdr() * 100.0)
    }
}

impl Default for Drone {
    fn default() -> Self {
        Self::new()
    }
}

impl Drone {
    pub fn new() -> Self {
        Self {
            latest_deliveries: VecDeque::with_capacity(MEMORY_SIZE),
        }
    }

    pub fn with_delivery(deliveries: Vec<FragmentDelivery>) -> Self {
        Self {
            latest_deliveries: deliveries.into(),
        }
    }

    pub fn merge_drone(&mut self, other: Drone) {
        for delivery in other.latest_deliveries.into_iter() {
            self.record_delivery(delivery);
        }
    }

    pub fn record_delivery(&mut self, delivery: FragmentDelivery) {
        self.latest_deliveries.push_back(delivery);
        if self.latest_deliveries.len() > MEMORY_SIZE {
            self.latest_deliveries.pop_front();
        }
    }

    pub fn calculate_pdr(&self) -> f32 {
        if self.latest_deliveries.len() < MEMORY_SIZE / 10 {
            return 0.0;
        }
        let dropped = self
            .latest_deliveries
            .iter()
            .filter(|&d| *d == FragmentDelivery::Dropped)
            .count();
        let total = self.latest_deliveries.len();
        dropped as f32 / total as f32
    }
}

#[derive(Clone, Debug)]
pub enum NodeType {
    Drone(Drone),
    Server(ApplicationType),
    Client(ApplicationType),
}

impl Display for NodeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            NodeType::Drone(drone) => {
                write!(f, "Drone: PDR = {:.2}%", drone.calculate_pdr() * 100.0)
            }
            NodeType::Server(application) => write!(f, "Server: {:?}", application),
            NodeType::Client(application) => write!(f, "Client: {:?}", application),
        }
    }
}

impl NodeType {
    pub fn new(simple_node_type: SimpleNodeType) -> Self {
        match simple_node_type {
            SimpleNodeType::Drone => NodeType::Drone(Drone::new()),
            SimpleNodeType::Server => NodeType::Server(ApplicationType::Unknown),
            SimpleNodeType::Client => NodeType::Client(ApplicationType::Unknown),
        }
    }

    pub fn weak_counter_part(&self) -> NodeType {
        match self {
            NodeType::Drone(drone) => NodeType::Drone(drone.clone()),
            NodeType::Server(_) => NodeType::Client(ApplicationType::Unknown),
            NodeType::Client(_) => NodeType::Server(ApplicationType::Unknown),
        }
    }

    pub fn strong_counter_part(&self) -> NodeType {
        match self {
            NodeType::Drone(drone) => NodeType::Drone(drone.clone()),
            NodeType::Server(application) => NodeType::Server(*application),
            NodeType::Client(application) => NodeType::Client(*application),
        }
    }

    pub fn to_simple(&self) -> SimpleNodeType {
        match self {
            NodeType::Drone(_) => SimpleNodeType::Drone,
            NodeType::Server(_) => SimpleNodeType::Server,
            NodeType::Client(_) => SimpleNodeType::Client,
        }
    }
    pub fn cost(&self) -> f32 {
        match self {
            NodeType::Drone(drone) => {
                const ALPHA: f32 = 0.1;
                const B: f32 = 2.0;
                let pdr = drone.calculate_pdr().min(0.9999);
                let retransmit_exp = 1.0 / (1.0 - pdr);
                (1.0 - ALPHA) * retransmit_exp.log(B) + ALPHA
            }
            _ => 0.0,
        }
    }
    pub fn application(&self) -> Option<&ApplicationType> {
        match self {
            NodeType::Server(application) => Some(application),
            NodeType::Client(application) => Some(application),
            _ => None,
        }
    }
    pub fn application_mut(&mut self) -> Option<&mut ApplicationType> {
        match self {
            NodeType::Server(application) => Some(application),
            NodeType::Client(application) => Some(application),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub node_type: NodeType,
}

impl Node {
    pub fn new(id: NodeId, node_type: NodeType) -> Self {
        Self { id, node_type }
    }

    pub fn is_other_useful(&self, other: &Self) -> bool {
        if self.id != other.id {
            unreachable!();
        }
        if self.node_type.to_simple() != other.node_type.to_simple() {
            return false;
        }
        if self.node_type.application() == Some(&ApplicationType::Unknown) {
            return true;
        }
        if self.node_type.application() == Some(&ApplicationType::Unwanted) {
            return false;
        }
        if let NodeType::Drone(drone) = &other.node_type {
            if !drone.latest_deliveries.is_empty() {
                return true;
            }
        }
        false
    }

    pub fn update_delivery(&mut self, delivery: FragmentDelivery) {
        if let NodeType::Drone(drone) = &mut self.node_type {
            drone.record_delivery(delivery);
        }
    }

    pub fn cost(&self) -> f32 {
        self.node_type.cost()
    }

    pub fn is_route_meaningful(&self, other: &Self) -> bool {
        if self.node_type.to_simple() == SimpleNodeType::Drone
            || other.node_type.to_simple() == SimpleNodeType::Drone
        {
            return false;
        }
        if self.node_type.weak_counter_part().to_simple() != other.node_type.to_simple() {
            return false;
        }
        if let (Some(app1), Some(app2)) =
            (self.node_type.application(), other.node_type.application())
        {
            return app1.compatible(app2);
        }
        false
    }
}
