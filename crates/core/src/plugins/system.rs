use std::sync::{Arc, Mutex};

use crate::{
    component::ComponentId,
    plugins::{
        Plugin, PluginId,
        bindings::bedrock_ecs::plugin::system::{
            AccessDescriptor as PluginAccessDesc, AccessType as PluginAccessType, SystemManifest,
        },
    },
    scheduler::{AccessDesc, AccessType},
    system::{Sys, SysId, SysMeta},
    world::World,
};

impl From<PluginAccessType> for AccessType {
    fn from(ty: PluginAccessType) -> Self {
        match ty {
            PluginAccessType::None => Self::None,
            PluginAccessType::Entity => Self::Entity,
            PluginAccessType::Component(id) => Self::Component(ComponentId(id as usize)),
            PluginAccessType::Resource(id) => {
                todo!("resources should use a registry rather than type IDs")
            }
            PluginAccessType::World => Self::World,
        }
    }
}

impl From<PluginAccessDesc> for AccessDesc {
    fn from(desc: PluginAccessDesc) -> Self {
        Self {
            mutable: desc.mutable,
            ty: desc.ty.into(),
        }
    }
}

pub struct WasmSystem {
    /// The plugin that this system is associated with
    pub plugin: Arc<Mutex<Plugin>>,
    pub id: u32,
    pub access: Vec<AccessDesc>,
    pub meta: SysMeta,
}

impl Sys for WasmSystem {
    fn meta(&self) -> &SysMeta {
        &self.meta
    }

    #[inline]
    fn access(&self) -> &[AccessDesc] {
        &self.access
    }

    unsafe fn call(&self, world: &World) {
        let mut lock = self.plugin.lock().expect("failed to lock plugin");
        if let Err(err) = lock.call(self.id) {
            tracing::error!("Plugin was trapped while calling system {}", self.id);
        }
    }
}
