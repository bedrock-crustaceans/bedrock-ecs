use std::{iter::FusedIterator, marker::PhantomData, ptr::NonNull};

use crate::{entity::{Entity, EntityId}, world::World};

pub struct ColumnIter<'a, T> {
    /// Pointer to current component.
    pub(crate) curr: Option<NonNull<T>>,
    /// Remaining elements
    pub(crate) remaining: usize,
    pub(crate) _marker: PhantomData<&'a T>
}

impl<'a, T> Iterator for ColumnIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        if self.remaining == 0 && self.curr.is_none() {
            return None
        }

        let ptr = self.curr.as_mut().unwrap();
        let item = unsafe {
            &*ptr.as_ptr().cast_const()
        };

        self.remaining -= 1;
        *ptr = unsafe {
            ptr.add(1)
        };

        Some(item)   
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, T> ExactSizeIterator for ColumnIter<'a, T> {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<'a, T> FusedIterator for ColumnIter<'a, T> {}

pub struct ColumnIterMut<'a, T> {
    /// Pointer to current component.
    pub(crate) curr: Option<NonNull<T>>,
    /// Remaining elements
    pub(crate) remaining: usize,
    pub(crate) _marker: PhantomData<&'a mut T>
}

impl<'a, T> Iterator for ColumnIterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<&'a mut T> {
        if self.remaining == 0 && self.curr.is_none() {
            return None
        }

        let ptr = self.curr.as_mut().unwrap();
        let item = unsafe {
            &mut *ptr.as_ptr()
        };

        self.remaining -= 1;
        *ptr = unsafe {
            ptr.add(1)
        };

        Some(item)   
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, T> ExactSizeIterator for ColumnIterMut<'a, T> {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<'a, T> FusedIterator for ColumnIterMut<'a, T> {}

pub struct EntityIter<'w> {
    pub(crate) world: &'w World,
    pub(crate) iter: std::slice::Iter<'w, EntityId>
}

impl<'w> Iterator for EntityIter<'w> {
    type Item = Entity<'w>;

    fn next(&mut self) -> Option<Entity<'w>> {
        let id = self.iter.next()?;
        Some(Entity {
            id: *id,
            world: self.world
        })
    }
}

impl<'t> ExactSizeIterator for EntityIter<'t> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'t> FusedIterator for EntityIter<'t> {}