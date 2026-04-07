//! Implements the query iterator functionality.
//!
use std::any::TypeId;
use std::iter::FusedIterator;
use std::marker::PhantomData;
use std::ptr::NonNull;

use nonmax::NonMaxUsize;

use generic_array::{ArrayLength, GenericArray};
use rustc_hash::FxHashMap;

use crate::archetype::Signature;
use crate::component::TypeRegistry;
use crate::query::{
    ArchetypalFilter, Filter, QueryData, QueryGroup, QueryState, QueryType, TableCache,
};
use crate::scheduler::AccessDesc;
use crate::table::{ColumnRow, Table};
use crate::world::World;

/// An iterator that can jump from table to table.
///
/// This is useful when querying a component that is contained in multiple archetypes.
#[cfg(feature = "generics")]
pub trait FragmentIterator<'t, Q: QueryGroup + 't, F: Filter>:
    Iterator<Item = Q::Output<'t>>
{
    /// Creates a new iterator over the given cache.
    fn from_state(world: &'t World, meta: &'t QueryState<Q, F>) -> Self;
}

pub struct QueryIter<'query, Q: QueryGroup, F: Filter> {
    pub(crate) last_run_tick: u32,
    pub(crate) current_tick: u32,
    pub(crate) remaining: usize,
    pub(crate) cache: std::slice::Iter<'query, TableCache<Q>>,
    pub(crate) base_ptrs: Q::BasePtrs,
    pub(crate) filters: F::DynamicState,
}

impl<'query, Q: QueryGroup, F: Filter> FragmentIterator<'query, Q, F> for QueryIter<'query, Q, F> {
    fn from_state(world: &'query World, meta: &'query QueryState<Q, F>) -> Self {
        let mut cache = meta.cache.iter();

        // Look up all column base pointers.
        let Some(first_cache) = cache.next() else {
            return Self::empty();
        };

        let table = unsafe { &*first_cache.table.as_ptr() };
        Self {
            last_run_tick: meta.last_run_tick,
            current_tick: world.current_tick,
            remaining: table.len(),
            cache,
            base_ptrs: Q::get_base_ptrs(table),
            filters: F::set_dynamic_state(table),
        }
    }
}

impl<'query, Q: QueryGroup, F: Filter> QueryIter<'query, Q, F> {
    /// Creates an empty iterator that always returns `None`. This exists because
    /// [`std::iter::empty()`] returns a concrete [`Empty`] type that is incompatible with the trait.
    ///
    /// [`Empty`]: std::iter::Empty
    pub fn empty() -> Self {
        Self {
            last_run_tick: 0,
            current_tick: 0,
            remaining: 0,
            cache: [].iter(),
            base_ptrs: Q::dangling(),
            filters: F::dangling(),
        }
    }

    /// Computes the remaining length of the query iterator if it had no filters.
    fn unfiltered_len(&self) -> usize {
        let tables = self
            .cache
            .as_slice()
            .iter()
            .map(|c| unsafe { &*c.table.as_ptr() }.len())
            .sum::<usize>();

        tables + self.remaining
    }

    /// Jumps to the next table, returning whether the jump was successful
    /// or whether the end of the query has been reached.
    fn jump(&mut self) -> bool {
        if let Some(next_cache) = self.cache.next() {
            let table = unsafe { &*next_cache.table.as_ptr() };
            self.base_ptrs = Q::get_base_ptrs(table);
            self.remaining = table.len();

            true
        } else {
            self.remaining = 0;
            false
        }
    }
}

impl<'query, Q: QueryGroup, F: Filter> Iterator for QueryIter<'query, Q, F> {
    type Item = Q::Output<'query>;

    #[allow(non_snake_case, unused)]
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // Check whether iterator is empty
        if self.remaining == 0 && !self.jump() {
            return None;
        }

        if !F::IS_ARCHETYPAL {
            todo!("support double ended iteration and arbitrary splitting");
            let should_return = F::apply_dynamic(&self.filters, self.last_run_tick);
            assert!(should_return);
        }

        let item = unsafe { Q::fetch_relative(self.base_ptrs, 0, self.current_tick) };
        unsafe { Q::offset_ptrs(&mut self.base_ptrs, 1) };

        self.remaining -= 1;
        return Some(item);
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let upper_bound = self.unfiltered_len();

        if const { F::TRIVIAL } {
            // If this query performs no filtering, we know the exact size.
            (upper_bound, Some(upper_bound))
        } else {
            // Otherwise it has a size ranging from zero to the maximum of the query.
            (0, Some(upper_bound))
        }
    }
}

impl<'query, Q: QueryGroup, F: ArchetypalFilter> ExactSizeIterator for QueryIter<'query, Q, F> {
    #[inline]
    fn len(&self) -> usize {
        self.unfiltered_len()
    }
}

impl<'query, Q: QueryGroup, F: Filter> FusedIterator for QueryIter<'query, Q, F> {}

impl<'query, Q: QueryGroup, F: Filter> DoubleEndedIterator for QueryIter<'query, Q, F> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        if !F::IS_ARCHETYPAL {
            todo!("support double ended iteration and arbitrary splitting");
            let should_return = F::apply_dynamic(&self.filters, self.last_run_tick);
            assert!(should_return);
        }

        let item = unsafe {
            Q::fetch_relative(
                self.base_ptrs,
                self.remaining as isize - 1,
                self.current_tick,
            )
        };

        self.remaining -= 1;
        Some(item)
    }
}
