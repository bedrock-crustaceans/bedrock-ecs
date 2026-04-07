use std::marker::PhantomData;

use rayon::iter::plumbing::{
    Consumer, Folder, ProducerCallback, UnindexedConsumer, UnindexedProducer, bridge_unindexed,
};
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

use crate::query::{Filter, QueryGroup, QueryState, TableCache};
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
    remaining: usize,
    cache: std::slice::Iter<'query, TableCache<Q>>,
    base_ptrs: Q::BasePtrs,
    filters: F::DynamicState,
}

impl<'world, Q: QueryGroup, F: Filter> ParallelQueryIter<'world, Q, F> {
    pub fn from_state(world: &'world World, state: &'world QueryState<Q, F>) -> Self {
        todo!()
    }
}

impl<'query, Q: QueryGroup, F: Filter> ParallelIterator for ParallelQueryIter<'query, Q, F> {
    type Item = Q::Output<'query>;

    fn drive_unindexed<C: UnindexedConsumer<Self::Item>>(self, consumer: C) -> C::Result {
        let producer = UnindexedQueryProducer::new(self);
        bridge_unindexed(producer, consumer)
    }
}

impl<'query, Q: QueryGroup> IndexedParallelIterator for ParallelQueryIter<'query, Q, ()> {
    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {}

    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        rayon::iter::plumbing::bridge(self, consumer)
    }

    fn len(&self) -> usize {
        todo!()
    }
}
