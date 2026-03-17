use std::cell::UnsafeCell;

use rustc_hash::FxHashMap;

use crate::resource::{Resource, ResourceBundle, ResourceId};

#[derive(Default)]
pub struct ResourceRegistry {
    pub(crate) storage: FxHashMap<ResourceId, UnsafeCell<Box<dyn Resource>>>,
}

impl ResourceRegistry {
    /// Creates a new container for resources.
    #[inline]
    pub fn new() -> ResourceRegistry {
        ResourceRegistry::default()
    }

    /// Inserts a resource into the container.
    pub fn insert<R: Resource>(&mut self, resource: R) {
        let id = ResourceId::of::<R>();
        self.storage.insert(id, UnsafeCell::new(Box::new(resource)));
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.storage.reserve(additional);
    }

    /// Does the container have these resources?
    #[inline]
    pub fn contains<R: ResourceBundle>(&self) -> bool {
        R::contains_all(self)
    }

    pub fn get<R: Resource>(&self) -> Option<&R> {
        let id = ResourceId::of::<R>();
        let cell = self.storage.get(&id)?;

        let res = unsafe { &*cell.get().cast_const() };

        res.as_any().downcast_ref::<R>()
    }

    pub fn get_mut<R: Resource>(&mut self) -> Option<&mut R> {
        let id = ResourceId::of::<R>();
        self.storage
            .get_mut(&id)?
            .get_mut() // Take resource out of unsafe cell.
            .as_any_mut()
            .downcast_mut::<R>()
    }

    // Safety: This should only be called if you will have guaranteed unique access to this resource.
    pub(crate) unsafe fn get_mut_unchecked<R: Resource>(&self) -> Option<&mut R> {
        let id = ResourceId::of::<R>();
        let cell = self.storage.get(&id)?;
        let boxed = unsafe { &mut *cell.get() };

        boxed.as_any_mut().downcast_mut::<R>()
    }

    pub fn remove<R: Resource>(&mut self) -> Option<Box<R>> {
        let id = ResourceId::of::<R>();
        let cell = self.storage.remove(&id)?;

        cell.into_inner().into_any().downcast::<R>().ok()
    }
}
