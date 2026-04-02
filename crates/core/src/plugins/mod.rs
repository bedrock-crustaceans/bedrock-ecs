mod query;
mod system;

pub use query::*;
pub use system::*;

use std::{path::Path, ptr::NonNull};

use wasmtime::{
    Engine, Store,
    component::{Component, HasSelf, Linker},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

pub(super) mod bindings {
    wasmtime::component::bindgen!({
        path: "wit",
        world: "api"
    });
}

struct WasmPluginStore {
    pub ctx: WasiCtx,
    pub table: ResourceTable,
    pub world: NonNull<World>,
    pub scheduler: NonNull<Scheduler>,
}

unsafe impl Send for WasmPluginStore {}

impl WasiView for WasmPluginStore {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

impl host::Host for WasmPluginStore {
    fn get_version(&mut self) -> String {
        todo!()
    }

    fn get_component_id(&mut self, name: String) -> Option<u32> {
        todo!()
    }

    fn get_resource_id(&mut self, name: String) -> Option<u32> {
        todo!()
    }

    /// Registers a system and returns a unique identifier for this system.
    ///
    /// The host will use this identifier to refer to the system from now on.
    fn register_system(&mut self, system: host::SystemManifest) -> u32 {
        todo!()
    }

    fn deregister_system(&mut self, id: u32) {
        todo!()
    }
}

use bindings::bedrock_ecs::plugin::host;

use crate::{
    plugins::bindings::exports::bedrock_ecs::plugin::metadata, scheduler::Scheduler, world::World,
};

pub struct WasmPlugin {
    world: NonNull<World>,
    manifest: metadata::PluginManifest,
    store: Store<WasmPluginStore>,
    instance: bindings::Api,
}

impl WasmPlugin {
    pub fn new<P: AsRef<Path>>(
        path: P,
        engine: &Engine,
        world: &mut World,
    ) -> wasmtime::Result<Self> {
        let component = Component::from_file(&engine, path)?;
        let mut linker = Linker::new(&engine);

        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
        host::add_to_linker::<_, HasSelf<WasmPluginStore>>(&mut linker, |state| state)?;

        let mut store = Store::new(
            &engine,
            WasmPluginStore {
                ctx: WasiCtxBuilder::new()
                    .inherit_stdout()
                    .inherit_stderr()
                    .build(),
                table: ResourceTable::new(),
                world: NonNull::from_mut(world),
            },
        );

        let instance = bindings::Api::instantiate(&mut store, &component, &linker)?;

        // Obtain the plugin manifest.
        let plugin_manifest = instance
            .bedrock_ecs_plugin_metadata()
            .call_get_manifest(&mut store)?;

        // Initialise the plugin
        instance
            .bedrock_ecs_plugin_metadata()
            .call_init(&mut store)?;

        Ok(Self {
            manifest: plugin_manifest,
            store,
            instance,
            world: NonNull::from_mut(world),
        })
    }

    pub fn destroy(mut self) -> wasmtime::Result<()> {
        self.instance
            .bedrock_ecs_plugin_metadata()
            .call_deinit(&mut self.store)
    }
}
