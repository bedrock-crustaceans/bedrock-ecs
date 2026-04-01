mod system;

use std::path::Path;

pub use system::*;
use wasmtime::{
    Engine, Store,
    component::{Component, HasSelf, Linker},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::plugins::bindings::{
    bedrock_ecs::plugin::server, exports::bedrock_ecs::plugin::metadata::Manifest,
};

struct WasmPluginState {
    pub table: ResourceTable,
    pub ctx: WasiCtx,

    pub server_version: Vec<u32>,
}

impl server::Host for WasmPluginState {
    fn get_version(&mut self) -> Vec<u32> {
        self.server_version.clone()
    }
}

impl WasiView for WasmPluginState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

mod bindings {
    wasmtime::component::bindgen!({
        path: "wit",
        world: "plugin"
    });
}

pub struct WasmPlugin {
    store: Store<WasmPluginState>,
    manifest: Manifest,
    component: Component,
}

impl WasmPlugin {
    pub fn new<P: AsRef<Path>>(file: P, engine: &Engine) -> Result<Self, wasmtime::Error> {
        let component = Component::from_file(engine, file)?;

        let table = ResourceTable::new();
        let ctx = WasiCtxBuilder::new()
            .inherit_stdout()
            .inherit_stderr()
            .build();

        let state = WasmPluginState {
            ctx,
            table,
            server_version: vec![0, 42, 0],
        };

        let mut store = Store::new(&engine, state);
        let mut linker = Linker::new(engine);

        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;

        type Data = HasSelf<WasmPluginState>;
        server::add_to_linker::<_, Data>(&mut linker, |state: &mut WasmPluginState| state)?;

        let instance = bindings::Plugin::instantiate(&mut store, &component, &linker)?;

        // Attempt to load the manifest
        let manifest = instance
            .bedrock_ecs_plugin_metadata()
            .call_get_manifest(&mut store)?;

        Ok(Self {
            manifest,
            store,
            component,
        })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
}
