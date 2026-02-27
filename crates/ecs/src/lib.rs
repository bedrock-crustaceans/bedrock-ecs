#[cfg(test)]
mod test;

mod component;
mod entity;
mod error;
mod event;
mod filter;
mod query;
mod resource;
mod scheduler;
mod state;
mod system;
#[macro_use]
mod util;
mod world;

pub use component::*;
pub use entity::*;
pub use error::*;
pub use event::*;
pub use filter::*;
pub use query::*;
pub use resource::*;
pub use state::*;
pub use system::*;
pub use util::*;
pub use world::*;

pub mod prelude {
    #![allow(unused)]

    use super::component::Component;
    use super::entity::{Entity, EntityId};
    use super::event::{Event, EventId, EventReader, EventWriter};
    use super::filter::{Added, Changed, Removed, With, Without};
    use super::query::Query;
    use super::resource::{Res, ResMut, Resource};
    use super::state::State;
    use super::world::World;
}

pub(crate) mod sealed {
    pub trait Sealed {}
    pub enum Sealer {}

    impl Sealed for Sealer {}
}
