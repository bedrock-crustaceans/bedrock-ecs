use std::marker::PhantomData;

use rayon::iter::plumbing::{Folder, Producer, UnindexedProducer};

use crate::query::{ArchetypalFilter, ArchetypeIter, Filter, QueryGroup, TableCache, TableIter};

/// Split work inside of a table.
pub struct TableProducer<'query, Q: QueryGroup + 'query, F: Filter> {
    remaining: usize,
    ptrs: Q::BasePtrs,
    filters: F::DynamicState,
    _marker: PhantomData<&'query ()>,
}

impl<'query, Q: QueryGroup, F: Filter> TableProducer<'query, Q, F> {
    /// Creates a producer for an entire table.
    pub fn new(cache: &'query TableCache<Q>, iter: &ArchetypeIter<'query, Q, F>) -> Self {
        let table = unsafe { &*cache.table.as_ptr() };

        Self {
            remaining: table.len(),
            ptrs: Q::get_base_ptrs(table),
            filters: F::set_dynamic_state(table),
            _marker: PhantomData,
        }
    }
}

impl<'query, Q: QueryGroup, F: Filter> UnindexedProducer for TableProducer<'query, Q, F> {
    type Item = Q::Output<'query>;

    /// Attempts to split a table in half to be processed by multiple threads.
    fn split(self) -> (Self, Option<Self>) {
        todo!();
    }

    fn fold_with<T>(self, folder: T) -> T
    where
        T: Folder<Self::Item>,
    {
        todo!()
    }
}

impl<'query, Q: QueryGroup, F: ArchetypalFilter> Producer for TableProducer<'query, Q, F> {
    type Item = Q::Output<'query>;
    type IntoIter = TableIter<'query, Q, F>;

    fn into_iter(self) -> Self::IntoIter {
        todo!()
    }

    /// Splits a table in half to be processed by multiple threads.
    fn split_at(self, index: usize) -> (Self, Self) {
        todo!()
    }

    fn fold_with<T>(self, folder: T) -> T
    where
        T: Folder<Self::Item>,
    {
        todo!()
    }
}
