use std::{
    cell::UnsafeCell,
    fmt::{self, Debug, Display},
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::util::{ConstNonNull, SyncUnsafeCell};

#[cfg(debug_assertions)]
use crate::util::debug::BorrowEnforcer;

pub struct Ref<'w, T> {
    pub(crate) inner: &'w T,
}

impl<T: Debug> Debug for Ref<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T: Display> Display for Ref<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> Deref for Ref<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.inner
    }
}

/// Holds a mutable reference.
///
/// If the caller attempts to access the reference, the change tracker will register this change to notify other
/// systems next tick.
pub struct Mut<'w, T> {
    pub(crate) current_tick: u32,
    /// Reference to the tracker for this component.
    pub(crate) tracker: &'w mut u32,
    pub(crate) inner: &'w mut T,
}

impl<T> Mut<'_, T> {
    pub fn bypass_detection(&mut self) -> &mut T {
        self.inner
    }
}

impl<T: Debug> Debug for Mut<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T: Display> Display for Mut<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> Deref for Mut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.inner
    }
}

impl<T> DerefMut for Mut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        *self.tracker = self.current_tick;
        self.inner
    }
}

/// The changes made to a component.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Changes {
    /// The tick during which this component was added.
    pub added_tick: u32,
    /// The tick during which this component was last modified.
    pub changed_tick: u32,
}

/// Keeps track of changes to components.
#[derive(Default)]
pub struct ChangeTracker {
    #[cfg(debug_assertions)]
    enforcer: BorrowEnforcer,

    /// Keeps track of the tick when a component was added.
    ///
    /// Parallel queries will use `split_at_mut` to split this vec into multiple mutable references.
    pub(crate) added: SyncUnsafeCell<Vec<u32>>,
    /// Keeps track of the tick when a component was added.
    ///
    /// Parallel queries will use `split_at_mut` to split this vec into multiple mutable references.
    pub(crate) changed: SyncUnsafeCell<Vec<u32>>,
}

impl ChangeTracker {
    /// Creates a new change tracker.
    pub fn new() -> ChangeTracker {
        Self {
            #[cfg(debug_assertions)]
            enforcer: BorrowEnforcer::new(),

            added: SyncUnsafeCell::new(Vec::new()),
            changed: SyncUnsafeCell::new(Vec::new()),
        }
    }

    pub fn added_base_ptr(&self) -> ConstNonNull<u32> {
        let inner = unsafe { &*self.added.get() };

        assert!(!inner.is_empty());

        // Safety: Vec never returns a null pointer.
        unsafe { ConstNonNull::new_unchecked(inner.as_ptr()) }
    }

    pub fn changed_base_ptr(&self) -> ConstNonNull<u32> {
        let inner = unsafe { &*self.changed.get() };

        assert!(!inner.is_empty());

        // Safety: Vec never returns a null pointer.
        unsafe { ConstNonNull::new_unchecked(inner.as_ptr()) }
    }

    pub fn reserve(&mut self, n: usize) {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        self.added.get_mut().reserve(n);
        self.changed.get_mut().reserve(n);
    }

    pub fn resize(&mut self, n: usize, current_tick: u32) {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        self.added.get_mut().resize(n, current_tick);
        self.changed.get_mut().resize(n, current_tick);
    }
}
