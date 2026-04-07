use std::iter::FusedIterator;
use std::marker::PhantomData;

use rayon::iter::plumbing::{
    Consumer, Folder, Producer, ProducerCallback, UnindexedConsumer, UnindexedProducer, bridge,
    bridge_unindexed,
};
use rayon::iter::{FlatMap, IndexedParallelIterator, ParallelIterator};

use crate::query::{
    ArchetypalFilter, ArchetypalProducer, Filter, QueryGroup, QueryIter, QueryState, TableCache,
    TableProducer,
};
use crate::world::World;

pub const PAR_ITER_MIN_LEN: usize = 128;

pub struct ParallelQueryIter<'query, Q: QueryGroup, F: Filter> {
    pub(super) last_run_tick: u32,
    pub(super) current_tick: u32,
    pub(super) tables: &'query [TableCache<Q>],
    pub(super) _marker: PhantomData<F>,
}

impl<'query, Q: QueryGroup, F: Filter> ParallelQueryIter<'query, Q, F> {
    pub fn from_state(world: &'query World, state: &'query QueryState<Q, F>) {
        let iter = Self {
            current_tick: world.current_tick,
            last_run_tick: state.last_run_tick,
            tables: &state.cache,
            _marker: PhantomData,
        };
    }

    fn unfiltered_len(&self) -> usize {
        self.tables
            .iter()
            .map(|c| unsafe { &*c.table.as_ptr() }.len())
            .sum::<usize>()
    }
}

impl<'query, Q: QueryGroup, F: Filter> ParallelIterator for ParallelQueryIter<'query, Q, F> {
    type Item = TableProducer<'query, Q, F>;

    fn drive_unindexed<C: UnindexedConsumer<Self::Item>>(self, consumer: C) -> C::Result {
        let producer = ArchetypalProducer::new(self);
        bridge_unindexed(producer, consumer)
    }
}

impl<'query, Q: QueryGroup, F: Filter> IndexedParallelIterator for ParallelQueryIter<'query, Q, F> {
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }

    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
        callback.callback(ArchetypalProducer::new(self))
    }

    /// The amount of tables that this iterator will iterate over.
    fn len(&self) -> usize {
        self.tables.len()
    }
}

/// Creates an iterator, potentially over multiple tables.
///
/// If a single table is too large, the iterator might split a single table
/// into multiple parallel chunks.
pub struct ArchetypeIter<'query, Q: QueryGroup, F: Filter> {
    pub(super) current_tick: u32,
    pub(super) last_run_tick: u32,

    pub(super) tables: std::slice::Iter<'query, TableCache<Q>>,
    pub(super) _marker: PhantomData<F::DynamicState>,
}

impl<'query, Q: QueryGroup, F: Filter> Iterator for ArchetypeIter<'query, Q, F> {
    type Item = TableProducer<'query, Q, F>;

    fn next(&mut self) -> Option<Self::Item> {
        let table = self.tables.next()?;
        Some(TableProducer::new(table, self))
    }
}

impl<'query, Q: QueryGroup, F: Filter> FusedIterator for ArchetypeIter<'query, Q, F> {}

impl<'query, Q: QueryGroup, F: Filter> DoubleEndedIterator for ArchetypeIter<'query, Q, F> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let table = self.tables.next_back()?;
        Some(TableProducer::new(table, self))
    }
}

impl<'query, Q: QueryGroup, F: Filter> ExactSizeIterator for ArchetypeIter<'query, Q, F> {
    fn len(&self) -> usize {
        self.tables.len()
    }
}

pub struct TableIter<'query, Q: QueryGroup, F: Filter> {
    pub(super) remaining: usize,
    pub(super) ptrs: Q::BasePtrs,
    pub(super) filters: F::DynamicState,
    pub(super) _marker: PhantomData<&'query Q>,
}

impl<'query, Q: QueryGroup, F: Filter> Iterator for TableIter<'query, Q, F> {
    type Item = Q::Output<'query>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

impl<'query, Q: QueryGroup, F: Filter> FusedIterator for TableIter<'query, Q, F> {}

impl<'query, Q: QueryGroup, F: Filter> DoubleEndedIterator for TableIter<'query, Q, F> {
    fn next_back(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

impl<'query, Q: QueryGroup, F: ArchetypalFilter> ExactSizeIterator for TableIter<'query, Q, F> {
    fn len(&self) -> usize {
        todo!()
    }
}
