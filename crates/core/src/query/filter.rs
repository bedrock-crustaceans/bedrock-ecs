use std::fmt::Debug;
use std::marker::PhantomData;

#[cfg(not(feature = "generics"))]
use smallvec::SmallVec;

use crate::archetype::{Archetypes, Signature};
use crate::component::ComponentBundle;
use crate::table::{ChangeTracker, Changes};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FilterMethod {
    Coarse,
    Dynamic,
}

impl FilterMethod {
    #[inline]
    pub const fn is_dynamic(&self) -> bool {
        self.to_bool()
    }

    /// Convert the filter method to a bool.
    ///
    /// This is not implemented using [`Into`] since traits cannot have const functions.
    #[inline]
    pub const fn to_bool(self) -> bool {
        // `PartialEq` is not const so we cannot use regular comparison
        matches!(self, Self::Dynamic)
    }

    /// Converts a bool into a filter method.
    ///
    /// This is not implement using [`From`] since traits cannot have const functions.
    #[inline]
    pub const fn from_bool(v: bool) -> Self {
        match v {
            false => Self::Coarse,
            true => Self::Dynamic,
        }
    }
}

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
    const METHOD: FilterMethod;

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
    fn apply_coarse(&self, archetype: &Signature) -> bool;

    /// Applies dynamic filters. If this function returns `true`, the component will be yielded
    /// by the iterator. Otherwise it will be skipped.
    ///
    /// Dynamic filters run during iteration and therefore incur a slight cost.
    fn apply_dynamic(changes: Changes, last_tick: u32) -> bool;
}

/// A wrapper around `[bool; N]` that provides a method to create an array. This is basically a
/// type system way to work around generic const expressions.
pub trait FilterIterable: Debug {
    /// Gives an iterator over the results from the filters.
    fn iter(self) -> impl Iterator<Item = bool>;
}

impl FilterIterable for bool {
    #[inline]
    fn iter(self) -> impl Iterator<Item = bool> {
        std::iter::once(self)
    }
}

impl<const N: usize> FilterIterable for [bool; N] {
    #[inline]
    fn iter(self) -> impl Iterator<Item = bool> {
        <Self as IntoIterator>::into_iter(self)
    }
}

impl Filter for () {
    const METHOD: FilterMethod = FilterMethod::Coarse;

    fn init(_archetypes: &mut Archetypes) {}

    fn apply_coarse(&self, _archetype: &Signature) -> bool {
        true
    }

    fn apply_dynamic(_changes: Changes, _last_tick: u32) -> bool {
        true
    }
}

/// Consumes a filter bundle to produce a single result.
///
/// This is used to implement the logical expressions such as [`Not`], [`Any`], etc.
pub trait FilterBundle {
    const METHOD: FilterMethod;

    /// Initializes the state of the filters in this aggregator.
    fn init(archetypes: &mut Archetypes) -> Self;

    /// Applies the static filter of all filters in this collection and returns an iterators over the results.
    ///
    /// See [`Filter::apply_static_filter`] for more information about static filters.
    fn apply_coarse_iterable(&self, archetype: &Signature) -> impl FilterIterable;

    /// Applies the dynamic filter of all filters in this collection and returns an iterator over the
    /// results.
    fn apply_dynamic_iterable(changes: Changes, last_tick: u32) -> impl FilterIterable;

    /// Apply all coarse filters and perform this aggregator's action.
    fn apply_coarse(&self, signature: &Signature) -> bool;

    /// Apply all dynamic filters and perform this aggregator's action.
    fn apply_dynamic(changes: Changes, last_tick: u32) -> bool;
}

macro_rules! impl_filter_bundle {
    ($($gen:ident),*) => {
        #[allow(non_snake_case)]
        #[allow(unused_parens)]
        impl<$($gen:Filter),*> FilterBundle for ($($gen),*) {
            const METHOD: FilterMethod = FilterMethod::from_bool($($gen::METHOD.to_bool())||*);

            #[inline]
            fn init(archetypes: &mut Archetypes) -> Self {
                ($($gen::init(archetypes)),*)
            }

            #[inline]
            fn apply_coarse(&self, sig: &Signature) -> bool {
                let ($($gen),*) = &self;
                $($gen.apply_coarse(sig))&&*
            }

            #[inline]
            fn apply_coarse_iterable(&self, sig: &Signature) -> impl FilterIterable {
                let ($($gen),*) = &self;
                [$($gen.apply_coarse(sig)),*]
            }

            #[inline]
            fn apply_dynamic(changes: Changes, last_tick: u32) -> bool {
                $($gen::apply_dynamic(changes, last_tick))&&*
            }

            #[inline]
            fn apply_dynamic_iterable(changes: Changes, last_tick: u32) -> impl FilterIterable {
                [$($gen::apply_dynamic(changes, last_tick)),*]
            }
        }
    }
}

