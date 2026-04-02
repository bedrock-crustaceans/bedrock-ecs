mod query;
mod registry;
mod system;

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
}

pub type PluginResult<T> = Result<T, PluginError>;
