#[cfg(test)]
mod test;

pub mod archetype;
pub mod component;
pub mod entity;
pub mod filter;
pub mod local;
pub mod param;
pub mod query;
pub mod sparse_set;
pub mod spawn;
pub mod system;
pub mod util;
pub mod world;

pub mod prelude {

}

pub(crate) mod sealed {
    pub trait Sealed {}
    pub enum Sealer {}

    impl Sealed for Sealer {}
}