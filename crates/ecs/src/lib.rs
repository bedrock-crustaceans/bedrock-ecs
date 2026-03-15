// #[cfg(test)]
// mod test;

#![warn(clippy::pedantic)]

pub(crate) mod archetype;
pub(crate) mod signature;
pub(crate) mod component;
pub(crate) mod entity;
pub(crate) mod filter;
pub(crate) mod graph;
pub(crate) mod local;
pub(crate) mod param;
pub(crate) mod query;
pub(crate) mod schedule;
pub(crate) mod sparse;
pub(crate) mod spawn;
pub(crate) mod system;
pub(crate) mod table_iterator;
pub(crate) mod table;
pub(crate) mod util;
pub(crate) mod world;

pub use crate::archetype::Archetypes;
pub use crate::signature::Signature;
pub use crate::component::{Component, ComponentBundle};
pub use crate::entity::{Entity, EntityId};
pub use crate::filter::{Added, Changed, Removed, With, Without, Filter, FilterBundle};
pub use crate::local::Local;
pub use crate::param::{Param, ParamBundle};
pub use crate::query::{Query, QueryMeta, QueryBundle, TableCache};
pub use crate::schedule::{ScheduleBuilder, ScheduleLabel};
pub use crate::world::World;

pub(crate) mod sealed {
    pub trait Sealed {}
    pub enum Sealer {}

    impl Sealed for Sealer {}
}