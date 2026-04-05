use std::{
    cell::UnsafeCell,
    fmt::{self, Debug, Display},
    ops::{Deref, DerefMut},
};

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
    pub(crate) index: usize,
    pub(crate) current_tick: u32,
    pub(crate) tracker: &'w ChangeTracker,
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
        unsafe { self.tracker.set_changed(self.index, self.current_tick) };
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

    pub(crate) added: Vec<UnsafeCell<u32>>,
    pub(crate) changed: Vec<UnsafeCell<u32>>,
}

impl ChangeTracker {
    /// Creates a new change tracker.
    pub fn new() -> ChangeTracker {
        Self {
            #[cfg(debug_assertions)]
            enforcer: BorrowEnforcer::new(),

            added: Vec::new(),
            changed: Vec::new(),
        }
    }

    /// # Safety
    ///
    /// `index` must be within bounds of this tracker.
    #[inline]
    pub unsafe fn get_unchecked(&self, index: usize) -> Changes {
        let added_tick = unsafe { *self.added.get_unchecked(index).get() };
        let changed_tick = unsafe { *self.changed.get_unchecked(index).get() };

        Changes {
            added_tick,
            changed_tick,
        }
    }

    /// Sets the component at `index` as changed in `current_tick`.
    ///
    /// This causes queries using the [`Changed`] filter to return this specific component.
    ///
    /// # Safety
    ///
    /// There should not be any other references to this component's `changed` state.
    ///
    /// [`Changed`]: crate::query::Changed
    pub unsafe fn set_changed(&self, index: usize, current_tick: u32) {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        unsafe { *self.changed[index].get() = current_tick };
    }

    /// Sets the component at `index` as added in `current_tick`.
    ///
    /// This causes queries using the [`Added`] filter to return this specific component.
    ///
    /// # Safety
    ///
    /// There should not be any other references to this component's `added` state.
    ///
    /// [`Added`]: crate::query::Added
    pub unsafe fn set_added(&self, index: usize, current_tick: u32) {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        unsafe { *self.added[index].get() = current_tick };
    }

    pub fn reserve(&mut self, n: usize) {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        self.added.reserve(n);
        self.changed.reserve(n);
    }

    pub fn resize(&mut self, n: usize, current_tick: u32) {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        self.added.resize_with(n, || UnsafeCell::new(current_tick));
        self.changed
            .resize_with(n, || UnsafeCell::new(current_tick));
    }
}
