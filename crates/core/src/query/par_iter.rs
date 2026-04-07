use std::iter::FusedIterator;
use std::marker::PhantomData;

use rayon::iter::plumbing::{
    Consumer, Folder, Producer, ProducerCallback, UnindexedConsumer, UnindexedProducer, bridge,
    bridge_producer_consumer, bridge_unindexed,
};
use rayon::iter::{
    FlatMap, FlatMapIter, IndexedParallelIterator, IntoParallelIterator, ParallelIterator,
};

use crate::query::{ArchetypalFilter, Filter, QueryGroup, QueryIter, QueryState, TableCache};
use crate::world::World;

pub const PAR_ITER_MIN_LEN: usize = 128;

pub struct TableProducer<'query, Q: QueryGroup, F: Filter> {
    current_tick: u32,
    last_run_tick: u32,
    remaining: usize,
    base: Q::BasePtrs,
    filters: F::DynamicState,
    _marker: PhantomData<&'query ()>,
}

impl<'query, Q: QueryGroup, F: Filter> TableProducer<'query, Q, F> {
    pub fn new(iter: ParallelTableIter<'query, Q, F>) -> Self {
        Self {
            current_tick: iter.current_tick,
            last_run_tick: iter.last_run_tick,
            remaining: iter.remaining,
            base: iter.base,
            filters: iter.filters,
            _marker: PhantomData,
        }
    }
}

impl<'query, Q: QueryGroup + 'query, F: Filter> UnindexedProducer for TableProducer<'query, Q, F> {
    type Item = Q::Output<'query>;

    fn split(self) -> (Self, Option<Self>) {
        if self.remaining == 1 {
            // Cannot split any further
            return (self, None);
        }

        let left_remaining = self.remaining.div_ceil(2);
        let right_remaining = self.remaining / 2;

        assert!(left_remaining < isize::MAX as usize);

        let left = Self {
            remaining: left_remaining,
            ..self
        };

        let right = Self {
            base: unsafe { Q::offset_ptrs(self.base, left_remaining as isize) },
            filters: unsafe { F::offset_dynamic_state(self.filters, left_remaining as isize) },
            remaining: right_remaining,
            ..self
        };

        (left, Some(right))
    }

    fn fold_with<T>(self, folder: T) -> T
    where
        T: Folder<Self::Item>,
    {
        let seq_iter = SequentialTableIter::from_producer(self);
        folder.consume_iter(seq_iter)
    }
}

pub struct SequentialTableIter<'query, Q: QueryGroup + 'query, F: Filter> {
    current_tick: u32,
    last_run_tick: u32,
    remaining: usize,
    base: Q::BasePtrs,
    filters: F::DynamicState,
    _marker: PhantomData<&'query ()>,
}

impl<'query, Q: QueryGroup + 'query, F: Filter> SequentialTableIter<'query, Q, F> {
    pub fn from_producer(producer: TableProducer<'query, Q, F>) -> Self {
        Self {
            current_tick: producer.current_tick,
            last_run_tick: producer.last_run_tick,
            remaining: producer.remaining,
            base: producer.base,
            filters: producer.filters,
            _marker: PhantomData,
        }
    }
}

impl<'query, Q: QueryGroup + 'query, F: Filter> Iterator for SequentialTableIter<'query, Q, F> {
    type Item = Q::Output<'query>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining > 0 {
            if !F::IS_ARCHETYPAL {
                todo!("dynamic filtering in par iter");
            }

            let item = unsafe { Q::fetch_relative(self.base, 0, self.current_tick) };

            self.remaining -= 1;
            self.base = unsafe { Q::offset_ptrs(self.base, 1) };

            Some(item)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let lower_bound = if F::IS_ARCHETYPAL { self.remaining } else { 0 };
        (lower_bound, Some(self.remaining))
    }
}

impl<'query, Q: QueryGroup, F: Filter> FusedIterator for SequentialTableIter<'query, Q, F> {}

