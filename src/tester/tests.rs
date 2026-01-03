use super::sandbox::{create_test_environment, PDRPolicy, TestNodeInstructions};
use crate::application::assembler::{Assembler, Disassembler};
use crate::application::routing::SourceRouter;
use crate::application::topology::node::{ApplicationType, Node, NodeType};
use crate::initialization::dummies::DummyHostCreator;
use crate::initialization::network_initializer::Runnable;
use crate::initialization::node_creators::{ActualDroneCreator, ActualServerCreator, DroneCreator};
use crate::message::base_message::Message;
use crate::message::chat_message::ChatRequest;
use crate::message::content_message::{ContentRequest, ContentResponse};
use crate::message::media_message::{MediaRequest, MediaResponse};
use crate::server::media_server::MediaServer;
use bagel_bomber::BagelBomber;
use crossbeam_channel::{Receiver, Sender};
use graph::AdjacencyVecGraph;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use wg_2024::controller::DroneEvent;
use wg_2024::drone::Drone;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::NodeType as SimpleNodeType;
use wg_2024::packet::NodeType::Client;
use wg_2024::packet::{FloodRequest, Fragment, NackType, Packet, PacketType, FRAGMENT_DSIZE};

struct BagelBomberCreator {
    controller_send: Sender<DroneEvent>,
}

impl DroneCreator for BagelBomberCreator {
    fn new(controller_send: Sender<DroneEvent>) -> Self {
        Self { controller_send }
    }

    fn create_drone(
        &mut self,
        id: wg_2024::network::NodeId,
        controller_recv: Receiver<wg_2024::controller::DroneCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<wg_2024::network::NodeId, Sender<Packet>>,
        pdr: f32,
    ) -> Box<dyn Runnable> {
        Box::new(BagelBomber::new(
            id,
            self.controller_send.clone(),
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
        ))
    }
}

#[test]
fn flooding() {
    let client = TestNodeInstructions::with_random_id(
        &[1],
        |id,
         _controller_send,
         _controller_recv,
         packet_recv: Receiver<Packet>,
         packet_send: HashMap<u8, Sender<Packet>>| {
            println!("Client running");
            packet_send
                .get(&1)
                .unwrap()
                .send(Packet {
                    session_id: 0,
                    routing_header: SourceRoutingHeader {
                        hops: Vec::new(),
                        hop_index: 0,
                    },
                    pack_type: PacketType::FloodRequest(FloodRequest {
                        flood_id: 0,
                        initiator_id: id,
                        path_trace: vec![(id, Client)],
                    }),
                })
                .ok();

            thread::sleep(Duration::from_millis(100));

            let response_received = false;

            let mut router =
                SourceRouter::new(Node::new(id, NodeType::Client(ApplicationType::Unknown)));

            for packet in packet_recv.try_iter() {
                router.update_graph(&packet);
            }

            println!("----- GRAPH CREATED -----");
            // router.show_graph();

            assert!(response_received);
        },
    );
    create_test_environment::<ActualDroneCreator, DummyHostCreator, ActualServerCreator>(
        "topologies/examples/double-chain/topology.toml",
        vec![client],
        PDRPolicy::Zero,
    )
}

