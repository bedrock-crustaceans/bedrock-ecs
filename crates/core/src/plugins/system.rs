use crate::{plugins::bindings::bedrock_ecs::plugin::system::SystemManifest, system::SystemId};

#[derive(Debug)]
pub struct WasmSystem {
    id: u32,
    manifest: SystemManifest,
}
