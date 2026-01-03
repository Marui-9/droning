use std::sync::{Arc, Mutex};

use wg_2024::network::NodeId;

pub type TurnHandlerArc = Arc<Mutex<TurnHandler>>;

pub struct TurnHandler {
    nodes: Vec<NodeId>,
    current_turn: usize,
}

impl TurnHandler {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            current_turn: 0,
        }
    }

    pub fn current_turn(&self) -> NodeId {
        self.nodes[self.current_turn]
    }

    pub fn yield_turn(&mut self) {
        self.current_turn = (self.current_turn + 1) % self.nodes.len();
    }

    pub fn subscribe(&mut self, node: NodeId) {
        self.nodes.push(node);
    }

    pub fn unsubscribe(&mut self, node: NodeId) {
        self.nodes.retain(|&x| x != node);
    }
}

pub fn create_turn_handler() -> TurnHandlerArc {
    Arc::new(Mutex::new(TurnHandler::new()))
}
