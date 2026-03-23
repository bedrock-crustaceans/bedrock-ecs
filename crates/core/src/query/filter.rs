use std::fmt::Debug;
use std::marker::PhantomData;

#[cfg(not(feature = "generics"))]
use smallvec::SmallVec;

use crate::archetype::{Archetypes, Signature};
use crate::component::ComponentBundle;
use crate::table::{ChangeTracker, Changes};

/// Implements the filtering functionality in queries.
///
/// This allows queries to return only a subset of entities that match some predicate.
///
/// Examples of coarse filters are [`With`], [`Without`] while [`Changed`] and [`Added`] are examples of
/// dynamic filters.
pub trait Filter {
    /// Whether this filter uses "dynamic" filtering. Dynamic filtering is used to filter components
    /// within the table itself, while coarse filters will only filter based on table signature
    /// and do not perform any filtering during iteration.
    ///
    /// Dynamic filters usually have a coarse part as well, that filters entire tables.
    const IS_DYNAMIC: bool;

    /// Initialises the filter state.
    ///
    /// With most filters this just creates a bitset used to match with the archetype tables.
    fn init(archetypes: &mut Archetypes) -> Self;

    /// Applies the coarse filter, returning whether the table should be accepted.
    ///
    /// Before a query fetches the requested data, it will cache the tables it intends to access.
    /// These tables are found by performing a bitwise and of the query bitset and archetype bitset.
    /// Coarse filters are only able to discard entire archetype tables, fine filters should be used to filter
    /// individual components within a table.
    ///
    /// If this returns false, the table is ignored, otherwise the table is added to the query cache.
    ///
    /// Examples of coarse filters are [`With`] and [`Without`].
    /// However, nearly all dynamic filters also have a coarse part.
    /// [`Changed`] filters for tables that include its component for example.
    fn apply_coarse_filter(&self, archetype: &Signature) -> bool;

    /// Applies dynamic filters. If this function returns `true`, the component will be yielded
    /// by the iterator. Otherwise it will be skipped.
    ///
    /// Dynamic filters run during iteration and therefore incur a slight cost.
    fn apply_dynamic_filter(changes: Changes, last_tick: u32) -> bool;
}

/// A collection of filters.
pub trait FilterBundle: Sized {
    /// Whether any of the filters in this bundle are dynamic. This means the query iterators
    /// must switch to dynamic filtering during iteration.
    const IS_DYNAMIC: bool;

    /// The amount of filters contained in this collection.
    const LEN: usize;

    /// Initialises the filter state of all filters in this collection.
    ///
    /// With most filters this just creates a bitset used to match with the archetype tables.
    fn init(archetypes: &mut Archetypes) -> Self;

    /// Applies the static filter of all filters in this collection.
    ///
    /// See [`Filter::apply_static_filter`] for more information about static filters.
    fn apply_static_filters(&self, archetype: &Signature) -> bool;

    /// Applies the dynamic filter of all filters in this collection.
    fn apply_dynamic_filters(changes: Changes, last_tick: u32) -> bool;
}

impl FilterBundle for () {
    const IS_DYNAMIC: bool = false;
    const LEN: usize = 0;

    #[inline]
    fn init(_archetypes: &mut Archetypes) -> Self {}

    /// The empty filter always returns true.
    #[inline]
    fn apply_static_filters(&self, _archetype: &Signature) -> bool {
        true
    }

    #[inline]
    fn apply_dynamic_filters(_changes: Changes, _last_tick: u32) -> bool {
        true
    }
}

macro_rules! impl_bundle {
    ($count:expr, $($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens)]
            impl<$($gen:Filter),*> FilterBundle for ($($gen),*) {
                const IS_DYNAMIC: bool = $($gen::IS_DYNAMIC)&&*;
                const LEN: usize = $count;

                #[inline]
                fn init(archetypes: &mut Archetypes) -> Self {
                    ($($gen::init(archetypes)),*)
                }

                #[inline]
                fn apply_static_filters(&self, archetype: &Signature) -> bool {
                    #[allow(non_snake_case)]
                    let ($($gen),*) = self;
                    $($gen.apply_coarse_filter(archetype))&&+
                }

                #[inline]
                fn apply_dynamic_filters(changes: Changes, last_tick: u32) -> bool {
                    $($gen::apply_dynamic_filter(changes, last_tick))&&+
                }
            }
        }
    }
}

