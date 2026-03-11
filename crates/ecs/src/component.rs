use std::{any::TypeId, collections::HashMap, ops::Deref};

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct ComponentId(pub(crate) usize);

impl Deref for ComponentId {
    type Target = usize;

    fn deref(&self) -> &usize {
        &self.0
    }
}

impl From<usize> for ComponentId {
    fn from(v: usize) -> Self {
        Self(v)
    }
}

pub trait Component: 'static {}

#[derive(Debug, Default)]
pub struct ComponentRegistry {
    type_to_id: HashMap<TypeId, usize>,
    next_id: usize
}

impl ComponentRegistry {
    pub fn new() -> ComponentRegistry {
        ComponentRegistry {
            type_to_id: HashMap::new(),
            next_id: 0
        }
    }

    pub fn get_or_assign<T: Component>(&mut self) -> ComponentId {
        let ty_id = TypeId::of::<T>();

        let id = self.type_to_id
            .get(&ty_id)
            .copied()
            .unwrap_or_else(|| {
                self.type_to_id.insert(ty_id, self.next_id);
                self.next_id += 1;
                self.next_id - 1
            });

        ComponentId(id)
    }
}