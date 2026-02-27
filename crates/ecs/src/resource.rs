use std::{
    any::{Any, TypeId},
    cell::UnsafeCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use dashmap::DashMap;

use crate::{
    scheduler::SystemParamDescriptor, sealed, EcsError, EcsResult, PersistentLock, SystemParam,
    World,
};

struct ResourceSingleton<R: Resource> {
    lock: PersistentLock,
    resource: UnsafeCell<R>,
}

unsafe impl<R: Resource> Send for ResourceSingleton<R> {}
unsafe impl<R: Resource> Sync for ResourceSingleton<R> {}

impl<R: Resource> ResourceSingleton<R> {
    // pub fn acquire_read(&self) -> EcsResult<()> {
    //     self.lock_marker.read()
    // }

    // pub fn release_read(&self) {
    //     self.lock_marker.force_release_read();
    // }

    /// # Safety:
    ///
    /// Aliasing invariants must be upheld manually.
    /// Ensure you've acauired the appropriate lock with [`acquire_read`](Self::acquire_read) or [`acquire_write`](Self::acquire_write) before calling this.
    pub unsafe fn get(&self) -> *mut R {
        self.resource.get()
    }

    // pub fn acquire_write(&self) -> EcsResult<()> {
    //     self.lock_marker.write()
    // }

    // pub fn release_write(&self) {
    //     self.lock_marker.force_release_write();
    // }
}

trait ResourceHolder: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<R: Resource> ResourceHolder for ResourceSingleton<R> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Default)]
pub struct Resources {
    map: DashMap<TypeId, Box<dyn ResourceHolder>>,
}

impl Resources {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<R: Resource>(&self, resource: R) {
        self.map.insert(
            TypeId::of::<R>(),
            Box::new(ResourceSingleton {
                resource: UnsafeCell::new(resource),
                lock: PersistentLock::new(),
            }),
        );
    }

    /// # Safety:
    ///
    /// Aliasing invariants must be upheld manually.
    /// Ensure you've acauired a lock with [`acquire_read`](Self::acquire_read) before calling this.
    pub unsafe fn get<R: Resource>(&self) -> EcsResult<&R> {
        let singleton = self.map.get(&TypeId::of::<R>()).ok_or(EcsError::NotFound)?;
        let singleton: &ResourceSingleton<R> = singleton
            .as_any()
            .downcast_ref()
            .expect("Incorrect resource singleton inserted into map");

        Ok(unsafe { &*singleton.get() })
    }

    pub fn read<R: Resource>(&self) -> EcsResult<()> {
        let singleton = self.map.get(&TypeId::of::<R>()).ok_or(EcsError::NotFound)?;
        let singleton: &ResourceSingleton<R> = singleton
            .as_any()
            .downcast_ref()
            .expect("Incorrect resource singleton inserted into map");

        let guard = singleton.lock.read()?;
        std::mem::forget(guard);

        Ok(())
    }

    /// Unlocks the read lock on this resource.
    /// # Safety
    ///
    /// This function should only be called if previous call to [`read`](Self::read) was successful.
    pub unsafe fn force_release_read<R: Resource>(&self) {
        let Some(singleton) = self.map.get(&TypeId::of::<R>()) else {
            return;
        };
        let singleton: &ResourceSingleton<R> = singleton
            .as_any()
            .downcast_ref()
            .expect("Incorrect resource singleton inserted into map");

        unsafe { singleton.lock.force_release_read() }
    }

    /// # Safety:
    ///
    /// Aliasing invariants must be upheld manually.
    /// Ensure you've acquired a lock with [`acquire_write`](Self::acquire_write) before calling this.
    pub unsafe fn get_mut<R: Resource>(&self) -> EcsResult<&mut R> {
        let mut singleton = self
            .map
            .get_mut(&TypeId::of::<R>())
            .ok_or(EcsError::NotFound)?;

        let singleton: &mut ResourceSingleton<R> = singleton
            .as_any_mut()
            .downcast_mut()
            .expect("Incorrect resource singleton inserted into map");

        Ok(unsafe { &mut *singleton.get() })
    }

