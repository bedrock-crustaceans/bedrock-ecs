use std::any::TypeId;
use std::cell::UnsafeCell;
use std::ptr::NonNull;

use rustc_hash::FxHashMap;

use crate::resource::{Resource, ResourceBundle, ResourceId};

/// Implemented only by `UnsafeCell<R>` where `R: Resource`.
///
/// # Safety
///
/// - `type_id` must return the `TypeId` of `R`, not the whole type itself.
/// - `get` must return a valid and aligned pointer to the resource of type `R`.
pub(crate) unsafe trait UnsafeResourceCell {
    fn type_id(&self) -> TypeId;
    fn get(&self) -> *mut u8;
}

unsafe impl<R: Resource> UnsafeResourceCell for UnsafeCell<R> {
    fn type_id(&self) -> TypeId {
        TypeId::of::<R>()
    }

    #[inline]
    fn get(&self) -> *mut u8 {
        self.get().cast::<u8>()
    }
}

#[derive(Default)]
pub struct Resources {
    pub(crate) storage: FxHashMap<ResourceId, Box<dyn UnsafeResourceCell>>,
}

impl Resources {
    /// Creates a new container for resources.
    #[inline]
    pub fn new() -> Resources {
        Resources::default()
    }

    /// Inserts a resource into the container.
    pub fn insert<R: Resource>(&mut self, resource: R) {
        let id = ResourceId::of::<R>();
        self.storage.insert(id, Box::new(UnsafeCell::new(resource)));
    }

    /// Reserves enough capacity for `n` additional resources.
    #[inline]
    pub fn reserve(&mut self, n: usize) {
        self.storage.reserve(n);
    }

    /// Does the container have these resources?
    #[inline]
    pub fn contains<R: ResourceBundle>(&self) -> bool {
        R::contains_all(self)
    }

    /// Retrieves the given resource, returning `None` if it was not found.
    #[expect(clippy::missing_panics_doc, reason = "interval invariant")]
    pub fn get<R: Resource>(&self) -> Option<&R> {
        let id = ResourceId::of::<R>();
        let cell = self.storage.get(&id)?;

        assert_eq!(
            UnsafeResourceCell::type_id(cell.as_ref()),
            TypeId::of::<R>(),
            "incorrect resource type put into registry, this is a bug"
        );

        // Safety: This is safe because from the check above we know that `UnsafeResource::get` will
        // return a pointer to `R`. As it is an immutable pointer, we do not care about aliasing, which needs to be
        // upheld by the unsafe
        Some(unsafe { &*cell.get().cast::<R>().cast_const() })
    }

    /// Retrieves a mutable reference to the given resource, returning `None` if it was not found.
    #[expect(clippy::missing_panics_doc, reason = "interval invariant")]
    pub fn get_mut<R: Resource>(&mut self) -> Option<&mut R> {
        let id = ResourceId::of::<R>();
        let cell = self.storage.get_mut(&id)?;

        assert_eq!(
            UnsafeResourceCell::type_id(cell.as_ref()),
            TypeId::of::<R>(),
            "incorrect resource type put into registry, this is a bug"
        );

        // Safety:
        Some(unsafe { &mut *cell.get().cast::<R>() })
    }

    /// Returns a pointer to the resource in the registry, returning `None` if it was not found.
    #[expect(clippy::missing_panics_doc, reason = "interval invariant")]
    pub fn get_ptr<R: Resource>(&self) -> Option<NonNull<R>> {
        let id = ResourceId::of::<R>();
        let cell = self.storage.get(&id)?;

        assert_eq!(
            UnsafeResourceCell::type_id(cell.as_ref()),
            TypeId::of::<R>(),
            "incorrect resource type put into registry, this is a bug"
        );

        NonNull::new(cell.get().cast::<R>())
    }

    /// Removes the resource from the registry and returns it, if it exists.
    #[expect(clippy::missing_panics_doc, reason = "interval invariant")]
    pub fn remove<R: Resource>(&mut self) -> Option<R> {
        let id = ResourceId::of::<R>();
        let boxed = self.storage.remove(&id)?;

        assert_eq!(
            UnsafeResourceCell::type_id(boxed.as_ref()),
            TypeId::of::<R>(),
            "incorrect resource type put into registry, this is a bug"
        );

        // Move the resource cell to the stack...
        // Safety: This is safe because the pointer returned by `Box::into_raw` is valid and properly aligned.
        // It is also of type `UnsafeCell<R>` as guaranteed by the assert above. `Box::into_raw` also consumes
        // the box so we avoid double frees.
        let cell = unsafe { std::ptr::read(Box::into_raw(boxed).cast::<UnsafeCell<R>>()) };

        Some(cell.into_inner())
    }
}