#[test]
fn client_server_ping() {
    let client = TestNodeInstructions::with_node_id(
        40,
        &[3],
        |id,
         _controller_send,
         _controller_recv,
         packet_recv: Receiver<Packet>,
         packet_send: HashMap<u8, Sender<Packet>>| {
            thread::sleep(Duration::from_millis(1000));
            println!("Client running");
            packet_send
                .get(&3)
                .unwrap()
                .send(Packet {
                    session_id: 0,
                    routing_header: SourceRoutingHeader {
                        hops: vec![40, 3, 4, 6, 8, 50],
                        hop_index: 1,
                    },
                    pack_type: PacketType::MsgFragment(Fragment {
                        fragment_index: 0,
                        total_n_fragments: 1,
                        length: FRAGMENT_DSIZE as u8,
                        data: [0; FRAGMENT_DSIZE],
                    }),
                })
                .ok();

            thread::sleep(Duration::from_millis(5000));

            let mut response_received = false;

            for packet in packet_recv.try_iter() {
                if let PacketType::MsgFragment(response) = packet.pack_type {
                    println!("Client {} received {:?}", id, response);
                    assert_eq!(response.fragment_index, 0);
                    assert_eq!(response.total_n_fragments, 1);
                    assert_eq!(response.data, [1; FRAGMENT_DSIZE]);
                    response_received = true;
                }
            }

            assert!(response_received);
        },
    );

    let server = TestNodeInstructions::with_node_id(
        50,
        &[8],
        |id,
         _controller_send,
         _controller_recv,
         packet_recv: Receiver<Packet>,
         packet_send: HashMap<u8, Sender<Packet>>| {
            thread::sleep(Duration::from_millis(1000));

            println!("Server running");

            let mut request_received = false;

            for packet in packet_recv.try_iter() {
                if let PacketType::MsgFragment(response) = packet.pack_type {
                    println!("Server {} received {:?}", id, response);

                    assert_eq!(response.fragment_index, 0);
                    assert_eq!(response.total_n_fragments, 1);
                    assert_eq!(response.data, [0; FRAGMENT_DSIZE]);

                    request_received = true;

                    packet_send
                        .get(&8)
                        .unwrap()
                        .send(Packet {
                            session_id: 0,
                            routing_header: SourceRoutingHeader {
                                hops: vec![50, 8, 7, 5, 3, 40],
                                hop_index: 1,
                            },
                            pack_type: PacketType::MsgFragment(Fragment {
                                fragment_index: 0,
                                total_n_fragments: 1,
                                length: FRAGMENT_DSIZE as u8,
                                data: [1; FRAGMENT_DSIZE],
                            }),
                        })
                        .ok();
                }
            }

            assert!(request_received);
        },
    );

    create_test_environment::<ActualDroneCreator, DummyHostCreator, DummyHostCreator>(
        "topologies/examples/double-chain/topology.toml",
        vec![client, server],
        PDRPolicy::Zero,
    )
}

#[test]
fn continuous_ping() {
    let ping_count = 600;

    let client = TestNodeInstructions::with_node_id(
        40,
        &[3],
        move |id,
              _controller_send,
              _controller_recv,
              packet_recv: Receiver<Packet>,
              packet_send: HashMap<u8, Sender<Packet>>| {
            println!("Client running");

            for i in 0..ping_count {
                let packet = Packet::new_fragment(
                    SourceRoutingHeader::with_first_hop(vec![40, 3, 4, 6, 8, 50]),
                    0,
                    Fragment::from_string(i, ping_count, "Hello, world!".to_string()),
                );

                packet_send.get(&3).unwrap().send(packet).ok();

                thread::sleep(Duration::from_millis(1000));

                for packet in packet_recv.try_iter() {
                    if let PacketType::MsgFragment(response) = packet.pack_type {
                        println!("Client {} received {}", id, response);
                    }
                }
            }

            println!("Client {} ending simulation", id);
        },
    );

    let server = TestNodeInstructions::with_node_id(
        50,
        &[8],
        |id,
         _controller_send,
         _controller_recv,
         packet_recv: Receiver<Packet>,
         packet_send: HashMap<u8, Sender<Packet>>| {
            println!("Server running");

            thread::sleep(Duration::from_millis(500));

            for in_packet in packet_recv.iter() {
                if let PacketType::MsgFragment(request) = in_packet.pack_type {
                    println!("Server {} received {}", id, request);

                    let packet = Packet::new_fragment(
                        SourceRoutingHeader::with_first_hop(vec![50, 8, 7, 5, 3, 40]),
                        0,
                        request.clone(),
                    );

                    let send = packet_send.get(&8).unwrap();

                    send.send(packet).ok();

                    if request.fragment_index == request.total_n_fragments - 1 {
                        while !send.is_empty() {
                            thread::sleep(Duration::from_millis(100));
                        }
                    }
                }
            }

            thread::sleep(Duration::from_millis(1000));

            println!("Server {} ending simulation", id);
        },
    );

    create_test_environment::<ActualDroneCreator, DummyHostCreator, DummyHostCreator>(
        "topologies/examples/double-chain/topology.toml",
        vec![client, server],
        PDRPolicy::Severe,
    )
}

#[test]
fn string_message() {
    let message = Message::new(0, 0, 0, ChatRequest::Register("daw".to_string()));

    let str_message = message.to_string_message();

    println!("{}", str_message);
}

