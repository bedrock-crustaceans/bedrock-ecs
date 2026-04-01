use crate::{
    scheduler::AccessDesc,
    system::{System, SystemMeta},
    world::World,
};

pub struct PluginSystemContainer {
    meta: SystemMeta,
    method: wasmtime::Func,
}

impl System for PluginSystemContainer {
    #[inline]
    fn meta(&self) -> &SystemMeta {
        &self.meta
    }

    fn access(&self) -> &[AccessDesc] {
        todo!()
    }

    unsafe fn call(&self, world: &World) {
        todo!()
    }
}
