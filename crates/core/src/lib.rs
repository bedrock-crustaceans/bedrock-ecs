// #[cfg(test)]
// mod test;

#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate)]

pub mod archetype;
pub mod command;
pub mod component;
pub mod entity;
pub mod local;
pub mod message;
pub mod query;
pub mod resource;
pub mod scheduler;
pub mod sparse;
pub mod system;
pub mod table;
pub mod time;
pub mod util;
pub mod world;

#[cfg(feature = "plugins")]
pub mod plugins;

pub mod prelude {
    pub use crate::archetype::Archetypes;
    pub use crate::component::{Component, ComponentBundle};
    pub use crate::entity::{Entity, EntityRef};
    pub use crate::local::Local;
    pub use crate::query::{Added, Changed, Filter, FilterBundle, With, Without};
    pub use crate::query::{Query, QueryBundle};
    pub use crate::resource::{Res, ResMut, Resource, ResourceId};
    pub use crate::scheduler::{ScheduleBuilder, ScheduleLabel};
    pub use crate::system::{Param, ParamBundle};
    pub use crate::world::World;
}

pub(crate) mod sealed {
    pub trait Sealed {}
    pub enum Sealer {}

    impl Sealed for Sealer {}
}
