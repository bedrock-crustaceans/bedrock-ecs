// #[cfg(test)]
// mod test;

#![warn(clippy::pedantic)]

pub mod archetype;
pub mod component;
pub mod entity;
pub mod filter;
pub mod graph;
pub mod local;
pub mod param;
pub mod query;
pub mod resource;
pub mod schedule;
pub mod signature;
pub mod sparse;
pub mod spawn;
pub mod system;
pub mod table;
pub mod table_iterator;
pub mod util;
pub mod world;

pub mod prelude {
    pub use crate::archetype::Archetypes;
    pub use crate::component::{Component, ComponentBundle};
    pub use crate::entity::{Entity, EntityId};
    pub use crate::filter::{Added, Changed, Filter, FilterBundle, Removed, With, Without};
    pub use crate::local::Local;
    pub use crate::param::{Param, ParamBundle};
    pub use crate::query::{Query, QueryBundle};
    pub use crate::resource::{Res, ResMut, Resource, ResourceId};
    pub use crate::schedule::{ScheduleBuilder, ScheduleLabel};
    pub use crate::world::World;
}

pub(crate) mod sealed {
    pub trait Sealed {}
    pub enum Sealer {}

    impl Sealed for Sealer {}
}
