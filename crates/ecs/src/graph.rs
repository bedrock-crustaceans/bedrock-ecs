use std::any::TypeId;
use crate::component::ComponentId;
use crate::system::SystemId;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AccessType {
    Entity,
    World,
    Component(ComponentId),
    Resource(TypeId),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AccessDesc {
    pub(crate) ty: AccessType,
    pub(crate) exclusive: bool
}

impl AccessDesc {
    pub fn has_conflict(&self, other: &AccessDesc) -> bool {
        self.ty == other.ty && (self.exclusive || other.exclusive)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GraphNode {
    pub sid: SystemId,
    pub access: Vec<AccessDesc>,
}

#[derive(Debug, Clone, Default)]
pub struct ScheduleGraph {
    nodes: Vec<GraphNode>,
    adjacency: Vec<Vec<usize>>,
    in_degrees: Vec<usize>
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

    fn add_edge(&mut self, from: usize, to: usize) {
        self.adjacency[from].push(to);
        self.in_degrees[to] += 1;
    }

    fn build_dependencies(&mut self) {
        // Determine edges
        for i in 0..self.nodes.len() {
            for j in (i + 1)..self.nodes.len() {
                if self.has_conflict(&self.nodes[i], &self.nodes[j]) {
                    self.add_edge(i, j);
                }
            }
        }
    }

    fn has_conflict(&self, a: &GraphNode, b: &GraphNode) -> bool {
        for i in &a.access {
            for j in &b.access {
                if i.has_conflict(j) {
                    return true
                }
            }
        }

        false
    }

    pub fn sort(&mut self) -> Schedule {
        // Create edges between all conflicting systems
        self.build_dependencies();

        let mut sets = Vec::new();
        let mut current_set = self.in_degrees
            .iter()
            .enumerate()
            .filter_map(|(i, deg)| 0.eq(deg).then_some(i))
            .collect::<Vec<_>>();

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

        Schedule {
            sets
        }
    }
}

#[derive(Debug, Clone)]
pub struct Schedule {
    pub(crate) sets: Vec<Vec<usize>>
}