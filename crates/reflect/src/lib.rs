use std::{any::TypeId, collections::HashMap};

use nohash_hasher::{BuildNoHashHasher, NoHashHasher};

macro_rules! assert_dyn_compatible {
    ($t:ident) => {
        const _: Option<&dyn $t> = None;
    };
}

#[derive(Default)]
pub struct ReflectRegistry {
    types: HashMap<TypeId, Box<dyn Reflect>, BuildNoHashHasher<u64>>,
}

impl ReflectRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T: Reflect + 'static>(&mut self) {
        let ty_id = TypeId::of::<T>();
        let reflect = Box::new()

        self.types.insert(ty_id, );
    }
}

pub trait Reflect {
    fn name(&self) -> &'static str;
    fn methods(&self, reg: &ReflectRegistry) -> &dyn FuncTable;
}

assert_dyn_compatible!(Reflect);

pub trait ReflectFunc {
    fn call(&self, args: &[&dyn Reflect]) -> dyn Reflect;
}

assert_dyn_compatible!(ReflectFunc);

pub trait FuncTable {
    fn get(&self, name: &str) -> Option<&dyn ReflectFunc>;
}

assert_dyn_compatible!(FuncTable);
