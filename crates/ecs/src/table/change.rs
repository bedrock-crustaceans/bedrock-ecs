use std::ops::{Deref, DerefMut};

use crate::archetype::{PartitionedSignature, Signature};

pub struct Ref<'w, T> {
    pub(crate) inner: &'w T,
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
    added: PartitionedSignature,
    changed: PartitionedSignature,
}

impl ChangeTracker {
    /// Creates a new change tracker.
    pub fn new() -> ChangeTracker {
        Self {
            added: PartitionedSignature::new(),
            changed: PartitionedSignature::new(),
        }
    }

    pub fn resize(&mut self, n: usize) {
        self.added.resize(n);
        self.changed.resize(n);
    }

    /// # Safety
    ///
    /// This is only safe to call if no other threads can write to the same 64-bit block containing `index`
    /// at the same time.
    ///
    /// # Panics
    ///
    /// This method panics if `index` is out of range.
    #[inline]
    pub unsafe fn set_added(&self, index: usize) {
        // Safety: The soundness conditions are guaranteed by the caller.
        unsafe { self.added.set(index) };
    }

    /// # Safety
    ///
    /// This is only safe to call if no other threads can write to the same 64-bit block containing `index`
    /// at the same time.
    ///
    /// # Panics
    ///
    /// This method panics if `index` is out of range.
    #[inline]
    pub unsafe fn set_changed(&self, index: usize) {
        self.changed.words_count();
        // Safety: The soundness conditions are guaranteed by the caller.
        unsafe { self.changed.set(index) };
    }
}
