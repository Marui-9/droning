use super::topology::information::{Information, InformationPack};
use super::topology::node::{self, Node, NodeType};
use graph::{AdjacencyVecGraph, ReferenceGraph};
use std::collections::{HashMap, HashSet};
use wg_2024::network::{NodeId, SourceRoutingHeader};

#[derive(Clone, Default)]
pub struct Route {
    hops: Vec<NodeId>,
}

impl Route {
    pub fn new(hops: Vec<NodeId>) -> Self {
        Route { hops }
    }
    pub fn cost(&self, graph: &AdjacencyVecGraph<NodeId, Node>) -> f32 {
        self.hops.iter().map(|id| graph[id].cost()).sum()
    }
    pub fn to_source_routing_header(&self) -> SourceRoutingHeader {
        SourceRoutingHeader::initialize(self.hops.clone())
    }
    pub fn source(&self) -> Option<NodeId> {
        self.hops.first().cloned()
    }
    pub fn destination(&self) -> Option<NodeId> {
        self.hops.last().cloned()
    }
    pub fn get_incremented(&self, last_hop: NodeId) -> Route {
        let mut hops = self.hops.clone();
        hops.push(last_hop);
        Route { hops }
    }

    fn contains(&self, adj: &NodeId) -> bool {
        self.hops.contains(adj)
    }

    fn contains_edge(&self, from: NodeId, to: NodeId) -> bool {
        self.hops.windows(2).any(|window| window == [from, to])
    }
}

impl From<Route> for SourceRoutingHeader {
    fn from(val: Route) -> Self {
        SourceRoutingHeader::initialize(val.hops)
    }
}

impl From<&Route> for SourceRoutingHeader {
    fn from(val: &Route) -> Self {
        val.clone().into()
    }
}

pub struct SourceRouter {
    #[cfg(test)]
    pub graph: AdjacencyVecGraph<NodeId, Node>,
    #[cfg(not(test))]
    graph: AdjacencyVecGraph<NodeId, Node>,
    source_id: NodeId,
    routes: Vec<Route>,
    request_count: usize,
}

impl SourceRouter {
    pub fn new(source: Node) -> Self {
        let mut graph = AdjacencyVecGraph::new();
        let source_id = source.id;
        graph.add_node(source.id, source);
        Self {
            graph,
            source_id,
            routes: Vec::new(),
            request_count: 0,
        }
    }
    pub fn get_best_route(&mut self, destination: NodeId) -> Option<SourceRoutingHeader> {
        let routes: Vec<_> = self
            .routes
            .iter()
            .filter(|route| route.destination() == Some(destination))
            .collect();

        let min_cost = routes.first()?.cost(&self.graph);

        #[allow(clippy::float_equality_without_abs)]
        let minimal_routes: Vec<_> = routes
            .into_iter()
            .take_while(|route| route.cost(&self.graph) - min_cost < f32::EPSILON)
            .collect();

        let route = minimal_routes
            .get(self.request_count % minimal_routes.len())
            .unwrap();

        self.request_count = self.request_count.overflowing_add(1).0;

        Some(route.to_source_routing_header())
    }
    pub fn update_graph(&mut self, infos: &impl InformationPack) {
        let source = self.graph[&self.source_id].clone();
        for info in infos.get_information(&source) {
            match info {
                Information::AddNode(node) => {
                    self.add_node(node);
                }
                Information::AddEdge(from, to) => self.add_edge(from, to),
                Information::RemoveEdge(from, to) => {
                    self.remove_edge(from, to);
                }
            }
        }
    }

    pub fn add_edge(&mut self, from: u8, to: u8) {
        self.graph.add_undirected_edge(from, to)
    }

    pub fn remove_edge(&mut self, from: u8, to: u8) {
        self.graph.remove_undirected_edge(&from, &to);
        self.routes.retain(|route| !route.contains_edge(from, to));
    }

    pub fn add_node(&mut self, node: Node) {
        if let Some(current) = self.graph.get_mut(&node.id) {
            if current.is_other_useful(&node) {
                match current.node_type {
                    NodeType::Drone(ref mut drone) => {
                        if let NodeType::Drone(new_drone) = node.node_type {
                            drone.merge_drone(new_drone);
                        }
                    }
                    NodeType::Server(_) | NodeType::Client(_) => {
                        self.graph.add_node(node.id, node);
                    }
                }
            }
        } else {
            self.graph.add_node(node.id, node);
        }
    }
    pub fn calculate_routes(&mut self) -> usize {
        self.routes = calculate_routes(&self.graph, self.source_id);
        let source_node = &self.graph[&self.source_id];
        self.routes.retain(|route| {
            let destination_id = route.destination().unwrap();
            let destination_node = &self.graph[&destination_id];
            let host_count = route.hops.iter().filter(|id| !matches!(self.graph[id].node_type, NodeType::Drone(_))).count();
            host_count == 2 && source_node.is_route_meaningful(destination_node)
        });
        self.routes
            .sort_by(|a, b| a.cost(&self.graph).total_cmp(&b.cost(&self.graph)));

        self.routes.len()
    }
    pub fn unwanted_node(&mut self, node_id: &NodeId) {
        if let Some(node) = self.graph.get_mut(node_id) {
            if let Some(application) = node.node_type.application_mut() {
                *application = node::ApplicationType::Unwanted;
            }
        }
        self.routes
            .retain(|route| route.destination() != Some(*node_id));
    }
    pub fn forget_topology(&mut self) {
        let source = self.graph.remove_node(&self.source_id).unwrap();
        self.graph.clear();
        self.graph.add_node(self.source_id, source);
        self.routes.clear();
    }
    pub fn print_reachable_servers(&self) {
        println!(
            "Reachable Servers are: {:?}",
            self.routes
                .iter()
                .map(|route| route.destination().unwrap())
                .collect::<HashSet<_>>()
        )
    }

    pub(crate) fn can_reach(&self, destination_id: u8) -> bool {
        self.routes
            .iter()
            .any(|route| route.destination() == Some(destination_id))
    }

    #[cfg(test)]
    #[allow(unused)]
    pub fn show_graph(&self) {
        println!("{:?}", self.graph);
    }
}

fn extend_route<G: ReferenceGraph<NodeKey = NodeId>>(graph: &G, route: Route) -> Vec<Route> {
    let last_node_id = route.destination().unwrap();
    graph
        .adjacents(&last_node_id)
        .copied()
        .filter(|adj| !route.contains(adj))
        .map(|adj| route.get_incremented(adj))
        .collect()
}

fn calculate_routes<G: ReferenceGraph<NodeKey = NodeId>>(
    graph: &G,
    source_id: NodeId,
) -> Vec<Route> {
    let mut routes = HashMap::with_capacity(15);
    routes.insert(1, vec![Route::new(vec![source_id])]);

    for i in 1u8.. {
        let old_routes = routes.get(&i).unwrap();
        let mut new_routes = Vec::new();

        for route in old_routes.iter() {
            new_routes.extend(extend_route(graph, route.clone()));
        }

        if new_routes.is_empty() {
            break;
        }

        routes.insert(i + 1, new_routes);
    }

    routes.into_values().flatten().collect()
}
