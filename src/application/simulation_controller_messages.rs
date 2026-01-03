use crate::message::base_message::Message;
use crossbeam_channel::Sender;
use wg_2024::{network::NodeId, packet::Packet};

#[derive(Debug)]
pub enum HostEvent {
    MessageSent(Message<String>),
    MessageReceived(Message<String>),
    FloodInitiated(NodeId, u64),
}

#[derive(Debug)]
pub enum HostCommand {
    Crash,
    AddConnectedDrone(NodeId, Sender<Packet>),
    RemoveConnectedDrone(NodeId),
}
