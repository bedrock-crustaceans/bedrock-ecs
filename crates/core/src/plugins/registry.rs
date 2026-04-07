use std::{
    cell::UnsafeCell,
    collections::HashMap,
    path::Path,
    ptr::NonNull,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU32, Ordering},
    },
};

use nohash_hasher::{BuildNoHashHasher, NoHashHasher};
use wasmtime::{
    Config, Engine, Store,
    component::{Component, HasSelf, Linker},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::{
    plugins::{
        PluginError, PluginResult, WasmSystem,
        bindings::{
            self, Api,
            bedrock_ecs::plugin::{host, system::SystemManifest, types::SystemId as WasmSystemId},
            exports::bedrock_ecs::plugin::plugin::PluginManifest,
        },
    },
    prelude::ScheduleBuilder,
    scheduler::AccessDesc,
    system::SysMeta,
    world::World,
};

use host::PluginError as GuestPluginError;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PluginId(pub(crate) u32);

enum PluginStoreStage {
    Uninitialized,
    Initializing { systems: Vec<WasmSystem> },
    Initialized,
}

struct PluginStore {
    ctx: WasiCtx,
    table: ResourceTable,
    plugin_id: PluginId,

    /// Maps system IDs to names for debugging purposes.
    system_map: HashMap<WasmSystemId, String, BuildNoHashHasher<u32>>,
    stage: PluginStoreStage,

    /// Cyclical reference. This is used to create new strong references to the plugin
    /// to give to systems. This ensures that while systems are active, the plugin exists.
    plugin: Option<Weak<Mutex<Plugin>>>,
}

impl host::Host for PluginStore {
    fn register_system(&mut self, manifest: SystemManifest) -> Result<u32, GuestPluginError> {
        match &mut self.stage {
            PluginStoreStage::Initializing { systems } => {
                let id = systems.len() as u32;

                self.system_map.insert(id, manifest.name.clone());
                systems.push(WasmSystem {
                    plugin: self
                        .plugin
                        .as_ref()
                        .expect("weak plugin reference was not set")
                        .upgrade()
                        .expect("no strong plugin references existed"),
                    meta: SysMeta {
                        name: manifest.name,
                        last_ran: 0,
                    },
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
            _ => {
                tracing::error!("systems can only be registered during plugin `init`");
                Err(GuestPluginError::OutsideInit)
            }
        }
    }

    fn get_version(&mut self) -> String {
        String::from("0.1.0")
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
    store: Store<PluginStore>,
    instance: Api,
}

impl Plugin {
    pub fn init(&mut self) -> wasmtime::Result<()> {
        self.instance
            .bedrock_ecs_plugin_plugin()
            .call_init(&mut self.store)?;

        tracing::trace!(
            "plugin {}@{} initialized",
            self.manifest.name,
            self.manifest.version
        );

        Ok(())
    }

    pub fn get_manifest(&mut self) -> wasmtime::Result<PluginManifest> {
        self.instance
            .bedrock_ecs_plugin_plugin()
            .call_get_manifest(&mut self.store)
    }

    pub fn deinit(&mut self) -> wasmtime::Result<()> {
        self.instance
            .bedrock_ecs_plugin_plugin()
            .call_deinit(&mut self.store)?;

        tracing::trace!(
            "plugin {}@{} deinitialized",
            self.manifest.name,
            self.manifest.version
        );

        Ok(())
    }

    pub fn call(&mut self, id: u32) -> wasmtime::Result<()> {
        tracing::trace!("calling system {id} in {}", self.manifest.name);

        self.instance
            .bedrock_ecs_plugin_plugin()
            .call_call(&mut self.store, id)
    }
}

pub struct PluginRegistry {
    plugins: Vec<Arc<Mutex<Plugin>>>,
    engine: Engine,
}

impl PluginRegistry {
    pub fn new() -> wasmtime::Result<Self> {
        let mut config = Config::new();
        config
            .strategy(wasmtime::Strategy::Cranelift)
            .wasm_component_model(true);

        Ok(Self {
            plugins: Vec::new(),
            engine: Engine::new(&config)?,
        })
    }

    pub fn add<P: AsRef<Path>>(&mut self, module_path: P) -> wasmtime::Result<PluginId> {
        let id = self.plugins.len() as u32;

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
                plugin_id: PluginId(id),
                stage: PluginStoreStage::Uninitialized,
                plugin: None,
                system_map: HashMap::default(),
            },
        );

        let instance = bindings::Api::instantiate(&mut store, &component, &linker)?;

        let mut plugin = Plugin {
            // set empty manifest until `get_manifest is called`.
            // the runtime needs a reference to the plugin itself before being able to call any of its functions.
            manifest: PluginManifest {
                name: String::new(),
                version: String::new(),
            },
            instance,
            store,
        };

        let arc = Arc::new(Mutex::new(plugin));
        // And set the weak reference inside the plugin
        {
            let mut lock = arc.lock().expect("plugin lock was poisoned");
            lock.store.data_mut().plugin = Some(Arc::downgrade(&arc));

            // Obtain the plugin manifest
            lock.manifest = lock.get_manifest()?;

            // set stage to initializing, this is to ensure only `ìnit` can access the initialization methods.
            lock.store.data_mut().stage = PluginStoreStage::Initializing {
                systems: Vec::new(),
            };

            // and initialize the plugin
            lock.init()?;
        }

        self.plugins.push(arc);

        Ok(PluginId(id))
    }

    /// Registers the plugin's systems to the scheduler and sets the state to initialized.
    pub fn resolve_systems(&mut self, builder: &mut ScheduleBuilder) -> PluginResult<()> {
        for plugin in &mut self.plugins {
            let mut lock = plugin.lock()?;
            let store = lock.store.data_mut();

            match &mut store.stage {
                PluginStoreStage::Initializing { systems } => {
                    tracing::trace!("resolving {} systems", systems.len());
                    for system in systems.drain(..) {
                        Self::resolve_system(builder, system);
                    }

                    // Then set plugin state to initialized
                    store.stage = PluginStoreStage::Initialized;
                }
                _ => return Err(PluginError::AlreadyInitialized),
            }
        }

        Ok(())
    }

    fn resolve_system(builder: &mut ScheduleBuilder, system: WasmSystem) {
        let contained = Box::new(system);
        builder.add_contained(contained)
    }
}
