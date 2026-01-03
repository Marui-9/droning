use std::iter::zip;

use super::node::{Drone, FragmentDelivery, Node, NodeType};
use wg_2024::{
    network::{NodeId, SourceRoutingHeader},
    packet::{Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, Packet, PacketType},
};

pub enum Information {
    AddNode(Node),
    AddEdge(NodeId, NodeId),
    RemoveEdge(NodeId, NodeId),
}

pub trait InformationPack {
    fn get_information(&self, source_node: &Node) -> Vec<Information>;
}

impl InformationPack for Packet {
    fn get_information(&self, source_node: &Node) -> Vec<Information> {
        match &self.pack_type {
            PacketType::MsgFragment(fragment) => {
                (&self.routing_header, fragment).get_information(source_node)
            }
            PacketType::Ack(ack) => (&self.routing_header, ack).get_information(source_node),
            PacketType::Nack(nack) => (&self.routing_header, nack).get_information(source_node),
            PacketType::FloodRequest(request) => request.get_information(source_node),
            PacketType::FloodResponse(response) => response.get_information(source_node),
        }
    }
}

impl InformationPack for (&SourceRoutingHeader, &Fragment) {
    fn get_information(&self, source_node: &Node) -> Vec<Information> {
        let (routing_header, _fragment) = self;
        let mut iter = routing_header.hops.iter();

        let edges = iter
            .clone()
            .zip(iter.clone().skip(1))
            .map(|(from, to)| Information::AddEdge(*from, *to));

        let mut result = Vec::new();

        let first_id = *iter.next().unwrap();
        result.push(Information::AddNode(Node::new(
            first_id,
            source_node.node_type.weak_counter_part(),
        )));

        for id in iter {
            result.push(Information::AddNode(Node::new(
                *id,
                NodeType::Drone(Drone::with_delivery(vec![FragmentDelivery::Forwarded])),
            )));
        }

        result.pop();

        result.extend(edges);

        result
    }
}

impl InformationPack for (&SourceRoutingHeader, &Ack) {
    fn get_information(&self, source_node: &Node) -> Vec<Information> {
        let (routing_header, _ack) = self;
        let mut iter = routing_header.hops.iter();

        let edges = iter
            .clone()
            .zip(iter.clone().skip(1))
            .map(|(from, to)| Information::AddEdge(*from, *to));

        let mut result = Vec::new();

        let first_id = *iter.next().unwrap();
        result.push(Information::AddNode(Node::new(
            first_id,
            source_node.node_type.strong_counter_part(),
        )));

        for id in iter {
            result.push(Information::AddNode(Node::new(
                *id,
                NodeType::Drone(Drone::new()),
            )));
        }

        result.pop();

        result.extend(edges);

        result
    }
}

impl InformationPack for (&SourceRoutingHeader, &Nack) {
    fn get_information(&self, source_node: &Node) -> Vec<Information> {
        let (routing_header, nack) = self;
        match &nack.nack_type {
            NackType::ErrorInRouting(not_connected) => {
                let mut iter = routing_header.hops.iter();

                let edges = iter
                    .clone()
                    .zip(iter.clone().skip(1))
                    .map(|(from, to)| Information::AddEdge(*from, *to));

                let mut result = Vec::new();

                let first_id = *iter.next().unwrap();
                result.push(Information::AddNode(Node::new(
                    first_id,
                    NodeType::Drone(Drone::new()),
                )));

                for id in iter {
                    result.push(Information::AddNode(Node::new(
                        *id,
                        NodeType::Drone(Drone::with_delivery(vec![FragmentDelivery::Forwarded])),
                    )));
                }

                result.pop();

                result.extend(edges);

                result.push(Information::RemoveEdge(first_id, *not_connected));

                result
            }
            NackType::DestinationIsDrone => {
                let iter = routing_header.hops.iter();

                let edges = iter
                    .clone()
                    .zip(iter.clone().skip(1))
                    .map(|(from, to)| Information::AddEdge(*from, *to));

                let mut result = Vec::new();

                for id in iter {
                    result.push(Information::AddNode(Node::new(
                        *id,
                        NodeType::Drone(Drone::with_delivery(vec![FragmentDelivery::Forwarded])),
                    )));
                }

                result.pop();

                result.extend(edges);

                result
            }
            NackType::Dropped => {
                let mut iter = routing_header.hops.iter();

                let edges = iter
                    .clone()
                    .zip(iter.clone().skip(1))
                    .map(|(from, to)| Information::AddEdge(*from, *to));

                let mut result = Vec::new();

                let first_id = *iter.next().unwrap();
                result.push(Information::AddNode(Node::new(
                    first_id,
                    NodeType::Drone(Drone::with_delivery(vec![FragmentDelivery::Dropped])),
                )));

                for id in iter {
                    result.push(Information::AddNode(Node::new(
                        *id,
                        NodeType::Drone(Drone::with_delivery(vec![FragmentDelivery::Forwarded])),
                    )));
                }

                result.pop();

                result.extend(edges);

                result
            }
            NackType::UnexpectedRecipient(who) => {
                let mut iter = routing_header.hops.iter();

                let edges = iter
                    .clone()
                    .zip(iter.clone().skip(1))
                    .map(|(from, to)| Information::AddEdge(*from, *to));

                let mut result = Vec::new();

                let first_id = *iter.next().unwrap();
                result.push(Information::AddNode(Node::new(
                    first_id,
                    NodeType::Drone(Drone::with_delivery(vec![FragmentDelivery::Dropped])),
                )));

                for id in iter {
                    result.push(Information::AddNode(Node::new(
                        *id,
                        NodeType::Drone(Drone::with_delivery(vec![FragmentDelivery::Forwarded])),
                    )));
                }

                result.pop();

                let my_application = source_node.node_type.application().unwrap();
                let mut other_node = Node::new(*who, source_node.node_type.weak_counter_part());
                *other_node.node_type.application_mut().unwrap() = *my_application;

                result.push(Information::AddNode(other_node));

                result.extend(edges);

                result
            }
        }
    }
}

impl InformationPack for &FloodRequest {
    fn get_information(&self, _source_node: &Node) -> Vec<Information> {
        self.path_trace
            .iter()
            .map(|(id, node_type)| Information::AddNode(Node::new(*id, NodeType::new(*node_type))))
            .chain(
                zip(
                    self.path_trace.iter().map(|(id, _)| *id),
                    self.path_trace.iter().map(|(id, _)| *id).skip(1),
                )
                .map(|(from, to)| Information::AddEdge(from, to)),
            )
            .collect()
    }
}

impl InformationPack for &FloodResponse {
    fn get_information(&self, _source_node: &Node) -> Vec<Information> {
        self.path_trace
            .iter()
            .map(|(id, node_type)| Information::AddNode(Node::new(*id, NodeType::new(*node_type))))
            .chain(
                zip(
                    self.path_trace.iter().map(|(id, _)| *id),
                    self.path_trace.iter().map(|(id, _)| *id).skip(1),
                )
                .map(|(from, to)| Information::AddEdge(from, to)),
            )
            .collect()
    }
}
