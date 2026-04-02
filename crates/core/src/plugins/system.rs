use crate::{
    component::ComponentId,
    plugins::{
        PluginId,
        bindings::bedrock_ecs::plugin::system::{
            AccessDescriptor as PluginAccessDesc, AccessType as PluginAccessType, SystemManifest,
        },
    },
    scheduler::{AccessDesc, AccessType},
    system::{System, SystemId, SystemMeta},
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

#[derive(Debug)]
pub struct WasmSystem {
    pub plugin_id: PluginId,
    pub id: u32,
    pub access: Vec<AccessDesc>,
    pub meta: SystemMeta,
}

impl System for WasmSystem {
    fn meta(&self) -> &SystemMeta {
        &self.meta
    }

    #[inline]
    fn access(&self) -> &[AccessDesc] {
        &self.access
    }

    unsafe fn call(&self, world: &World) {
        todo!()
    }
}
