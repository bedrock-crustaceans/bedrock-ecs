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
    pub fn conflicts(&self, other: &Self) -> bool {
        if self.ty == AccessType::World {
            return true;
        }

        self.ty == other.ty && (self.exclusive || other.exclusive)
    }
}

#[derive(Debug)]
pub struct ScheduleNode {
    pub id: SystemId,
}

#[derive(Default)]
pub struct ScheduleGraph {
    pub(crate) systems: FxHashMap<SystemId, Box<dyn System>>,
    pub(crate) nodes: Vec<ScheduleNode>,
    pub(crate) edges: Vec<(usize, usize)>,
}

impl ScheduleGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render(&self, systems: &FxHashMap<SystemId, Box<dyn System>>) -> String {
        let mut output = String::from("digraph {");

        for node in &self.nodes {
            let name = systems.get(&node.id).unwrap().meta().name();
            output.push_str(&format!("{name};"));
        }

        for (from, to) in &self.edges {
            let from_name = systems.get(&self.nodes[*from].id).unwrap().meta().name();
            let to_name = systems.get(&self.nodes[*to].id).unwrap().meta().name();

            output.push_str(&format!("{from_name} -> {to_name};"));
        }

        output.push('}');
        output
    }
}
