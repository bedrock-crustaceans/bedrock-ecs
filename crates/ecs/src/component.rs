use std::{any::{Any, TypeId}, collections::HashMap, marker::PhantomData};

use crate::entity::EntityId;

pub trait Component: 'static {}

pub(crate) struct Storage<T: Component> {
    pub(crate) map: HashMap<EntityId, usize>,
    pub(crate) storage: Vec<T>,
    _marker: PhantomData<T>
}

impl<T: Component> Storage<T> {
    pub fn with(entity: EntityId, component: T) -> Storage<T> {
        todo!()
    }

    pub fn insert(&mut self, entity: EntityId, component: T) {
        todo!()
    }

    pub fn has_entity(&self, entity: EntityId) -> bool {
        todo!()
    }
}

pub(crate) trait ErasedStorage {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn has_entity(&self, entity: EntityId) -> bool;
}

impl<T: Component> ErasedStorage for Storage<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }    

    fn has_entity(&self, entity: EntityId) -> bool {
        self.has_entity(entity)
    }
}

#[derive(Default)]
pub(crate) struct Components {
    pub(crate) map: HashMap<TypeId, Box<dyn ErasedStorage>>
}

impl Components {
    pub fn new() -> Components {
        Components::default()
    }

    pub fn insert<T: Component>(&mut self, entity: EntityId, component: T) -> Option<T> {
        let type_id = TypeId::of::<T>();
        let entry = self.map.entry(type_id)
            .and_modify(|s| {
                let downcast: &mut Storage<T> = s
                    .as_any_mut()
                    .downcast_mut()
                    .expect("Wrong component storage type found in map");

                downcast.insert(entity, component);
            })
            .or_insert_with(|| {
                todo!();
                // Box::new(Storage::with(entity, component))
            });

        todo!();
    }

    pub fn has_component<T: Component>(&self, entity: EntityId) -> bool {
        let type_id = TypeId::of::<T>();
        self.map
            .get(&type_id)
            .map(|v| v.has_entity(entity))
            .unwrap_or(false)
    }
}