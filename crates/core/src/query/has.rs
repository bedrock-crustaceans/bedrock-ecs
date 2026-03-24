use std::{
    any::TypeId,
    iter::{Cycle, FusedIterator, Once},
    marker::PhantomData,
};

use nonmax::NonMaxUsize;
use rustc_hash::FxHashMap;

use crate::{
    archetype::Signature,
    component::{Component, ComponentId, ComponentRegistry},
    prelude::ComponentBundle,
    query::{
        EmptyableIterator, Filter, Impossible, QueryBundle, QueryData, QueryState, QueryType,
        TableCache,
    },
    scheduler::{AccessDesc, AccessType},
    table::{Table, TableRow},
    world::World,
};

/// Whether the entity has the given set of components.
pub struct Has<T: ComponentBundle> {
    _marker: PhantomData<T>,
}

unsafe impl<T: ComponentBundle> QueryData for Has<T> {
    type Unref = Has<T>;

    type Output<'w> = bool;
    type Iter<'t, F: Filter> = HasIter<'t>;

    const TY: QueryType = QueryType::Has;

    fn access(_reg: &mut ComponentRegistry) -> AccessDesc {
        AccessDesc {
            ty: AccessType::None,
            exclusive: false,
        }
    }

    fn component_id(_reg: &mut ComponentRegistry) -> ComponentId {
        unimplemented!()
    }

    fn cache_column(_map: &FxHashMap<TypeId, usize>) -> NonMaxUsize {
        unimplemented!()
    }

    fn get<'w, Q: QueryBundle, F: Filter>(
        world: &'w World,
        _state: &'w QueryState<Q, F>,
        table: &'w Table,
        _row: TableRow,
        _col: Option<NonMaxUsize>,
    ) -> Option<Self::Output<'w>> {
        let signature = T::try_get_signature(&world.archetypes.component_registry).unwrap();
        Some(table.signature.contains(&signature))
    }

    fn iter<F: Filter>(
        world: &World,
        table: usize,
        col: Option<NonMaxUsize>,
        _last_tick: u32,
        _current_tick: u32,
    ) -> HasIter<'_> {
        debug_assert!(col.is_none(), "column index passed to `Has` iterator");

        // TODO: This should be stored in some kind of persistent state.
        let signature = T::try_get_signature(&world.archetypes.component_registry).unwrap();
        let table = world.archetypes.get_by_index(table);

        let matches = table.signature.contains(&signature);
        HasIter {
            remaining: table.len(),
            matches,
            _marker: PhantomData,
        }
    }
}

/// Iterator that returns whether the entity has the component.
pub struct HasIter<'t> {
    matches: bool,
    remaining: usize,
    // There is nothing preventing this iterator from outliving the query, but
    // the query iterators are required to not outlive the query for soundness reasons.
    _marker: PhantomData<&'t ()>,
}

impl<'t> EmptyableIterator<'t, bool> for HasIter<'t> {
    fn empty(_world: &'t World) -> HasIter<'t> {
        Self {
            matches: false,
            remaining: 0,
            _marker: PhantomData,
        }
    }
}

impl Iterator for HasIter<'_> {
    type Item = bool;

    #[inline]
    fn next(&mut self) -> Option<bool> {
        if self.remaining == 0 {
            return None;
        }

        self.remaining -= 1;
        Some(self.matches)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for HasIter<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.remaining
    }
}

impl FusedIterator for HasIter<'_> {}