    /// Obtains a write lock on this resource.
    pub fn write<R: Resource>(&self) -> EcsResult<()> {
        let singleton = self.map.get(&TypeId::of::<R>()).ok_or(EcsError::NotFound)?;
        let singleton: &ResourceSingleton<R> = singleton
            .as_any()
            .downcast_ref()
            .expect("Incorrect resource singleton inserted into map");

        let guard = singleton.lock.write()?;
        std::mem::forget(guard);

        Ok(())
    }

    /// Unlocks the write lock on this resource.
    /// # Safety
    ///
    /// This function should only be called if previous call to [`write`](Self::write) was successful.
    pub unsafe fn force_release_write<R: Resource>(&self) {
        let Some(singleton) = self.map.get(&TypeId::of::<R>()) else {
            return;
        };
        let singleton: &ResourceSingleton<R> = singleton
            .as_any()
            .downcast_ref()
            .expect("Incorrect resource singleton inserted into map");

        unsafe { singleton.lock.force_release_write() }
    }
}

pub trait Resource: Send + Sync + 'static {}

pub struct Res<R: Resource> {
    pub(crate) locked: AtomicBool,
    pub(crate) world: Arc<World>,
    pub(crate) _marker: PhantomData<R>,
}

impl<R: Resource> SystemParam for Res<R> {
    type State = ();

    fn descriptor() -> SystemParamDescriptor {
        SystemParamDescriptor::Res(TypeId::of::<R>())
    }

    fn fetch<S: sealed::Sealed>(world: &Arc<World>, _state: &Arc<Self::State>) -> Self {
        Res {
            locked: AtomicBool::new(false),
            world: Arc::clone(world),
            _marker: PhantomData,
        }
    }

    fn state(_world: &Arc<World>) -> Arc<Self::State> {
        Arc::new(())
    }
}

impl<R: Resource> Deref for Res<R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        let locked = self.locked.load(Ordering::SeqCst);
        if !locked {
            self.world.resources.read::<R>().unwrap();
            self.locked.store(true, Ordering::SeqCst);
        }

        unsafe { self.world.resources.get::<R>().unwrap() }
    }
}

impl<R: Resource> Drop for Res<R> {
    fn drop(&mut self) {
        if self.locked.load(Ordering::SeqCst) {
            // Safety: This is safe because due to the condition above, this lock has been properly acquired.
            // It is therefore safe to release it.
            unsafe {
                self.world.resources.force_release_read::<R>();
            };
        }
    }
}

pub struct ResMut<R: Resource> {
    pub(crate) locked: AtomicBool,
    pub(crate) world: Arc<World>,
    pub(crate) _marker: PhantomData<R>,
}

impl<R: Resource> SystemParam for ResMut<R> {
    type State = ();

    fn descriptor() -> SystemParamDescriptor {
        SystemParamDescriptor::ResMut(TypeId::of::<R>())
    }

    fn fetch<S: sealed::Sealed>(world: &Arc<World>, _state: &Arc<Self::State>) -> Self {
        ResMut {
            locked: AtomicBool::new(false),
            world: Arc::clone(world),
            _marker: PhantomData,
        }
    }

    fn state(_world: &Arc<World>) -> Arc<Self::State> {
        Arc::new(())
    }
}

impl<R: Resource> Deref for ResMut<R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        let locked = self.locked.load(Ordering::SeqCst);
        if !locked {
            self.world.resources.write::<R>().unwrap();
            self.locked.store(true, Ordering::SeqCst);
        }

        unsafe { self.world.resources.get_mut::<R>().unwrap() }
    }
}

impl<R: Resource> DerefMut for ResMut<R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let locked = self.locked.load(Ordering::SeqCst);
        if !locked {
            self.world.resources.write::<R>().unwrap();
            self.locked.store(true, Ordering::SeqCst);
        }

        unsafe { self.world.resources.get_mut::<R>().unwrap() }
    }
}

impl<R: Resource> Drop for ResMut<R> {
    fn drop(&mut self) {
        if self.locked.load(Ordering::SeqCst) {
            // Safety: This is safe because due to the condition above, this lock has been properly acquired.
            // It is therefore safe to release it.
            unsafe {
                self.world.resources.force_release_write::<R>();
            }
        }
    }
}