#[test]
fn download_chad_face() {
    let client = TestNodeInstructions::with_node_id(
        120,
        &[3, 5],
        |id,
         _command_send,
         _command_recv,
         packet_recv: Receiver<Packet>,
         packet_send: HashMap<u8, Sender<Packet>>| {
            let message = Message::new(
                id,
                200,
                0,
                ContentRequest::MediaRequest(MediaRequest::Media("chadface.png".to_string())),
            );
            let mut disassembler = Disassembler::new();

            let fragments = disassembler.disassembly(message);
            for fragment in fragments {
                packet_send
                    .get(&3)
                    .unwrap()
                    .send(Packet::new_fragment(
                        SourceRoutingHeader::with_first_hop(vec![120, 3, 5, 7, 9, 200]),
                        0,
                        fragment,
                    ))
                    .ok();
            }

            let mut retransmit_count = 0;

            // re-sending
            for packet in packet_recv.iter() {
                match packet.pack_type {
                    PacketType::Nack(nack) => {
                        if let NackType::Dropped = nack.nack_type {
                            retransmit_count += 1;

                            let fragment =
                                disassembler.get_fragment(0, nack.fragment_index).unwrap();
                            packet_send
                                .get(&3)
                                .unwrap()
                                .send(Packet::new_fragment(
                                    SourceRoutingHeader::with_first_hop(vec![120, 3, 5, 7, 9, 250]),
                                    0,
                                    fragment,
                                ))
                                .ok();
                        }
                    }
                    PacketType::Ack(ack) => {
                        disassembler.forget_fragment(0, ack.fragment_index);
                        if !disassembler.has_fragments(0) {
                            break;
                        }
                    }
                    _ => {}
                }
            }

            println!("Retransmitted {} fragments", retransmit_count);

            let mut assembler = Assembler::new();

            let mut packet_count = 0;

            for packet in packet_recv.iter() {
                let session_id = packet.session_id;
                if let PacketType::MsgFragment(fragment) = packet.pack_type {
                    packet_count += 1;
                    if let Some(message_res) = assembler.insert_fragment(session_id, fragment) {
                        match message_res {
                            Ok(message) => {
                                assert_eq!(message.source_id, 250);
                                assert_eq!(message.destination_id, id);
                                assert_eq!(message.session_id, session_id);
                                if let ContentResponse::MediaResponse(MediaResponse::Media(media)) =
                                    message.content
                                {
                                    println!(
                                        "Client {} received media with {} bytes in {} fragments",
                                        id,
                                        media.len(),
                                        packet_count
                                    );
                                }
                            }
                            Err(_) => {
                                println!("Error while assembling message");
                            }
                        }

                        break;
                    }
                }
            }

            println!("Client {} ending simulation", id);

            thread::sleep(Duration::from_secs(1));
        },
    );

    let server = TestNodeInstructions::with_node_id(
        250,
        &[9],
        |id, controller_send, controller_recv, packet_recv, packet_send| {
            let mut server = MediaServer::with_default_behaviour(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
            );

            server.run();
        },
    );

    create_test_environment::<ActualDroneCreator, DummyHostCreator, DummyHostCreator>(
        "topologies/examples/double-chain/topology.toml",
        vec![client, server],
        PDRPolicy::Zero,
    );
}

