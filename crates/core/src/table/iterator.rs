use std::iter::FusedIterator;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr::NonNull;

use crate::entity::{Entity, EntityRef};
use crate::query::{ArrayLike, Filter};
use crate::table::{ChangeTracker, Mut};
use crate::world::World;

#[cfg(debug_assertions)]
use crate::util::debug::{ReadGuard, WriteGuard};

pub struct ColumnArray<'a, T, F: Filter> {
    pub(crate) current_tick: u32,
    pub(crate) tracker: &'a ChangeTracker,
    pub(crate) len: usize,
    pub(crate) base: NonNull<T>,
    /// Ensures that the components and filters live at least as long as the column.
    pub(crate) _marker: PhantomData<(&'a T, F)>,

    #[cfg(debug_assertions)]
    pub(crate) _guard: Option<ReadGuard>,
}

unsafe impl<'a, T, F: Filter> ArrayLike for ColumnArray<'a, T, F> {
    type Item = &'a T;

    #[inline]
    unsafe fn get_unchecked(&self, index: usize) -> &'a T {
        debug_assert!(isize::try_from(index).is_ok());

        unsafe { &*self.base.add(index).as_ptr().cast_const() }
    }

    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, T, F: Filter> ColumnArray<'a, T, F> {
    /// Constructs a ZST directly from a dangling pointer.
    /// This skips having to add to the base pointer.
    ///
    /// # Safety
    ///
    /// `T` must be an inhabited ZST.
    #[inline]
    unsafe fn get_zst() -> &'a T {
        debug_assert_eq!(std::mem::size_of::<T>(), 0); // this is unsound for non-ZSTs.

        // Safety: we cannot construct `T` directly so we dereference a
        // dangling (but aligned) pointer, which is safe for inhabited ZSTs.
        unsafe { &*std::ptr::dangling::<T>() }
    }

    /// Adds `index` to the base pointer and turns the pointer into a reference.
    ///
    /// # Safety
    ///
    /// `index` must be in range.
    #[inline]
    unsafe fn get_nozst(&self, index: usize) -> &'a T {
        let ptr = unsafe { self.base.add(index) };
        unsafe { &*ptr.as_ptr() }
    }

    /// # Safety
    ///
    /// `ìndex` must be in range.
    #[inline]
    pub unsafe fn filter(&self, index: usize) -> bool {
        F::apply_dynamic(
            unsafe { self.tracker.index_unchecked(index) },
            self.current_tick,
        )
    }

    /// Retrieves the component at `index`.
    ///
    /// # Safety:
    ///
    /// `index` must be in range.
    #[inline]
    pub unsafe fn index_unchecked(&self, index: usize) -> &'a T {
        debug_assert!(index < self.len, "column iterator index out of range");

        if const { std::mem::size_of::<T>() == 0 } {
            unsafe { Self::get_zst() }
        } else {
            unsafe { self.get_nozst(index) }
        }
    }
}

pub struct ColumnIterMut<'a, T, F: Filter> {
    pub(crate) tracker: &'a ChangeTracker,
    pub(crate) last_tick: u32,
    pub(crate) current_tick: u32,
    pub(crate) len: usize,
    pub(crate) base: NonNull<T>,
    pub(crate) _marker: PhantomData<(&'a mut T, F)>,

    #[cfg(debug_assertions)]
    pub(crate) _guard: Option<WriteGuard>,
}

impl<'a, T, F: Filter> ColumnIterMut<'a, T, F> {
    /// Constructs a ZST directly from a dangling pointer.
    /// This skips having to add to the base pointer.
    ///
    /// # Safety
    ///
    /// `T` must be an inhabited ZST.
    #[inline]
    unsafe fn get_zst() -> &'a mut T {
        debug_assert_eq!(std::mem::size_of::<T>(), 0); // this is unsound for non-ZSTs.

        // Safety: we cannot construct `T` directly so we dereference a
        // dangling (but aligned) pointer, which is safe for inhabited ZSTs.
        unsafe { &mut *std::ptr::dangling_mut::<T>() }
    }

    /// Adds `index` to the base pointer and turns the pointer into a reference.
    ///
    /// # Safety
    ///
    /// `index` must be in range.
    #[inline]
    unsafe fn get_nozst(&self, index: usize) -> &'a mut T {
        let ptr = unsafe { self.base.add(index) };
        unsafe { &mut *ptr.as_ptr() }
    }

    /// # Safety
    ///
    /// `ìndex` must be in range.
    #[inline]
    pub unsafe fn filter(&self, index: usize) -> bool {
        F::apply_dynamic(
            unsafe { self.tracker.index_unchecked(index) },
            self.current_tick,
        )
    }

    /// Retrieves the component at `index`.
    ///
    /// # Safety:
    ///
    /// `index` must be in range.
    #[inline]
    pub unsafe fn index_unchecked(&self, index: usize) -> &'a mut T {
        debug_assert!(index < self.len, "column iterator index out of range");

        if const { std::mem::size_of::<T>() == 0 } {
            unsafe { Self::get_zst() }
        } else {
            unsafe { self.get_nozst(index) }
        }
    }
}

unsafe impl<'a, T, F: Filter> ArrayLike for ColumnIterMut<'a, T, F> {
    type Item = Mut<'a, T>;

    #[inline]
    unsafe fn get_unchecked(&self, index: usize) -> Mut<'a, T> {
        debug_assert!(isize::try_from(index).is_ok()); // soundness requirement for `std::ptr::add`.
        debug_assert!(
            index < self.len,
            "mutable column iterator index out of range"
        );

        // Safety: Soundness must be upheld by the caller.
        let inner = unsafe { self.index_unchecked(index) };
        Mut {
            index,
            current_tick: self.current_tick,
            tracker: self.tracker,
            inner,
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}

/// Iterates over entities in the current table.
///
// This not in the `entity` module since it iterates over entities in a table, not general entities.
pub struct EntityIter<'w> {
    pub(crate) slice: &'w [Entity],

    #[cfg(debug_assertions)]
    pub(crate) _guard: Option<ReadGuard>,
}

unsafe impl ArrayLike for EntityIter<'_> {
    type Item = Entity;

    #[inline]
    unsafe fn get_unchecked(&self, index: usize) -> Self::Item {
        debug_assert!(
            index < self.slice.len(),
            "index out of bounds in entity iter"
        );

        *unsafe { self.slice.get_unchecked(index) }
    }

    #[inline]
    fn len(&self) -> usize {
        self.slice.len()
    }
}
