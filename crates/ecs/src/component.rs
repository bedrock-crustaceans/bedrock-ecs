use crate::entity::EntityId;
use crate::{EcsResult, PersistentLock};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::any::{Any, TypeId};
use std::cell::UnsafeCell;

pub trait Component: Send + Sync + 'static {}

pub trait SpawnBundle {
    fn insert_into(self, components: &Components, entity: EntityId) -> EcsResult<()>;
}

impl SpawnBundle for () {
    fn insert_into(self, _components: &Components, _entity: EntityId) -> EcsResult<()> {
        Ok(())
    }
}

impl<'a, C0: Component + 'static> SpawnBundle for C0 {
    fn insert_into(self, components: &Components, entity: EntityId) -> EcsResult<()> {
        components.insert(entity, self)?;
        Ok(())
    }
}

impl<'a, C0, C1> SpawnBundle for (C0, C1)
where
    C0: Component + 'static,
    C1: Component + 'static,
{
    fn insert_into(self, components: &Components, entity: EntityId) -> EcsResult<()> {
        components.insert(entity, self.0)?;
        components.insert(entity, self.1)?;

        Ok(())
    }
}

pub trait TypelessStorage: Send + Sync {
    /// Casts the storage to `Any`.
    fn as_any(&self) -> &dyn Any;
    /// Removes the entity from the storage. Returns `true` if the storage is now empty.
    fn remove(&self, entity: EntityId) -> EcsResult<bool>;
    /// Returns whether the storage contains the given entity.
    fn has_entity(&self, entity: EntityId) -> bool;
}

pub struct TypedStorage<T> {
    pub(crate) map: DashMap<EntityId, usize>,
    pub(crate) reverse_map: RwLock<Vec<EntityId>>,

    pub(crate) lock: PersistentLock,
    pub(crate) storage: UnsafeCell<Vec<T>>,
}

unsafe impl<T: Send + Sync + 'static> Send for TypedStorage<T> {}
unsafe impl<T: Send + Sync + 'static> Sync for TypedStorage<T> {}

impl<T: Send + Sync + 'static> TypedStorage<T> {
    /// Creates a new storage, storing the component and entity.
    pub fn with(entity: EntityId, component: T) -> Box<dyn TypelessStorage> {
        Box::new(Self {
            map: DashMap::from_iter([(entity, 0)]),
            reverse_map: RwLock::new(vec![entity]),

            lock: PersistentLock::new(),
            storage: UnsafeCell::new(vec![component]),
        })
    }

    /// Inserts a component for the given entity, returning the old component if it had one.
    ///
    /// This function returns an error if the component storage is currently locked.
    pub fn insert(&self, entity: EntityId, component: T) -> EcsResult<Option<T>> {
        if let Some(index) = self.map.get(&entity) {
            // Entity already has a component of this type, replace it.

            let replaced = {
                let _guard = self.lock.write()?;
                // Safety: Acquiring a mutable reference to the vec is safe because the
                // `acquire_write` call above ensures exclusive access.
                let storage = unsafe { &mut *self.storage.get() };

                std::mem::replace(&mut storage[*index], component)
            };

            Ok(Some(replaced))
        } else {
            // Entity does not have a component of this type yet.

            let index;
            {
                let _guard = self.lock.write()?;
                // Safety: Acquiring a mutable reference to the vec is safe because the
                // `acquire_write` call above ensures exclusive access.
                let storage = unsafe { &mut *self.storage.get() };

                index = storage.len();
                storage.push(component);
            }

            self.map.insert(entity, index);
            self.reverse_map.write().push(entity);

            Ok(None)
        }
    }
}

impl<T> Default for TypedStorage<T> {
    fn default() -> Self {
        Self {
            map: DashMap::new(),
            reverse_map: RwLock::new(Vec::new()),
            lock: PersistentLock::new(),
            storage: UnsafeCell::new(Vec::new()),
        }
    }
}

impl<T: Send + Sync + 'static> TypelessStorage for TypedStorage<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn remove(&self, entity: EntityId) -> EcsResult<bool> {
        if let Some((_, index)) = self.map.remove(&entity) {
            let is_empty;
            {
                let _guard = self.lock.write()?;
                // Safety: Acquiring a mutable reference to the vec is safe because the
                // `acquire_write` call above ensures exclusive access.
                let storage = unsafe { &mut *self.storage.get() };

                storage.swap_remove(index);
                is_empty = storage.is_empty();
            }

            if is_empty {
                self.reverse_map.write().clear();
                return Ok(false);
            }

            // Modify mapping for affected tail entity.
            let mut reverse_lock = self.reverse_map.write();
            let modified_id = reverse_lock[reverse_lock.len() - 1];
            self.map.insert(modified_id, index);
            reverse_lock.swap_remove(index);

            Ok(true)
        } else {
            let _guard = self.lock.read()?;
            // Safety: Acquiring an immutable reference to the vec is safe because the
            // `acquire_write` call above ensures non-mutable access.
            let storage = unsafe { &*self.storage.get() };

            Ok(storage.is_empty())
        }
    }

    fn has_entity(&self, entity: EntityId) -> bool {
        self.map.contains_key(&entity)
    }
}

#[derive(Default)]
pub struct Components {
    pub(crate) map: DashMap<TypeId, Box<dyn TypelessStorage>>,
}

impl Components {
    pub fn insert<T: Component>(&self, entity: EntityId, component: T) -> EcsResult<Option<T>> {
        let type_id = TypeId::of::<T>();

        if let Some(store) = self.map.get_mut(&type_id) {
            let downcast: &TypedStorage<T> = store.as_any().downcast_ref().unwrap();
            downcast.insert(entity, component)
        } else {
            // Storage does not exist, create and insert it.
            self.map
                .insert(type_id, TypedStorage::with(entity, component));

            Ok(None)
        }
    }

    pub fn has_component<T: Component>(&self, entity: EntityId) -> bool {
        let type_id = TypeId::of::<T>();
        if let Some(store_kv) = self.map.get(&type_id) {
            store_kv.value().has_entity(entity)
        } else {
            false
        }
    }

    pub fn despawn(&self, entity: EntityId) {
        self.map.retain(|_, store| {
            !store
                .remove(entity)
                .expect("Cannot despawn components, storage is locked.")
        });
    }
}
