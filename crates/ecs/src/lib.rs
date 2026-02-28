#[cfg(test)]
mod test;

pub mod component;
pub mod local;
pub mod param;
pub mod system;
pub mod world;

pub mod prelude {

}

pub(crate) mod sealed {
    pub trait Sealed {}
    pub enum Sealer {}

    impl Sealed for Sealer {}
}