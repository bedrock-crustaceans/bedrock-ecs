// #[cfg(test)]
// mod test;

#![warn(clippy::pedantic)]

pub mod archetype;
pub mod command;
pub mod component;
pub mod entity;
pub mod local;
pub mod query;
pub mod resource;
pub mod scheduler;
pub mod system;
pub mod table;
pub mod util;
pub mod world;

pub mod prelude {
    pub use crate::archetype::Archetypes;
    pub use crate::component::{Component, ComponentBundle};
    pub use crate::entity::{EntityHandle, EntityRef};
    pub use crate::local::Local;
    pub use crate::query::{Added, Changed, Filter, FilterBundle, Removed, With, Without};
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
