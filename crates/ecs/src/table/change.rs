use std::{
    cell::UnsafeCell,
    fmt::{self, Debug, Display},
    ops::{Deref, DerefMut},
};

use crate::archetype::PartitionedSignature;

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
    pub(crate) tracker: &'w ChangeTracker,
    pub(crate) inner: &'w mut T,
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
        unsafe { self.tracker.set_changed(self.index) };
        self.inner
    }
}

#[derive(Default)]
pub struct ChangeTracker {
    pub(crate) added: Vec<UnsafeCell<u32>>,
    pub(crate) changed: Vec<UnsafeCell<u32>>,
}

impl ChangeTracker {
    /// Creates a new change tracker.
    pub fn new() -> ChangeTracker {
        Self {
            added: Vec::new(),
            changed: Vec::new(),
        }
    }
}
