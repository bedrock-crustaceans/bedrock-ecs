use std::collections::HashSet;
use std::fmt::Write;
use std::hash::{Hash, Hasher};

use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
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

#[derive(Default, Debug)]
pub struct ScheduleGraph {
    pub(crate) edges: Vec<Vec<usize>>,
}

impl ScheduleGraph {
    pub fn new() -> Self {
        Self::default()
    }
}
