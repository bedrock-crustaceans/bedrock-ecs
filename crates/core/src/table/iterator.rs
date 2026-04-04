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
    pub(crate) tracker: *const ChangeTracker,
    pub(crate) len: usize,
    // pub(crate) base: NonNull<T>,
    pub(crate) curr: *mut T,
    // pub(crate) end: NonNull<T>,
    /// Ensures that the components and filters live at least as long as the column.
    pub(crate) _marker: PhantomData<(&'a T, F)>,

    #[cfg(debug_assertions)]
    pub(crate) _guard: Option<ReadGuard>,
}

unsafe impl<'a, T, F: Filter> ArrayLike for ColumnArray<'a, T, F> {
    type Item = &'a T;

    #[inline]
    unsafe fn get_unchecked(&mut self, index: usize) -> &'a T {
        assert!(isize::try_from(index).is_ok());
        // debug_assert!(index < self.len, "column iterator index out of range");

        if const { std::mem::size_of::<T>() == 0 } {
            // Safety: we cannot construct `T` directly so we dereference a
            // dangling (but aligned) pointer, which is safe for inhabited ZSTs.
            unsafe { &*std::ptr::dangling::<T>() }
        } else {
            let item = unsafe { &*self.curr.cast_const() };
            self.curr = unsafe { self.curr.add(1) };
            item
        }
    }

    #[inline]
    fn empty() -> Self {
        Self {
            current_tick: 0,
            tracker: std::ptr::null(),
            len: 0,
            // curr: NonNull::<T>::dangling(),
            curr: std::ptr::dangling_mut::<T>(),
            // base: NonNull::<T>::dangling(),
            _marker: PhantomData,

            #[cfg(debug_assertions)]
            _guard: None,
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, T, F: Filter> ColumnArray<'a, T, F> {
    /// # Safety
    ///
    /// `ìndex` must be in range.
    #[inline]
    pub unsafe fn filter(&self, index: usize) -> bool {
        todo!();
        // F::apply_dynamic(
        //     unsafe { (*self.tracker).index_unchecked(index) },
        //     self.current_tick,
        // )
    }
}

pub struct ColumnIterMut<'a, T, F: Filter> {
    pub(crate) tracker: *mut ChangeTracker,
    pub(crate) last_tick: u32,
    pub(crate) current_tick: u32,
    pub(crate) len: usize,
    pub(crate) base: NonNull<T>,
    pub(crate) _marker: PhantomData<(&'a mut T, F)>,

    #[cfg(debug_assertions)]
    pub(crate) _guard: Option<WriteGuard>,
}

impl<'a, T, F: Filter> ColumnIterMut<'a, T, F> {
    /// # Safety
    ///
    /// `ìndex` must be in range.
    #[inline]
    pub unsafe fn filter(&self, index: usize) -> bool {
        F::apply_dynamic(
            unsafe { (*self.tracker).index_unchecked(index) },
            self.current_tick,
        )
    }
}

unsafe impl<'a, T, F: Filter> ArrayLike for ColumnIterMut<'a, T, F> {
    type Item = Mut<'a, T>;

    #[inline]
    unsafe fn get_unchecked(&mut self, index: usize) -> Mut<'a, T> {
        debug_assert!(isize::try_from(index).is_ok()); // soundness requirement for `std::ptr::add`.
        debug_assert!(
            index < self.len,
            "mutable column iterator index out of range"
        );

        let inner = if const { std::mem::size_of::<T>() == 0 } {
            // Safety: we cannot construct `T` directly so we dereference a
            // dangling (but aligned) pointer, which is safe for inhabited ZSTs.
            unsafe { &mut *std::ptr::dangling_mut::<T>() }
        } else {
            // let ptr = unsafe { self.base.add(index) };
            // unsafe { &mut *ptr.as_ptr() }
            //
            let slice = unsafe { std::slice::from_raw_parts_mut(self.base.as_ptr(), self.len) };
            unsafe { slice.get_unchecked_mut(index) }
        };

        Mut {
            index,
            current_tick: self.current_tick,
            tracker: unsafe { &mut *self.tracker },
            inner,
        }
    }

    #[inline]
    fn empty() -> Self {
        Self {
            current_tick: 0,
            last_tick: 0,
            tracker: std::ptr::null_mut(),
            len: 0,
            base: NonNull::dangling(),
            _marker: PhantomData,

            #[cfg(debug_assertions)]
            _guard: None,
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
    unsafe fn get_unchecked(&mut self, index: usize) -> Self::Item {
        debug_assert!(
            index < self.slice.len(),
            "index out of bounds in entity iter"
        );

        *unsafe { self.slice.get_unchecked(index) }
    }

    #[inline]
    fn empty() -> Self {
        Self {
            slice: &[],

            #[cfg(debug_assertions)]
            _guard: None,
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.slice.len()
    }
}