impl_bundle!(1, A);
impl_bundle!(2, A, B);
impl_bundle!(3, A, B, C);
impl_bundle!(4, A, B, C, D);
impl_bundle!(5, A, B, C, D, E);

/// Filters out all entities that do not have component `T`.
///
/// Multiple components can also be used to filter for multiple components, i.e. `With<(Health, Transform)>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct With<T: ComponentBundle> {
    /// The bits of the components that the archetype table should have.
    signature: Signature,
    _marker: PhantomData<T>,
}

impl<T: ComponentBundle> Filter for With<T> {
    const IS_DYNAMIC: bool = false;

    fn init(archetypes: &mut Archetypes) -> Self {
        tracing::trace!(
            "constructing filter signature for `{}`",
            std::any::type_name::<Self>()
        );
        With {
            signature: T::get_or_assign_signature(&mut archetypes.component_registry),
            _marker: PhantomData,
        }
    }

    #[inline]
    fn apply_coarse_filter(&self, sig: &Signature) -> bool {
        sig.contains(&self.signature)
    }

    #[inline]
    fn apply_dynamic_filter(_changes: Changes, _last_tick: u32) -> bool {
        true
    }
}

/// Filters out all entities that have component `T`.
///
/// Multiple components can also be used to filter for multiple components, i.e. `With<(Health, Transform)>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Without<T: ComponentBundle> {
    /// The bits of the components that the archetype table should not have.
    signature: Signature,
    _marker: PhantomData<T>,
}

impl<T: ComponentBundle> Filter for Without<T> {
    const IS_DYNAMIC: bool = false;

    fn init(archetypes: &mut Archetypes) -> Self {
        tracing::trace!(
            "constructing filter signature for `{}`",
            std::any::type_name::<Self>()
        );
        Without {
            signature: T::get_or_assign_signature(&mut archetypes.component_registry),
            _marker: PhantomData,
        }
    }

    fn apply_coarse_filter(&self, sig: &Signature) -> bool {
        sig.is_disjoint(&self.signature)
    }

    #[inline]
    fn apply_dynamic_filter(_changes: Changes, _last_tick: u32) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added<T: ComponentBundle> {
    signature: Signature,
    _marker: PhantomData<T>,
}

impl<T: ComponentBundle> Filter for Added<T> {
    const IS_DYNAMIC: bool = true;

    fn init(archetypes: &mut Archetypes) -> Self {
        tracing::trace!(
            "constructing filter signature for `{}`",
            std::any::type_name::<Self>()
        );
        Added {
            signature: T::get_or_assign_signature(&mut archetypes.component_registry),
            _marker: PhantomData,
        }
    }

    #[inline]
    fn apply_coarse_filter(&self, sig: &Signature) -> bool {
        sig.contains(&self.signature)
    }

    #[inline]
    fn apply_dynamic_filter(changes: Changes, current_tick: u32) -> bool {
        changes.added_tick >= current_tick
    }
}

/// Queries using a `Changed` filter will always return everything the first time the system runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed<T: ComponentBundle> {
    signature: Signature,
    _marker: PhantomData<T>,
}

impl<T: ComponentBundle> Filter for Changed<T> {
    const IS_DYNAMIC: bool = true;

    fn init(archetypes: &mut Archetypes) -> Self {
        tracing::trace!(
            "constructing filter signature for `{}`",
            std::any::type_name::<Self>()
        );
        Changed {
            signature: T::get_or_assign_signature(&mut archetypes.component_registry),
            _marker: PhantomData,
        }
    }

    #[inline]
    fn apply_coarse_filter(&self, sig: &Signature) -> bool {
        sig.contains(&self.signature)
    }

    #[inline]
    fn apply_dynamic_filter(changes: Changes, current_tick: u32) -> bool {
        changes.changed_tick >= current_tick
    }
}
