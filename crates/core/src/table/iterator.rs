use std::iter::FusedIterator;
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::entity::{Entity, EntityRef};
use crate::query::{EmptyableIterator, Filter};
use crate::table::{ChangeTrackerIter, Mut, Ref};
use crate::world::World;

#[cfg(debug_assertions)]
use crate::util::debug::{ReadGuard, WriteGuard};

/// An immutable iterator over a column.
pub struct ColumnIter<'a, T, F: Filter> {
    pub(crate) current_tick: u32,
    pub(crate) tracker: ChangeTrackerIter<'a>,
    /// Pointer to current component.
    pub(crate) curr: Option<NonNull<T>>,
    /// Remaining elements
    pub(crate) remaining: usize,
    /// Ensures that the components and filters live at least as long as the column.
    pub(crate) _marker: PhantomData<(&'a T, F)>,

    #[cfg(debug_assertions)]
    pub(crate) _guard: Option<ReadGuard>,
}

impl<'a, T, F: Filter> Iterator for ColumnIter<'a, T, F> {
    type Item = Ref<'a, T>;

    fn next(&mut self) -> Option<Ref<'a, T>> {
        if self.remaining == 0 || self.curr.is_none() {
            return None;
        }

        // Check whether this item satisfies the filter
        if !F::apply_dynamic(self.tracker.next()?, self.current_tick) {
            return None;
        }

        let ptr = self.curr.as_mut().unwrap();
        let item = unsafe { ptr.as_ptr().cast_const().as_ref_unchecked() };

        self.remaining -= 1;

        // Safety: This is safe because by the check at the start of the function, there are
        // remaining elements.
        *ptr = unsafe { ptr.add(1) };

        Some(Ref { inner: item })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<T, F: Filter> ExactSizeIterator for ColumnIter<'_, T, F> {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<T, F: Filter> FusedIterator for ColumnIter<'_, T, F> {}

impl<'a, T, F: Filter> EmptyableIterator<'a, Ref<'a, T>> for ColumnIter<'a, T, F> {
    fn empty(_world: &'a World) -> ColumnIter<'a, T, F> {
        ColumnIter {
            current_tick: 0,
            tracker: ChangeTrackerIter::empty(),
            curr: None,
            remaining: 0,
            _marker: PhantomData,

            #[cfg(debug_assertions)]
            _guard: None,
        }
    }
}

/// A mutable iterator over a column.
pub struct ColumnIterMut<'a, T, F: Filter> {
    pub(crate) changes: ChangeTrackerIter<'a>,
    pub(crate) last_tick: u32,
    pub(crate) current_tick: u32,
    pub(crate) index: usize,
    /// Pointer to current component.
    pub(crate) curr: Option<NonNull<T>>,
    /// Remaining elements
    pub(crate) remaining: usize,
    pub(crate) _marker: PhantomData<(&'a mut T, F)>,

    #[cfg(debug_assertions)]
    pub(crate) _guard: Option<WriteGuard>,
}

impl<'a, T, F: Filter> Iterator for ColumnIterMut<'a, T, F> {
    type Item = Mut<'a, T>;

    fn next(&mut self) -> Option<Mut<'a, T>> {
        if self.remaining == 0 || self.curr.is_none() {
            return None;
        }

        if !F::apply_dynamic(self.changes.next()?, self.last_tick) {
            return None;
        }

        let ptr = self.curr.as_mut().unwrap();
        let item = unsafe { &mut *ptr.as_ptr() };

        self.remaining -= 1;
        self.index += 1;
        *ptr = unsafe { ptr.add(1) };

        Some(Mut {
            tracker: self
                .changes
                .tracker
                .expect("column iterator did not have a change tracker"),
            current_tick: self.current_tick,
            index: self.index - 1,
            inner: item,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<T, F: Filter> ExactSizeIterator for ColumnIterMut<'_, T, F> {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<T, F: Filter> FusedIterator for ColumnIterMut<'_, T, F> {}

impl<'a, T, F: Filter> EmptyableIterator<'a, Mut<'a, T>> for ColumnIterMut<'a, T, F> {
    fn empty(_world: &'a World) -> ColumnIterMut<'a, T, F> {
        ColumnIterMut {
            changes: ChangeTrackerIter::empty(),
            index: 0,
            last_tick: 0,
            current_tick: 0,

            curr: None,
            remaining: 0,
            _marker: PhantomData,

            #[cfg(debug_assertions)]
            _guard: None,
        }
    }
}

pub struct EntityIter<'w> {
    pub(crate) iter: std::slice::Iter<'w, Entity>,

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

impl<'w> EmptyableIterator<'w, Entity> for EntityIter<'w> {
    fn empty(_world: &'w World) -> EntityIter<'w> {
        EntityIter {
            iter: [].iter(),

            #[cfg(debug_assertions)]
            _guard: None,
        }
    }
}

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

impl<'w> EmptyableIterator<'w, EntityRef<'w>> for EntityRefIter<'w> {
    fn empty(world: &'w World) -> EntityRefIter<'w> {
        EntityRefIter {
            world,
            iter: [].iter(),

            #[cfg(debug_assertions)]
            _guard: None,
        }
    }
}
