use std::iter::FusedIterator;
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::entity::{Entity, EntityRef};
use crate::query::{ArrayLike, Filter};
use crate::table::{ChangeTracker, Mut};
use crate::world::World;

#[cfg(debug_assertions)]
use crate::util::debug::{ReadGuard, WriteGuard};

/// An immutable iterator over a column.
pub struct ColumnIter<'a, T, F: Filter> {
    pub(crate) current_tick: u32,
    pub(crate) tracker: &'a ChangeTracker,
    pub(crate) len: usize,
    pub(crate) base: NonNull<T>,
    /// Ensures that the components and filters live at least as long as the column.
    pub(crate) _marker: PhantomData<(&'a T, F)>,

    #[cfg(debug_assertions)]
    pub(crate) _guard: Option<ReadGuard>,
}

unsafe impl<'a, T, F: Filter> ArrayLike for ColumnIter<'a, T, F> {
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

impl<'a, T, F: Filter> ColumnIter<'a, T, F> {
    /// # Safety
    ///
    /// Caller must ensure that the iterator is still within range.
    #[inline]
    unsafe fn next_zst(&self) -> &'a T {
        debug_assert_eq!(std::mem::size_of::<T>(), 0);
        unsafe { &*std::ptr::dangling() }
    }

    /// # Safety
    ///
    /// Caller must ensure that the iterator is still within range.
    #[inline]
    unsafe fn next_nozst(&self, index: usize) -> &'a T {
        let ptr = unsafe { self.base.add(index) };
        unsafe { &*ptr.as_ptr() }
    }

    #[inline]
    pub unsafe fn filter(&self, index: usize) -> bool {
        F::apply_dynamic(
            unsafe { self.tracker.index_unchecked(index) },
            self.current_tick,
        )
    }

    #[inline]
    pub unsafe fn index(&self, index: usize) -> &'a T {
        if std::mem::size_of::<T>() == 0 {
            unsafe { self.next_zst() }
        } else {
            unsafe { self.next_nozst(index) }
        }
    }
}

/// A mutable iterator over a column.
pub struct ColumnIterMut<'a, T, F: Filter> {
    pub(crate) changes: &'a ChangeTracker,
    pub(crate) last_tick: u32,
    pub(crate) current_tick: u32,
    pub(crate) len: usize,
    pub(crate) base: NonNull<T>,
    pub(crate) _marker: PhantomData<(&'a mut T, F)>,

    #[cfg(debug_assertions)]
    pub(crate) _guard: Option<WriteGuard>,
}

unsafe impl<'a, T, F: Filter> ArrayLike for ColumnIterMut<'a, T, F> {
    type Item = Mut<'a, T>;

    #[inline]
    unsafe fn get_unchecked(&self, index: usize) -> Mut<'a, T> {
        debug_assert!(isize::try_from(index).is_ok());

        let inner = unsafe { &mut *self.base.add(index).as_ptr() };
        Mut {
            index,
            current_tick: self.current_tick,
            tracker: self.changes,
            inner,
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}

pub struct EntityIter<'w> {
    pub(crate) iter: std::slice::Iter<'w, Entity>,

    #[cfg(debug_assertions)]
    pub(crate) _guard: Option<ReadGuard>,
}

impl Iterator for EntityIter<'_> {
    type Item = Entity;

    fn next(&mut self) -> Option<Entity> {
        let handle = *self.iter.next()?;
        Some(handle)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl ExactSizeIterator for EntityIter<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl FusedIterator for EntityIter<'_> {}

pub struct EntityRefIter<'w> {
    pub(crate) world: &'w World,
    pub(crate) iter: std::slice::Iter<'w, Entity>,

    #[cfg(debug_assertions)]
    pub(crate) _guard: Option<ReadGuard>,
}

impl<'w> Iterator for EntityRefIter<'w> {
    type Item = EntityRef<'w>;

    fn next(&mut self) -> Option<EntityRef<'w>> {
        let id = self.iter.next()?;

        Some(EntityRef {
            handle: *id,
            world: self.world,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl ExactSizeIterator for EntityRefIter<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl FusedIterator for EntityRefIter<'_> {}