impl<'query, Q: QueryGroup, F: Filter> DoubleEndedIterator for SequentialTableIter<'query, Q, F> {
    fn next_back(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

impl<'query, Q: QueryGroup, F: ArchetypalFilter> ExactSizeIterator
    for SequentialTableIter<'query, Q, F>
{
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<'query, Q: QueryGroup + 'query, F: ArchetypalFilter> Producer for TableProducer<'query, Q, F> {
    type Item = Q::Output<'query>;
    type IntoIter = SequentialTableIter<'query, Q, F>;

    fn into_iter(self) -> Self::IntoIter {
        SequentialTableIter::from_producer(self)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        todo!()
    }
}

/// Parallel iterator over a single archetype table.
pub struct ParallelTableIter<'query, Q: QueryGroup + 'query, F: Filter> {
    current_tick: u32,
    last_run_tick: u32,
    remaining: usize,
    base: Q::BasePtrs,
    filters: F::DynamicState,
    _marker: PhantomData<&'query ()>,
}

impl<'query, Q: QueryGroup + 'query, F: Filter> ParallelTableIter<'query, Q, F> {
    /// Creates an iterator over the entire table.
    pub fn new(cache: &'query TableCache<Q>, last_run_tick: u32, current_tick: u32) -> Self {
        let table = unsafe { &*cache.table.as_ptr() };

        Self {
            remaining: table.len(),
            current_tick,
            last_run_tick,
            base: Q::get_base_ptrs(table),
            filters: F::set_dynamic_state(table),
            _marker: PhantomData,
        }
    }

    fn unfiltered_len(&self) -> usize {
        self.remaining
    }
}

impl<'query, Q: QueryGroup + 'query, F: Filter> ParallelIterator
    for ParallelTableIter<'query, Q, F>
{
    type Item = Q::Output<'query>;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let producer = TableProducer::new(self);
        bridge_unindexed(producer, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        F::IS_ARCHETYPAL.then(|| self.unfiltered_len())
    }
}

impl<'query, Q: QueryGroup, F: ArchetypalFilter> IndexedParallelIterator
    for ParallelTableIter<'query, Q, F>
{
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        bridge(self, consumer)
    }

    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
        todo!()
    }

    fn len(&self) -> usize {
        self.unfiltered_len()
    }
}

// impl<'query, Q: QueryGroup, F: ArchetypalFilter> ExactSizeIterator
//     for ParallelTableIter<'query, Q, F>
// {
//     fn len(&self) -> usize {
//         self.unfiltered_len()
//     }
// }

pub struct ParallelQueryIter<'query, Q: QueryGroup, F: Filter> {
    pub(super) last_run_tick: u32,
    pub(super) current_tick: u32,
    pub(super) tables: &'query [TableCache<Q>],
    pub(super) _marker: PhantomData<F>,
}

impl<'query, Q: QueryGroup, F: Filter> ParallelQueryIter<'query, Q, F> {
    pub fn from_state(state: &'query QueryState<Q, F>) -> Self {
        Self {
            last_run_tick: state.last_run_tick,
            current_tick: state.current_tick,
            tables: &state.cache,
            _marker: PhantomData,
        }
    }

    fn unfiltered_len(&self) -> usize {
        self.tables
            .iter()
            .map(|cache| unsafe { &*cache.table.as_ptr() }.len())
            .sum::<usize>()
    }
}

impl<'query, Q: QueryGroup, F: Filter> ParallelIterator for ParallelQueryIter<'query, Q, F> {
    type Item = Q::Output<'query>;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.tables
            .into_par_iter()
            .flat_map(|table| {
                ParallelTableIter::<Q, F>::new(table, self.last_run_tick, self.current_tick)
            })
            .drive_unindexed(consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        F::IS_ARCHETYPAL.then(|| self.unfiltered_len())
    }
}

impl<'query, Q: QueryGroup, F: ArchetypalFilter> IndexedParallelIterator
    for ParallelQueryIter<'query, Q, F>
{
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        todo!()
    }

    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
        todo!()
    }

    fn len(&self) -> usize {
        self.unfiltered_len()
    }
}
