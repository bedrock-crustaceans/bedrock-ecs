use std::iter::FusedIterator;
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::entity::{Entity, EntityHandle, EntityRef};
use crate::query::{EmptyableIterator, FilterBundle};
use crate::table::{ChangeTracker, ChangeTrackerIter, Mut, Ref, Table, TableRow};
use crate::world::World;

#[cfg(debug_assertions)]
use crate::util::debug::{ReadGuard, WriteGuard};

pub struct ColumnIter<'a, T, F: FilterBundle> {
    pub(crate) current_tick: u32,
    pub(crate) tracker: ChangeTrackerIter<'a>,
    /// Pointer to current component.
    pub(crate) curr: Option<NonNull<T>>,
    /// Remaining elements
    pub(crate) remaining: usize,
    pub(crate) _marker: PhantomData<(&'a T, F)>,

    #[cfg(debug_assertions)]
    pub(crate) _guard: Option<ReadGuard>,
}

impl<'a, T, F: FilterBundle> Iterator for ColumnIter<'a, T, F> {
    type Item = Ref<'a, T>;

    fn next(&mut self) -> Option<Ref<'a, T>> {
        if self.remaining == 0 || self.curr.is_none() {
            return None;
        }

        // Check whether this item satisfies the filter
        if !F::apply_dynamic_filters(self.tracker.next()?, self.current_tick) {
            return None;
        }

        let ptr = self.curr.as_mut().unwrap();
        let item = unsafe { &*ptr.as_ptr().cast_const() };

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

impl<T, F: FilterBundle> ExactSizeIterator for ColumnIter<'_, T, F> {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<T, F: FilterBundle> FusedIterator for ColumnIter<'_, T, F> {}

impl<'a, T, F: FilterBundle> EmptyableIterator<'a, Ref<'a, T>> for ColumnIter<'a, T, F> {
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

pub struct ColumnIterMut<'a, T, F: FilterBundle> {
    pub(crate) changes: Option<&'a ChangeTracker>,
    pub(crate) index: usize,
    /// Pointer to current component.
    pub(crate) curr: Option<NonNull<T>>,
    /// Remaining elements
    pub(crate) remaining: usize,
    pub(crate) _marker: PhantomData<(&'a mut T, F)>,

    #[cfg(debug_assertions)]
    pub(crate) _guard: Option<WriteGuard>,
}

impl<'a, T, F: FilterBundle> Iterator for ColumnIterMut<'a, T, F> {
    type Item = Mut<'a, T>;

    fn next(&mut self) -> Option<Mut<'a, T>> {
        if self.remaining == 0 || self.curr.is_none() {
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
                .as_ref()
                .expect("column iterator did not have a change tracker"),
            index: self.index - 1,
            inner: item,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<T, F: FilterBundle> ExactSizeIterator for ColumnIterMut<'_, T, F> {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<T, F: FilterBundle> FusedIterator for ColumnIterMut<'_, T, F> {}

impl<'a, T, F: FilterBundle> EmptyableIterator<'a, Mut<'a, T>> for ColumnIterMut<'a, T, F> {
    fn empty(_world: &'a World) -> ColumnIterMut<'a, T, F> {
        ColumnIterMut {
            changes: None,
            index: 0,

            curr: None,
            remaining: 0,
            _marker: PhantomData,

            #[cfg(debug_assertions)]
            _guard: None,
        }
    }
}

pub struct EntityIter<'w> {
    pub(crate) table: Option<NonNull<Table>>,
    pub(crate) row_index: usize,
    pub(crate) iter: std::slice::Iter<'w, EntityHandle>,
}

impl Iterator for EntityIter<'_> {
    type Item = Entity;

    fn next(&mut self) -> Option<Entity> {
        let handle = *self.iter.next()?;
        let row_index = self.row_index;

        self.row_index += 1;

        Some(Entity {
            table: self.table,
            row: TableRow(row_index),
            handle,
        })
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
            table: None,
            row_index: 0,
            iter: [].iter(),
        }
    }
}

pub struct EntityRefIter<'w> {
    pub(crate) world: &'w World,
    pub(crate) iter: std::slice::Iter<'w, EntityHandle>,
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
        }
    }
}
