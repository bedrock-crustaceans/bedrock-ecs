use std::marker::PhantomData;

use rayon::iter::plumbing::{
    Consumer, Folder, Producer, ProducerCallback, UnindexedConsumer, UnindexedProducer, bridge,
    bridge_unindexed,
};
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

use crate::query::{ArchetypalFilter, Filter, QueryGroup, QueryIter, QueryState, TableCache};
use crate::world::World;

pub struct QueryFolder<'query, Q: QueryGroup, F: Filter> {
    _marker: PhantomData<&'query (Q, F)>,
}

impl<'query, Q: QueryGroup, F: Filter> Folder<Q::Output<'query>> for QueryFolder<'query, Q, F> {
    type Result = Q::Output<'query>;

    fn consume(self, item: Q::Output<'query>) -> Self {
        todo!()
    }

    fn complete(self) -> Self::Result {
        todo!()
    }

    fn full(&self) -> bool {
        todo!()
    }
}

pub struct UnindexedQueryProducer<'query, Q: QueryGroup, F: Filter> {
    last_run_tick: u32,
    current_tick: u32,
    remaining: usize,
    cache: &'query [TableCache<Q>],
    base_ptrs: Q::BasePtrs,
    filters: F::DynamicState,
}

impl<'query, Q: QueryGroup, F: Filter> UnindexedQueryProducer<'query, Q, F> {
    pub fn new(iter: ParallelQueryIter<'query, Q, F>) -> Self {
        todo!()
    }
}

impl<'query, Q: QueryGroup, F: Filter> UnindexedProducer for UnindexedQueryProducer<'query, Q, F> {
    type Item = Q::Output<'query>;

    fn split(self) -> (Self, Option<Self>) {
        todo!()
    }

    fn fold_with<T: Folder<Self::Item>>(self, folder: T) -> T {
        todo!()
    }
}

pub struct ParallelQueryIter<'query, Q: QueryGroup, F: Filter> {
    last_run_tick: u32,
    current_tick: u32,
    cache: &'query [TableCache<Q>],
    _marker: PhantomData<F>,
}

impl<'query, Q: QueryGroup, F: Filter> ParallelQueryIter<'query, Q, F> {
    pub fn from_state(world: &'query World, state: &'query QueryState<Q, F>) -> Self {
        Self {
            current_tick: world.current_tick,
            last_run_tick: state.last_run_tick,
            cache: &state.cache,
            _marker: PhantomData,
        }
    }

    fn unfiltered_len(&self) -> usize {
        self.cache
            .iter()
            .map(|c| unsafe { &*c.table.as_ptr() }.len())
            .sum::<usize>()
    }
}

impl<'query, Q: QueryGroup, F: Filter> ParallelIterator for ParallelQueryIter<'query, Q, F> {
    type Item = Q::Output<'query>;

    fn drive_unindexed<C: UnindexedConsumer<Self::Item>>(self, consumer: C) -> C::Result {
        let producer = UnindexedQueryProducer::<Q, F>::new(self);
        bridge_unindexed(producer, consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        F::IS_ARCHETYPAL.then(|| self.unfiltered_len())
    }
}

pub struct QueryProducer<'query, Q: QueryGroup, F: ArchetypalFilter> {
    _marker: PhantomData<(&'query Q, F)>,
}

impl<'query, Q: QueryGroup, F: ArchetypalFilter> Producer for QueryProducer<'query, Q, F> {
    type Item = Q::Output<'query>;
    type IntoIter = QueryIter<'query, Q, F>;

    fn into_iter(self) -> Self::IntoIter {
        todo!()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        todo!()
    }
}

impl<'query, Q: QueryGroup, F: ArchetypalFilter> IndexedParallelIterator
    for ParallelQueryIter<'query, Q, F>
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
