use std::any::TypeId;

use rustc_hash::FxHashMap;

use crate::component::{Component, ComponentId};

/// Maintains a consistent mapping from component type IDs to unique integers.
///
/// This is used to reduce the size of component IDs from random 128-bit type id hashes to
/// smaller consecutive 64-bit IDs.
#[derive(Debug, Default)]
pub struct TypeRegistry {
    /// The map from type IDs to component IDs.
    mapping: FxHashMap<TypeId, usize>,
    /// The next ID to be assigned to a component.
    next_id: usize,
}

impl TypeRegistry {
    /// Creates a new registry.
    pub fn new() -> TypeRegistry {
        TypeRegistry {
            mapping: FxHashMap::default(),
            next_id: 0,
        }
    }

    /// Returns the component's ID or `None` if it has not been registered.
    pub fn get<T: 'static>(&self) -> Option<ComponentId> {
        let ty_id = TypeId::of::<T>();
        self.mapping.get(&ty_id).copied().map(ComponentId::from)
    }

    /// Returns the component's ID if it exists, or assigns and returns a new one if it does not.
    pub fn get_or_assign<T: 'static>(&mut self) -> ComponentId {
        let ty_id = TypeId::of::<T>();

        let id = self.mapping.get(&ty_id).copied().unwrap_or_else(|| {
            self.mapping.insert(ty_id, self.next_id);
            self.next_id += 1;
            self.next_id - 1
        });

        ComponentId(id)
    }
}
