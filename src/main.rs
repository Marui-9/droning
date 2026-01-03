mod application;
mod client;
mod initialization;
mod message;
mod server;
mod simulation_controller_alex;
mod simulation_controller_pilli;
mod tester;
use std::{env, str::FromStr};

use initialization::network_initializer::*;

#[derive(Debug, Clone, Copy)]
pub enum Topology {
    Butterfly,
    DoubleChain,
    StarDecagram,
    SubnetStars,
    SubnetTriangles,
    Tree,
    Custom,
}

impl Topology {
    fn to_path(self) -> &'static str {
        match self {
            Topology::Butterfly => "topologies/examples/butterfly/topology.toml",
            Topology::DoubleChain => "topologies/examples/double-chain/topology.toml",
            Topology::StarDecagram => "topologies/examples/star-decagram/topology.toml",
            Topology::SubnetStars => "topologies/examples/subnet-stars/topology.toml",
            Topology::SubnetTriangles => "topologies/examples/subnet-triangles/topology.toml",
            Topology::Tree => "topologies/examples/tree/topology.toml",
            Topology::Custom => "topologies/examples/config_3/topology.toml",
        }
    }
}

impl FromStr for Topology {
    type Err = Topology;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "butterfly" => Ok(Topology::Butterfly),
            "double-chain" => Ok(Topology::DoubleChain),
            "star-decagram" => Ok(Topology::StarDecagram),
            "subnet-stars" => Ok(Topology::SubnetStars),
            "subnet-triangles" => Ok(Topology::SubnetTriangles),
            "tree" => Ok(Topology::Tree),
            "custom" => Ok(Topology::Custom),
            _ => Err(Topology::DoubleChain),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum SimulationControllerType {
    Pilli,
    Shrimp,
    None,
}

impl FromStr for SimulationControllerType {
    type Err = SimulationControllerType;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "--pilli" => Ok(SimulationControllerType::Pilli),
            "--shrimp" => Ok(SimulationControllerType::Shrimp),
            _ => Err(SimulationControllerType::None),
        }
    }
}

fn main() {
    let args: Vec<_> = env::args().collect();

    let sim_con_type = match args.len() - 1 {
        0 => SimulationControllerType::None,
        1 => match args[1].parse() {
            Ok(sim_type) | Err(sim_type) => sim_type,
        },
        2 => match args[1].parse() {
            Ok(sim_type) => sim_type,
            Err(sim_type) => {
                println!("Invalid simulation controller type, defaulting to None");
                println!("Available simulation controllers: --pilli, --shrimp");
                sim_type
            }
        },
        _ => {
            panic!("Invalid argument provided. Please provide a path to a topology file.")
        }
    };

    match sim_con_type {
        SimulationControllerType::Pilli => {
            simulation_controller_pilli::main().ok();
        }
        SimulationControllerType::Shrimp => {
            simulation_controller_alex::main().ok();
        }
        SimulationControllerType::None => {
            let topology = args
                .get(1)
                .map(|arg| arg.parse().unwrap_or_else(|top| {
                    println!("Invalid topology, defaulting to DoubleChain");
                    println!("Available topologies: butterfly, double-chain, star-decagram, subnet-stars, subnet-triangles, tree");
                    top
                }))
                .unwrap_or(Topology::DoubleChain);
            start_without_simulation_controller(topology);
        }
    }
}

fn start_without_simulation_controller(topology: Topology) {
    println!("{}", topology.to_path());
    let info = start_actual_simulation(topology.to_path());

    for handle in info.handles.into_values() {
        handle.join().unwrap();
    }
}
