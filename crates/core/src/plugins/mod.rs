mod query;
mod registry;
mod system;

use std::sync::PoisonError;

pub use query::*;
pub use registry::*;
pub use system::*;

pub(super) mod bindings {
    wasmtime::component::bindgen!({
        path: "wit",
        world: "api"
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    AlreadyInitialized,
    Poisoned,
}

impl<T> From<PoisonError<T>> for PluginError {
    fn from(_value: PoisonError<T>) -> Self {
        Self::Poisoned
    }
}

pub type PluginResult<T> = Result<T, PluginError>;