#[test]
fn pdr_estimate() {
    let client = TestNodeInstructions::with_node_id(
        150,
        &[2],
        |id,
         _controller_send,
         _controller_recv,
         packet_recv: Receiver<Packet>,
         packet_send: HashMap<u8, Sender<Packet>>| {
            let mut router =
                SourceRouter::new(Node::new(id, NodeType::Client(ApplicationType::Unknown)));

            for sender in packet_send.values() {
                sender
                    .send(Packet::new_flood_request(
                        SourceRoutingHeader::empty_route(),
                        0,
                        FloodRequest::initialize(0, 150, SimpleNodeType::Client),
                    ))
                    .ok();
            }

            for packet in packet_recv.iter() {
                match packet.pack_type {
                    PacketType::FloodResponse(_) => {
                        router.update_graph(&packet);
                    }
                    PacketType::Ack(_) => break,
                    _ => unreachable!(),
                }
            }

            println!("----- CLIENT GRAPH CREATED -----");

            for packet in packet_recv.iter() {
                router.update_graph(&packet);

                if let PacketType::FloodRequest(request) = packet.pack_type {
                    let mut response = request
                        .get_incremented(150, SimpleNodeType::Client)
                        .generate_response(0);
                    response.routing_header.increase_hop_index();
                    let ack = Packet::new_ack(response.routing_header.clone(), 0, 0);

                    packet_send.get(&2).unwrap().send(response).ok();
                    thread::sleep(Duration::from_millis(1000));
                    packet_send.get(&2).unwrap().send(ack).ok();
                    break;
                }
            }

            router.calculate_routes();

            let ping = Fragment::from_string(0, 1, "Hello World!".to_string());

            for i in 0..10_000 {
                let mut ping_packet =
                    Packet::new_fragment(router.get_best_route(250).unwrap(), i, ping.clone());

                ping_packet.routing_header.increase_hop_index();

                packet_send[&ping_packet.routing_header.current_hop().unwrap()]
                    .send(ping_packet)
                    .ok();

                let mut response = None;

                for resp in packet_recv.iter() {
                    router.update_graph(&resp);

                    match resp.pack_type {
                        PacketType::Ack(ack) => {
                            assert_eq!(ack.fragment_index, 0);
                            break;
                        }
                        PacketType::Nack(nack) => match nack.nack_type {
                            NackType::Dropped => {
                                let mut packet = Packet::new_fragment(
                                    router.get_best_route(250).unwrap(),
                                    i,
                                    ping.clone(),
                                );

                                packet.routing_header.increase_hop_index();

                                packet_send[&packet.routing_header.current_hop().unwrap()]
                                    .send(packet)
                                    .ok();
                            }
                            _ => unreachable!(),
                        },
                        PacketType::MsgFragment(_) => response = Some(resp),
                        _ => unreachable!(),
                    }
                }

                response = response.or_else(|| packet_recv.recv().ok());

                assert!(response.is_some());

                let response = response.unwrap();

                assert!(matches!(response.pack_type, PacketType::MsgFragment(_)));

                router.update_graph(&response);

                let mut ack = Packet::new_ack(router.get_best_route(250).unwrap(), i, 0);

                ack.routing_header.increase_hop_index();

                packet_send[&ack.routing_header.current_hop().unwrap()]
                    .send(ack)
                    .ok();

                if i % 1000 == 0 && i != 0 {
                    let (count, sum) = router
                        .graph
                        .values()
                        .map(|value| value.cost() - 1.0)
                        .filter(|&pdr| pdr > f32::EPSILON)
                        .fold((0usize, 0.0), |(count, sum), current| {
                            (count + 1, sum + current)
                        });
                    let pdr_estimate = sum / count as f32;

                    println!(
                        "current pdr estimate for server is {:.2}%",
                        pdr_estimate * 100.0
                    );
                }

                // print!("{}", ".".red());
            }
        },
    );

    let server = TestNodeInstructions::with_node_id(
        250,
        &[7],
        |id,
         _controller_send,
         _controller_recv,
         packet_recv: Receiver<Packet>,
         packet_send: HashMap<u8, Sender<Packet>>| {
            let mut router =
                SourceRouter::new(Node::new(id, NodeType::Server(ApplicationType::Unknown)));

            thread::sleep(Duration::from_millis(500));

            for packet in packet_recv.iter() {
                if let PacketType::FloodRequest(request) = packet.pack_type {
                    let mut response = request
                        .get_incremented(250, SimpleNodeType::Server)
                        .generate_response(0);
                    response.routing_header.increase_hop_index();
                    let ack = Packet::new_ack(response.routing_header.clone(), 0, 0);

                    packet_send.get(&7).unwrap().send(response).ok();
                    thread::sleep(Duration::from_millis(1000));
                    packet_send.get(&7).unwrap().send(ack).ok();
                    break;
                }
            }

            for sender in packet_send.values() {
                sender
                    .send(Packet::new_flood_request(
                        SourceRoutingHeader::empty_route(),
                        0,
                        FloodRequest::initialize(0, 250, SimpleNodeType::Server),
                    ))
                    .ok();
            }

            for packet in packet_recv.iter() {
                match packet.pack_type {
                    PacketType::FloodResponse(_) => {
                        router.update_graph(&packet);
                    }
                    PacketType::Ack(_) => break,
                    _ => unreachable!(),
                }
            }

            println!("----- SERVER GRAPH CREATED -----");

            router.calculate_routes();

            let pong = Fragment::from_string(0, 1, "Hello To You!".to_string());

            let mut request_opt = None;

            for i in 0..10_000 {
                request_opt = request_opt.or_else(|| packet_recv.recv().ok());

                assert!(request_opt.is_some());

                let request = request_opt.take().unwrap();

                assert!(matches!(request.pack_type, PacketType::MsgFragment(_)));

                router.update_graph(&request);

                let mut ack = Packet::new_ack(router.get_best_route(150).unwrap(), i, 0);

                ack.routing_header.increase_hop_index();

                packet_send[&ack.routing_header.current_hop().unwrap()]
                    .send(ack)
                    .ok();

                if i % 1000 == 0 && i != 0 {
                    let (count, sum) = router
                        .graph
                        .values()
                        .map(|value| value.cost() - 1.0)
                        .filter(|&pdr| pdr > f32::EPSILON)
                        .fold((0usize, 0.0), |(count, sum), current| {
                            (count + 1, sum + current)
                        });
                    let pdr_estimate = sum / count as f32;

                    println!(
                        "current pdr estimate for server is {:.2}%",
                        pdr_estimate * 100.0
                    );
                }

                let mut packet =
                    Packet::new_fragment(router.get_best_route(150).unwrap(), i, pong.clone());

                packet.routing_header.increase_hop_index();

                packet_send[&packet.routing_header.current_hop().unwrap()]
                    .send(packet)
                    .ok();

                for req in packet_recv.iter() {
                    router.update_graph(&req);

                    match req.pack_type {
                        PacketType::Ack(ack) => {
                            assert_eq!(ack.fragment_index, 0);
                            break;
                        }
                        PacketType::Nack(nack) => match nack.nack_type {
                            NackType::Dropped => {
                                let mut packet = Packet::new_fragment(
                                    router.get_best_route(150).unwrap(),
                                    i,
                                    pong.clone(),
                                );

                                packet.routing_header.increase_hop_index();

                                packet_send[&packet.routing_header.current_hop().unwrap()]
                                    .send(packet)
                                    .ok();
                            }
                            _ => unreachable!(),
                        },
                        PacketType::MsgFragment(_) => request_opt = Some(req),
                        _ => unreachable!(),
                    }
                }
            }
        },
    );

    create_test_environment::<BagelBomberCreator, DummyHostCreator, DummyHostCreator>(
        "topologies/examples/double-chain/topology.toml",
        vec![client, server],
        PDRPolicy::Constant(0.5),
    );
}