impl_filter_bundle!(A);
impl_filter_bundle!(A, B);
impl_filter_bundle!(A, B, C);
impl_filter_bundle!(A, B, C, D);
impl_filter_bundle!(A, B, C, D, E);

/// Performs an exclusive OR on all filters.
///
/// It is extended to an arbitrary number of filters by returning true if and only if an odd number of
/// filters returned true.
#[repr(transparent)]
pub struct Xor<B: FilterBundle>(B);

impl<B: FilterBundle> Filter for Xor<B> {
    const METHOD: FilterMethod = B::METHOD;

    #[inline]
    fn init(archetypes: &mut Archetypes) -> Xor<B> {
        Xor(B::init(archetypes))
    }

    #[inline]
    fn apply_dynamic(changes: Changes, last_tick: u32) -> bool {
        if B::METHOD.is_dynamic() {
            let out = B::apply_dynamic(changes, last_tick);
            let truthy = out.iter().map(|b| b as u8).sum::<u8>();
            truthy % 2 == 1
        } else {
            true
        }
    }

    #[inline]
    fn apply_coarse(&self, archetypes: &Signature) -> bool {
        let out = self.0.apply_coarse(archetypes);
        let truthy = out.iter().map(|b| b as u8).sum::<u8>();
        truthy % 2 == 1
    }
}

#[repr(transparent)]
pub struct Or<B: FilterBundle>(B);

impl<B: FilterBundle> Filter for Or<B> {
    const METHOD: FilterMethod = B::METHOD;

    #[inline]
    fn init(archetypes: &mut Archetypes) -> Or<B> {
        Or(B::init(archetypes))
    }

    #[inline]
    fn apply_dynamic(changes: Changes, last_tick: u32) -> bool {
        if B::METHOD.is_dynamic() {
            let out = B::apply_dynamic_iterable(changes, last_tick);
            out.iter().any(|b| b)
        } else {
            true
        }
    }

    #[inline]
    fn apply_coarse(&self, archetypes: &Signature) -> bool {
        let out = self.0.apply_coarse_iterable(archetypes);
        out.iter().any(|b| b)
    }
}

/// Inverts the filter. For example `Not<With<T>>` is equivalent to `Without<T>`.
#[repr(transparent)]
pub struct Not<B>(B);

impl<B: FilterBundle> Filter for Not<B> {
    const METHOD: FilterMethod = B::METHOD;

    #[inline]
    fn init(archetypes: &mut Archetypes) -> Not<B> {
        Not(B::init(archetypes))
    }

    #[inline]
    fn apply_dynamic(changes: Changes, last_tick: u32) -> bool {
        if B::METHOD.is_dynamic() {
            let out = B::apply_dynamic_iterable(changes, last_tick);
            out.iter().any(|b| !b)
        } else {
            true
        }
    }

    #[inline]
    fn apply_coarse(&self, archetypes: &Signature) -> bool {
        let out = self.0.apply_coarse_iterable(archetypes);
        out.iter().any(|b| !b)
    }
}

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
    const METHOD: FilterMethod = FilterMethod::Coarse;

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
    fn apply_coarse(&self, sig: &Signature) -> bool {
        sig.contains(&self.signature)
    }

    #[inline]
    fn apply_dynamic(_changes: Changes, _last_tick: u32) -> bool {
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
    const METHOD: FilterMethod = FilterMethod::Coarse;

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

    fn apply_coarse(&self, sig: &Signature) -> bool {
        sig.is_disjoint(&self.signature)
    }

    #[inline]
    fn apply_dynamic(_changes: Changes, _last_tick: u32) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added<T: ComponentBundle> {
    signature: Signature,
    _marker: PhantomData<T>,
}

impl<T: ComponentBundle> Filter for Added<T> {
    const METHOD: FilterMethod = FilterMethod::Dynamic;

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
    fn apply_coarse(&self, sig: &Signature) -> bool {
        sig.contains(&self.signature)
    }

    #[inline]
    fn apply_dynamic(changes: Changes, current_tick: u32) -> bool {
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
    const METHOD: FilterMethod = FilterMethod::Dynamic;

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
    fn apply_coarse(&self, sig: &Signature) -> bool {
        sig.contains(&self.signature)
    }

    #[inline]
    fn apply_dynamic(changes: Changes, current_tick: u32) -> bool {
        changes.changed_tick >= current_tick
    }
}
