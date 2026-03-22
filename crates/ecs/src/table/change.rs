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
    pub(crate) current_tick: u32,
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
        tracing::error!("CHANGED {}", std::any::type_name::<T>());
        unsafe { self.tracker.changed(self.index, self.current_tick) };
        self.inner
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Changes {
    pub added: u32,
    pub changed: u32,
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

    pub unsafe fn changed(&self, index: usize, current_tick: u32) {
        unsafe { *self.changed[index].get() = current_tick };
    }

    pub unsafe fn added(&self, index: usize, current_tick: u32) {
        unsafe { *self.added[index].get() = current_tick };
    }

    pub fn reserve(&mut self, n: usize) {
        self.added.reserve(n);
        self.changed.reserve(n);
    }

    pub fn resize(&mut self, n: usize, current_tick: u32) {
        self.added.resize_with(n, || UnsafeCell::new(current_tick));
        self.changed
            .resize_with(n, || UnsafeCell::new(current_tick));
    }
}

pub struct ChangeTrackerIter<'a> {
    index: usize,
    pub(crate) tracker: Option<&'a ChangeTracker>,
}

impl<'a> ChangeTrackerIter<'a> {
    pub fn empty() -> ChangeTrackerIter<'a> {
        Self {
            index: 0,
            tracker: None,
        }
    }

    pub fn new(tracker: &'a ChangeTracker) -> ChangeTrackerIter<'a> {
        Self {
            index: 0,
            tracker: Some(tracker),
        }
    }
}

impl Iterator for ChangeTrackerIter<'_> {
    type Item = Changes;

    fn next(&mut self) -> Option<Changes> {
        let tracker = self.tracker?;
        let index = self.index;

        self.index += 1;

        let added = unsafe { *tracker.added.get(index)?.get().cast_const() };

        debug_assert_eq!(tracker.changed.len(), tracker.added.len());
        let changed = unsafe { *tracker.changed.get_unchecked(index).get().cast_const() };

        Some(Changes { added, changed })
    }
}
