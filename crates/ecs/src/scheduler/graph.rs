use std::fmt::Write;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::component::ComponentId;
use crate::resource::ResourceId;
use crate::system::{System, SystemId};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AccessType {
    None,
    Entity,
    World,
    Component(ComponentId),
    Resource(ResourceId),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct AccessDesc {
    pub(crate) ty: AccessType,
    pub(crate) exclusive: bool,
}

impl AccessDesc {
    pub fn has_conflict(&self, other: &AccessDesc) -> bool {
        // `World` conflicts with every other resource.
        let conflict_ty = self.ty == other.ty || self.ty == AccessType::World;
        println!(
            "is_conflict_ty: {conflict_ty}, {:?} {:?}",
            self.ty, other.ty
        );
        conflict_ty && (self.exclusive || other.exclusive)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GraphNode {
    pub sid: SystemId,
    pub access: SmallVec<[AccessDesc; 4]>,
}

#[derive(Debug, Clone, Default)]
pub struct ScheduleGraph {
    nodes: Vec<GraphNode>,
    adjacency: Vec<Vec<usize>>,
    in_degrees: Vec<usize>,
}

impl ScheduleGraph {
    pub fn new() -> ScheduleGraph {
        ScheduleGraph::default()
    }

    pub(crate) fn add_node(&mut self, node: GraphNode) {
        self.nodes.push(node);
        self.adjacency.push(Vec::new());
        self.in_degrees.push(0);
    }

    fn build_dependencies(&mut self) {
        let mut last_writer: FxHashMap<AccessType, usize> = FxHashMap::default();
        let mut readers: FxHashMap<AccessType, Vec<usize>> = FxHashMap::default();

        for (i, node) in self.nodes.iter().enumerate() {
            for access in &node.access {
                if access.exclusive {
                    println!("adding writer: {access:?}");

                    if let Some(writer) = last_writer.get(&access.ty) {
                        self.adjacency[*writer].push(i);
                        self.in_degrees[i] += 1;
                    }

                    if let Some(nodes) = readers.get(&access.ty) {
                        for read in nodes {
                            self.adjacency[*read].push(i);
                            self.in_degrees[i] += 1;
                        }
                    } else if let Some(nodes) = readers.get(&AccessType::World) {
                        for read in nodes {
                            self.adjacency[*read].push(i);
                            self.in_degrees[i] += 1;
                        }
                    }

                    last_writer.insert(access.ty, i);
                } else {
                    if access.ty == AccessType::World {
                        // Check if there is any writer
                        for writer in last_writer.values() {
                            self.adjacency[*writer].push(i);
                            self.in_degrees[i] += 1;
                        }
                    } else if let Some(writer) = last_writer.get(&access.ty) {
                        self.adjacency[*writer].push(i);
                        self.in_degrees[i] += 1;
                    }

                    readers.entry(access.ty).or_default().push(i);
                }
            }
        }

        // let mut resource_access: HashMap<AccessType, Vec<usize>> = HashMap::new();
        // for (i, node) in self.nodes.iter().enumerate() {
        //     for access in &node.access {
        //         resource_access.entry(access.ty).or_default().push(i);
        //     }
        // }

        // for nodes in resource_access.values() {
        //     for i in 0..nodes.len() {
        //         for j in i..nodes.len() {
        //             println!("{i} {j}");

        //             let node1 = nodes[i];
        //             let node2 = nodes[j];

        //             if self.has_conflict(&self.nodes[node1], &self.nodes[node2]) {
        //                 self.add_edge(node1, node2);
        //             }
        //         }
        //     }
        // }
    }

    fn has_conflict(&self, a: &GraphNode, b: &GraphNode) -> bool {
        for i in &a.access {
            for j in &b.access {
                if i.has_conflict(j) {
                    return true;
                }
            }
        }

        false
    }

    /// Produces a dotviz file.
    pub(crate) fn render(&self, systems: &FxHashMap<SystemId, Box<dyn System>>) -> String {
        let mut out = String::from("digraph {");

        for (i, nodes) in self.adjacency.iter().enumerate() {
            let i_id = self.nodes[i].sid;
            let system = systems.get(&i_id).unwrap();
            let i_name = system.name();

            for j in nodes {
                let system = systems.get(&self.nodes[*j].sid).unwrap();
                let j_name = system.name();

                let _ = write!(out, "{i_name} -> {j_name};");
            }
        }

        out.push('}');
        out
    }

    pub fn sort(&mut self, systems: FxHashMap<SystemId, Box<dyn System>>) -> Schedule {
        tracing::debug!("Generating dependency graph...");

        // Create edges between all conflicting systems
        self.build_dependencies();

        tracing::debug!("Rendered: {}", self.render(&systems));

        let mut sets = Vec::new();
        let mut current_set = self
            .in_degrees
            .iter()
            .enumerate()
            .filter_map(|(i, deg)| 0.eq(deg).then_some(i))
            .collect::<Vec<_>>();

        tracing::debug!("Finding optimal schedule...");
        while !current_set.is_empty() {
            let mut next_set = Vec::new();

            for &node_idx in &current_set {
                for &neighbor in &self.adjacency[node_idx] {
                    self.in_degrees[neighbor] -= 1;
                    if self.in_degrees[neighbor] == 0 {
                        next_set.push(neighbor);
                    }
                }
            }

            sets.push(std::mem::replace(&mut current_set, next_set));
        }

        // Map node index to system ID
        let sets = sets
            .iter()
            .map(|set| {
                set.iter()
                    .map(|&node| self.nodes[node].sid)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let named = sets
            .iter()
            .map(|ids| {
                ids.iter()
                    .map(|id| systems.get(id).unwrap().name())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        tracing::info!("Optimal schedule: {named:?}");

        Schedule { systems, sets }
    }
}

pub struct Schedule {
    pub(crate) systems: FxHashMap<SystemId, Box<dyn System>>,
    pub(crate) sets: Vec<Vec<SystemId>>,
}
