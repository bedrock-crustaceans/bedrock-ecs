use std::{
    collections::HashMap,
    path::Path,
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

use nohash_hasher::BuildNoHashHasher;
use wasmtime::{
    Config, Engine, Store,
    component::{Component, HasSelf, Linker},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::{
    plugins::{
        WasmSystem,
        bindings::{
            self,
            bedrock_ecs::plugin::{host, system::SystemManifest},
            exports::bedrock_ecs::plugin::metadata::PluginManifest,
        },
    },
    prelude::ScheduleBuilder,
    scheduler::AccessDesc,
    world::World,
};

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PluginId(pub(crate) u32);

enum PluginStoreData {
    Initializing { systems: Vec<WasmSystem> },
}

struct PluginStore {
    ctx: WasiCtx,
    table: ResourceTable,
    plugin_id: PluginId,
    data: PluginStoreData,
}

impl host::Host for PluginStore {
    fn register_system(&mut self, manifest: SystemManifest) -> Result<u32, ()> {
        match &mut self.data {
            PluginStoreData::Initializing { systems } => {
                let id = systems.len() as u32;

                systems.push(WasmSystem {
                    plugin_id: self.plugin_id,
                    name: manifest.name,
                    access: manifest
                        .access
                        .iter()
                        .copied()
                        .map(AccessDesc::from)
                        .collect::<Vec<_>>(),
                    id,
                });

                Ok(id)
            }
            _ => Err(()),
        }
    }

    fn get_version(&mut self) -> String {
        todo!("get server version");
    }

    fn get_component_id(&mut self, name: String) -> Option<host::ComponentId> {
        todo!("requires reflection")
    }

    fn get_resource_id(&mut self, name: String) -> Option<host::ResourceId> {
        todo!("requires reflection")
    }
}

impl WasiView for PluginStore {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

pub struct Plugin {
    manifest: PluginManifest,
}

pub struct PluginRegistry {
    plugins: Vec<Plugin>,
    engine: Engine,
}

impl PluginRegistry {
    pub fn new() -> wasmtime::Result<Self> {
        let config = Config::default();
        Ok(Self {
            plugins: Vec::new(),
            engine: Engine::new(&config)?,
        })
    }

    pub fn add<P: AsRef<Path>>(&mut self, module_path: P) -> wasmtime::Result<PluginId> {
        let id = self.plugins.len() as u32 - 1;

        let component = Component::from_file(&self.engine, module_path)?;
        let mut linker = Linker::new(&self.engine);

        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
        host::add_to_linker::<_, HasSelf<PluginStore>>(&mut linker, |state| state)?;

        let mut store = Store::new(
            &self.engine,
            PluginStore {
                ctx: WasiCtxBuilder::new()
                    .inherit_stdout()
                    .inherit_stderr()
                    .build(),
                table: ResourceTable::new(),
            },
        );

        let instance = bindings::Api::instantiate(&mut store, &component, &linker)?;

        // Obtain the plugin manifest
        let manifest = instance
            .bedrock_ecs_plugin_metadata()
            .call_get_manifest(&mut store)?;

        let plugin = Plugin { manifest };

        self.plugins.push(plugin);

        Ok(PluginId(id))
    }
}
