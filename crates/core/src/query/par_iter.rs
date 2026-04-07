use rayon::iter::plumbing::{Consumer, ProducerCallback, UnindexedConsumer};
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

use crate::query::{Filter, QueryGroup, QueryState};
use crate::world::World;

pub struct ParallelQueryIter<'query, Q: QueryGroup, F: Filter> {}

impl<'world, Q: QueryGroup, F: Filter> ParallelQueryIter<'world, Q, F> {
    pub fn from_state(world: &'world World, state: &'world mut QueryState<Q, F>) {
        todo!()
    }
}

impl<'query, Q: QueryGroup, F: Filter> ParallelIterator for ParallelQueryIter<'query, Q, F> {
    type Item: Q::Output<'query>;

    fn drive_unindexed<C: UnindexedConsumer<Self::Item>>(self, consumer: C) -> C::Result {
        rayon::iter::plumbing::bridge(self, consumer)
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
