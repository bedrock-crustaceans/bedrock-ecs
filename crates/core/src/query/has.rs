use std::{
    any::TypeId,
    iter::{Cycle, FusedIterator, Once},
    marker::PhantomData,
};

use nonmax::NonMaxUsize;
use rustc_hash::FxHashMap;

use crate::{
    archetype::Signature,
    component::{Component, ComponentId, TypeRegistry},
    prelude::ComponentBundle,
    query::{Filter, QueryData, QueryGroup, QueryState, QueryType, TableCache},
    scheduler::{AccessDesc, AccessType},
    table::{ColumnRow, Table},
    world::World,
};

/// Determines whether the entity has the given components.
///
/// Unlike [`With`], this item provides no filtering and simply returns a boolean indicating whether
/// the current entity has the given components. Consider using [`With`] instead if you do not care
/// about the entities that do not own such components.
///
/// Note that this since this is a query data item it belongs to the data section of the query, not the filters.
/// Checking multiple components is also supported with tuple syntax like so: `Has<(A, B, C)>`.
///
/// # Access
///
/// This type does not require access to any data other than table metadata (which can only be modified at sync points),
/// and can therefore run in parallel with any other system.
///
/// [`With`]: crate::query::With
pub struct Has<T: ComponentBundle> {
    _marker: PhantomData<T>,
}

unsafe impl<T: ComponentBundle> QueryData for Has<T> {
    type Deref = Has<T>;
    type Output<'w> = bool;
    type CurrPtr = bool;

    const TY: QueryType = QueryType::Has;

    fn access(_reg: &mut TypeRegistry) -> AccessDesc {
        AccessDesc {
            ty: AccessType::None,
            mutable: false,
        }
    }

    fn get_base_ptr(table: &Table) -> Self::CurrPtr {
        todo!()
    }

    #[inline]
    fn dangling() -> Self::CurrPtr {
        false
    }

    #[inline]
    unsafe fn offset_ptr(base: Self::CurrPtr, _n: isize) -> Self::CurrPtr {
        base
    }

    #[inline]
    unsafe fn fetch_relative<'w>(
        base: Self::CurrPtr,
        _offset: isize,
        _current_tick: u32,
    ) -> Self::Output<'w> {
        base
    }

    fn get<'w, Q: QueryGroup, F: Filter>(
        world: &'w World,
        _state: &'w QueryState<Q, F>,
        table: &'w Table,
        _row: ColumnRow,
        _col: Option<NonMaxUsize>,
    ) -> Option<Self::Output<'w>> {
        let signature = T::try_get_signature(&world.archetypes.component_registry).unwrap();
        Some(table.signature.contains(&signature))
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
