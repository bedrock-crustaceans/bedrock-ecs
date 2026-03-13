// #[cfg(test)]
// mod test;

#![warn(clippy::pedantic)]

pub mod archetype;
pub mod bitset;
pub mod component;
pub mod entity;
pub mod filter;
pub mod graph;
pub mod local;
pub mod param;
pub mod query;
pub mod schedule;
pub mod sparse_set;
pub mod spawn;
pub mod system;
pub mod table_iterator;
pub mod table;
pub mod util;
pub mod world;

pub mod prelude {

}

pub(crate) mod sealed {
    pub trait Sealed {}
    pub enum Sealer {}

    impl Sealed for Sealer {}
}