#[test]
fn route_with_host() {
    let graph: AdjacencyVecGraph<NodeId, Node> = AdjacencyVecGraph::from_iter([
        (0, (Node::new(0, NodeType::Client(ApplicationType::Chat)), vec![3, 5])),
        (1, (Node::new(1, NodeType::Client(ApplicationType::Chat)), vec![3, 4])),
        (2, (Node::new(2, NodeType::Server(ApplicationType::Chat)), vec![4, 8])),
        (3, (Node::new(3, NodeType::Drone(Default::default())), vec![0, 1, 4])),
        (4, (Node::new(4, NodeType::Drone(Default::default())), vec![1, 2, 3])),
        (5, (Node::new(5, NodeType::Drone(Default::default())), vec![0, 6])),
        (6, (Node::new(6, NodeType::Drone(Default::default())), vec![5, 7])),
        (7, (Node::new(7, NodeType::Drone(Default::default())), vec![6, 8])),
        (8, (Node::new(8, NodeType::Drone(Default::default())), vec![2, 7])),
    ].into_iter());

    let mut router = SourceRouter::new(Node::new(0, NodeType::Client(ApplicationType::Chat)));
    router.graph = graph;

    let count = router.calculate_routes();

    println!("Calculated {} routes", count);

    let route = router.get_best_route(2).unwrap();

    println!("Route to 2: {:?}", route);

    let route = router.get_best_route(2).unwrap();

    println!("Route to 2: {:?}", route);

    let route = router.get_best_route(2).unwrap();

    println!("Route to 2: {:?}", route);
}
