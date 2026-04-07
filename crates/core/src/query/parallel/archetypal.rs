use std::marker::PhantomData;

use rayon::iter::plumbing::{Folder, Producer, UnindexedProducer};

use crate::query::{
    ArchetypeIter, Filter, ParallelQueryIter, QueryGroup, TableCache, TableProducer,
};

pub struct ArchetypalProducer<'query, Q: QueryGroup, F: Filter> {
    current_tick: u32,
    last_run_tick: u32,

    tables: std::slice::Iter<'query, TableCache<Q>>,
    _marker: PhantomData<F>,
}

impl<'query, Q: QueryGroup, F: Filter> ArchetypalProducer<'query, Q, F> {
    pub fn new(iter: ParallelQueryIter<'query, Q, F>) -> Self {
        ArchetypalProducer {
            current_tick: iter.current_tick,
            last_run_tick: iter.last_run_tick,

            tables: iter.tables.iter(),
            _marker: PhantomData,
        }
    }
}

impl<'query, Q: QueryGroup, F: Filter> Producer for ArchetypalProducer<'query, Q, F> {
    type Item = TableProducer<'query, Q, F>;
    type IntoIter = ArchetypeIter<'query, Q, F>;

    fn into_iter(self) -> Self::IntoIter {
        ArchetypeIter {
            current_tick: self.current_tick,
            last_run_tick: self.last_run_tick,

            tables: self.tables,
            _marker: PhantomData,
        }
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left_tables, right_tables) = self.tables.as_slice().split_at(index);

        let left = Self {
            current_tick: self.current_tick,
            last_run_tick: self.last_run_tick,

            tables: left_tables.iter(),
            _marker: PhantomData,
        };

        let right = Self {
            current_tick: self.current_tick,
            last_run_tick: self.last_run_tick,

            tables: right_tables.iter(),
            _marker: PhantomData,
        };

        (left, right)
    }
}

impl<'query, Q: QueryGroup, F: Filter> UnindexedProducer for ArchetypalProducer<'query, Q, F> {
    type Item = TableProducer<'query, Q, F>;

    fn split(self) -> (Self, Option<Self>) {
        let index = self.tables.len() / 2;
        let (left, right) = self.split_at(index);

        (left, Some(right))
    }

    fn fold_with<T>(self, folder: T) -> T
    where
        T: Folder<Self::Item>,
    {
        todo!()
    }
}
