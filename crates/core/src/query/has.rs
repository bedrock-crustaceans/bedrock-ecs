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
    query::{Filter, QueryBundle, QueryData, QueryState, QueryType, RandomAccessArray, TableCache},
    scheduler::{AccessDesc, AccessType},
    table::{ColumnRow, Table},
    world::World,
};

/// Determines whether the entity has the given component.
///
/// Unlike the [`With`] filter, the query will also return entities that do not have
/// this component.
///
/// It can be added to the data section of the query like so: `Query<(Entity, Has<Player>)>`. This example
/// query would return a tuple `(Entity, bool)` where the boolean is `true` if the entity has the `Player` tag
/// and `false` otherwise.
///
/// # Access
///
/// This type does not require access to any data and can therefore run in parallel with any other system.
///
/// [`With`]: crate::query::With
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
            mutable: false,
        }
    }

    fn component_id(_reg: &mut ComponentRegistry) -> ComponentId {
        unimplemented!()
    }

    fn map_column(_table: &Table) -> NonMaxUsize {
        unimplemented!()
    }

    fn get<'w, Q: QueryBundle, F: Filter>(
        world: &'w World,
        _state: &'w QueryState<Q, F>,
        table: &'w Table,
        _row: ColumnRow,
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
            matches,
            len: table.width(),
            _marker: PhantomData,
        }
    }
}

/// Iterator that returns whether the entity has the component.
pub struct HasIter<'t> {
    /// While this iterator does not actually fetch any data from tables,
    /// it still needs to know when the table ends to jump to the next one.
    len: usize,
    /// Whether the current table has the component. This is computed once when the iterator
    /// is created.
    matches: bool,
    // There is nothing preventing this iterator from outliving the query, but
    // the query iterators are required to not outlive the query for soundness reasons.
    _marker: PhantomData<&'t ()>,
}

impl RandomAccessArray for HasIter<'_> {
    type Item = bool;

    #[inline]
    unsafe fn get_unchecked(&self, index: usize) -> Self::Item {
        self.matches
    }

    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}
