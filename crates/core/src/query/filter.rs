use std::fmt::Debug;
use std::marker::PhantomData;
use std::ptr::NonNull;

#[cfg(not(feature = "generics"))]
use smallvec::SmallVec;

use crate::archetype::{Archetypes, Signature};
use crate::component::{ComponentBundle, TrackerFilterImpl};
use crate::prelude::Component;
use crate::table::{ChangeTracker, Changes, Table};

/// Marker trait for archetypal filters
pub trait ArchetypalFilter: Filter {}

/// Implements the filtering functionality in queries.
///
/// This allows queries to return only a subset of entities that match some predicate.
///
/// Examples of archetypal filters are [`With`], [`Without`] while [`Changed`] and [`Added`] are examples of
/// dynamic filters.
pub trait Filter: Send + 'static {
    /// The state for the filter that is stored inside of the query.
    ///
    /// This is used by dynamic filters to keep track of the change columns.
    /// archetypal filters will not use this at all, keeping the iterator bundle structs very small.
    ///
    /// This is important because an older version of the ECS stored all state in the iterator regardless,
    /// which caused performance issues due to register pressure.
    type DynamicState: Send;

    /// Which filtering method this filter uses.
    ///
    /// Please note that dynamic filters can impact performance since the [`apply_dynamic`]
    /// method must be called for every entity that is iterated over. If the query only uses
    /// archetypal filters, this entire check is removed by the Rust compiler.
    ///
    /// [`apply_dynamic`]: Filter::apply_dynamic
    const IS_ARCHETYPAL: bool;

    /// Whether this filter always returns `true`. This is used to detect when the `()` filter is being used in queries,
    /// since specialisation is unstable.
    ///
    /// This should only be set to true for the `()` type.
    // The main use of this currently is specialising the `size_hint` implementation. If the filter is `()` we can return the exact
    // size as lower and upper bound, while for nontrivial filters we give a lower bound of 0 instead.
    const TRIVIAL: bool = false;

    /// Initialises the filter state.
    ///
    /// With most filters this just creates a bitset used to match with the archetype tables.
    /// This state is stored inside of the query state and is persistent across system calls.
    fn init(archetypes: &mut Archetypes) -> Self;

    /// Applies the archetypal filter, returning whether the table matches the filter.
    ///
    /// Before a query fetches the requested data, it will cache the tables it intends to access.
    /// These tables are found by performing a bitwise AND of the query signature and table signature.
    /// archetypal filters are only able to discard entire archetype tables, dynamic filters should be used to filter
    /// individual entities within a table.
    ///
    /// If this returns false, the table is ignored, otherwise the table is added to the query cache.
    ///
    /// Examples of archetypal filters are [`With`] and [`Without`].
    /// However, nearly all dynamic filters also have a archetypal part.
    /// [`Changed`] filters for tables that include its component for example.
    ///
    /// See [`FilterMethod`] for more information.
    fn apply_archetypal(&self, archetype: &Signature) -> bool;

    /// Applies dynamic filters. If this function returns `true`, the component will be yielded
    /// by the iterator. Otherwise this component and any other components belonging to the same entity in the query
    /// will be skipped.
    ///
    /// Dynamic filters run during iteration and therefore incur a slight cost.
    ///
    /// See [`FilterMethod`] for more information.
    fn apply_dynamic(state: &Self::DynamicState, last_run_tick: u32) -> bool;

    /// Creates a new iterator state.
    fn set_dynamic_state(table: &Table) -> Self::DynamicState;

    /// Creates a dangling but still aligned iterator state.
    fn dangling() -> Self::DynamicState;
}

/// A wrapper around `[bool; N]` that provides a method to create an array. This makes it possible for filter
/// bundles to return bool arrays with different sizes while avoiding the currently unstable [`generic_const_exprs`] feature.
///
/// See [`FilterBundle`] for more information.
///
/// [`generic_const_exprs`]: https://github.com/rust-lang/rust/issues/76560
pub trait FilterIterable: Debug {
    /// Yields an iterator over the filter results.
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

/// This is simply an empty filter that matches everything. It is the default filter used by queries.
impl Filter for () {
    type DynamicState = ();

    const IS_ARCHETYPAL: bool = true;
    const TRIVIAL: bool = true;

    #[inline]
    fn init(_archetypes: &mut Archetypes) {}

    #[inline]
    fn apply_archetypal(&self, _archetype: &Signature) -> bool {
        true
    }

    #[inline]
    fn apply_dynamic(_state: &Self::DynamicState, _last_run_tick: u32) -> bool {
        true
    }

    #[inline]
    fn set_dynamic_state(table: &Table) {}

    #[inline]
    fn dangling() -> Self::DynamicState {}
}

/// A tuple of [`Filter`]s.
///
/// This enables using multiple togethers. A standalone tuple such as `(With<T>, Without<U>)` will perform
/// a logical AND, requiring all filters to match in order to yield the entity.
///
/// This is also used to implement the logical expressions such as [`Not`], [`Or`], [`Xor`], etc.
pub trait FilterBundle: Send + 'static {
    type DynamicState: Send;

    /// The filter method required to apply this filter bundle. If _any_ of the filters in the bundle
    /// are dynamic, this will be set to dynamic.
    const IS_ARCHETYPAL: bool;

    /// Initializes the state of all filters in this bundle.
    fn init(archetypes: &mut Archetypes) -> Self;

    /// Applies archetypal part of all filters in this collection and returns an iterators over the results.
    ///
    /// This iterator is ingested by the logical expressions such as [`Not`], [`Or`], etc. If you simply want to
    /// perform a logical AND, use [`apply_archetypal`] instead.
    ///
    /// [`apply_archetypal`]: Self::apply_archetypal
    fn apply_archetypal_iterable(&self, archetype: &Signature) -> impl FilterIterable;

    /// Applies the dynamic filter of all filters in this collection and returns an iterator over the
    /// results.
    ///
    /// This iterator is ingested by the logical expressions such as [`Not`], [`Or`], etc. If you simply want to
    /// perform a logical AND, use [`apply_dynamic`] instead.
    ///
    /// [`apply_dynamic`]: Self::apply_dynamic
    fn apply_dynamic_iterable(
        state: &Self::DynamicState,
        last_run_tick: u32,
    ) -> impl FilterIterable;

    /// Apply all archetypal filters and AND them together.
    fn apply_archetypal(&self, signature: &Signature) -> bool;

    /// Apply all dynamic filters and AND them together.
    fn apply_dynamic(state: &Self::DynamicState, last_run_tick: u32) -> bool;

    fn set_dynamic_state(table: &Table) -> Self::DynamicState;

    fn dangling() -> Self::DynamicState;
}

/// Implements [`FilterBundle`] for several sizes of tuples.
macro_rules! impl_filter_bundle {
    ($($gen:ident),*) => {
        #[allow(non_snake_case)] // Because we use `$gen` as a variable name to avoid having to create custom identifiers.
        #[allow(unused_parens)] // Using this macro on a single type will result in `(A)`, this suppresses that warning.
        impl<$($gen:Filter),*> FilterBundle for ($($gen),*) {
            type DynamicState = ($($gen::DynamicState),*);

            // Set method to dynamic (true) if at least one of the filters in the bundle is dynamic.
            // Otherwise it is set to archetypal.
            const IS_ARCHETYPAL: bool = $($gen::IS_ARCHETYPAL)&&*;

            #[inline]
            fn init(archetypes: &mut Archetypes) -> Self {
                // `Self` is a tuple of filters, initialise them all.
                ($($gen::init(archetypes)),*)
            }

            #[inline]
            fn apply_archetypal(&self, sig: &Signature) -> bool {
                // Since `self` is a tuple, we destructure it like this...
                let ($($gen),*) = &self;
                // ...and then apply all filters and combine them.
                $($gen.apply_archetypal(sig))&&*
            }

            #[inline]
            fn apply_archetypal_iterable(&self, sig: &Signature) -> impl FilterIterable {
                // Since `self` is a tuple, we destructure it like this...
                let ($($gen),*) = &self;
                // ...and then return an array of the results.
                [$($gen.apply_archetypal(sig)),*]
            }

            #[inline]
            fn apply_dynamic(($($gen),*): &Self::DynamicState, last_run_tick: u32) -> bool {
                // This does not take `self`, so we do not need to destructure.
                // Just call the method on every filter in this bundle.
                $($gen::apply_dynamic($gen, last_run_tick))&&*
            }

            #[inline]
            fn apply_dynamic_iterable(($($gen),*): &Self::DynamicState, last_run_tick: u32) -> impl FilterIterable {
                // This does not take `self`, so we do not need to destructure.
                // Just call the method on every filter in this bundle and collect into an array.
                [$($gen::apply_dynamic($gen, last_run_tick)),*]
            }

            #[inline]
            fn set_dynamic_state(table: &Table) -> Self::DynamicState {
                ($($gen::set_dynamic_state(table)),*)
            }

            #[inline]
            fn dangling() -> Self::DynamicState {
                ($($gen::dangling()),*)
            }
        }
    }
}

// Implement [`FilterBundle`] for several tuple sizes.
impl_filter_bundle!(A);
impl_filter_bundle!(A, B);
impl_filter_bundle!(A, B, C);
impl_filter_bundle!(A, B, C, D);
impl_filter_bundle!(A, B, C, D, E);

/// Performs an exclusive OR on all filters.
///
/// Filters can be inserted into this expression using tuple syntax. For example,
/// `Xor<With<T>>`, or `Xor<(With<T>, Without<U>)>`. Nested logical expressions are also permitted
/// this makes it possible to make more complicated filters.
///
/// If it contains more than two filters, it will match if and only if an odd
/// number of filters match.
#[repr(transparent)]
pub struct Xor<B: FilterBundle>(B);

impl<B: FilterBundle> Filter for Xor<B> {
    type DynamicState = B::DynamicState;

    const IS_ARCHETYPAL: bool = B::IS_ARCHETYPAL;

    #[inline]
    fn init(archetypes: &mut Archetypes) -> Xor<B> {
        Xor(B::init(archetypes))
    }

    #[inline]
    fn apply_dynamic(state: &Self::DynamicState, last_run_tick: u32) -> bool {
        if B::IS_ARCHETYPAL {
            true
        } else {
            // Only apply dynamic filters if at least one of the contained filters is dynamic.
            let out = B::apply_dynamic(state, last_run_tick);
            let truthy = out.iter().map(|b| u8::from(b)).sum::<u8>();
            truthy % 2 == 1
        }
    }

    #[inline]
    fn apply_archetypal(&self, archetypes: &Signature) -> bool {
        let out = self.0.apply_archetypal(archetypes);
        let truthy = out.iter().map(|b| u8::from(b)).sum::<u8>();
        truthy % 2 == 1
    }

    #[inline]
    fn set_dynamic_state(table: &Table) -> Self::DynamicState {
        B::set_dynamic_state(table)
    }

    #[inline]
    fn dangling() -> Self::DynamicState {
        B::dangling()
    }
}

/// Performs a logical OR on the contained filters. In other words, this filter
/// will always return `true` if at least one of the contained filters does.
///
/// Filters can be inserted into this expression using tuple syntax. For example,
/// `Or<With<T>>`, or `Or<(With<T>, Without<U>)>`. Nested logical expressions are also permitted
/// this makes it possible to make more complicated filters.
#[repr(transparent)]
pub struct Or<B: FilterBundle>(B);

impl<B: FilterBundle> Filter for Or<B> {
    type DynamicState = B::DynamicState;

    const IS_ARCHETYPAL: bool = B::IS_ARCHETYPAL;

    #[inline]
    fn init(archetypes: &mut Archetypes) -> Or<B> {
        Or(B::init(archetypes))
    }

    #[inline]
    fn apply_dynamic(state: &Self::DynamicState, last_run_tick: u32) -> bool {
        if B::IS_ARCHETYPAL {
            true
        } else {
            let out = B::apply_dynamic_iterable(state, last_run_tick);
            out.iter().any(|b| b)
        }
    }

    #[inline]
    fn apply_archetypal(&self, archetypes: &Signature) -> bool {
        let out = self.0.apply_archetypal_iterable(archetypes);
        out.iter().any(|b| b)
    }

    #[inline]
    fn set_dynamic_state(table: &Table) -> Self::DynamicState {
        B::set_dynamic_state(table)
    }

    #[inline]
    fn dangling() -> Self::DynamicState {
        B::dangling()
    }
}

/// Inverts the filter. For example `Not<With<T>>` is equivalent to `Without<T>`.
///
/// This filter uses tuple syntax and also supports nesting multiple logical filters.
///
/// If multiple filters are used, such as `Not<(With<T>, With<U>)>` this filter will return
/// `true` as long as at least one of the contained filters returns `false`.
#[repr(transparent)]
pub struct Not<B>(B);

impl<B: FilterBundle> Filter for Not<B> {
    type DynamicState = B::DynamicState;

    const IS_ARCHETYPAL: bool = B::IS_ARCHETYPAL;

    #[inline]
    fn init(archetypes: &mut Archetypes) -> Not<B> {
        Not(B::init(archetypes))
    }

    #[inline]
    fn apply_dynamic(state: &Self::DynamicState, last_run_tick: u32) -> bool {
        if B::IS_ARCHETYPAL {
            true
        } else {
            let out = B::apply_dynamic_iterable(state, last_run_tick);
            out.iter().any(|b| !b)
        }
    }

    #[inline]
    fn apply_archetypal(&self, archetypes: &Signature) -> bool {
        let out = self.0.apply_archetypal_iterable(archetypes);
        out.iter().any(|b| !b)
    }

    #[inline]
    fn set_dynamic_state(table: &Table) -> Self::DynamicState {
        B::set_dynamic_state(table)
    }

    #[inline]
    fn dangling() -> Self::DynamicState {
        B::dangling()
    }
}

/// Matches only entity that have the component `T`, without actually returning the component.
/// Since this filter only checks for existence of `T` and does not access it, it is able to run in parallel
/// with systems that do need access to `T`. Prefer using this filter over requesting the components in the data
/// section and then not using them.
///
/// This filter uses tuple syntax and can match multiple components at a time, such as `With<(A, B)>`.
///
/// Merging multiple components into a single `With` statement should be preferred over two separate ones since
/// it can be optimised into a single comparison for two components at once, while two separate statements will always
/// require two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct With<T: ComponentBundle> {
    /// The bits of the components that the archetype table should have.
    signature: Signature,
    _marker: PhantomData<T>,
}

impl<T: ComponentBundle> Filter for With<T> {
    type DynamicState = ();

    const IS_ARCHETYPAL: bool = true;

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
    fn apply_archetypal(&self, sig: &Signature) -> bool {
        sig.contains(&self.signature)
    }

    fn apply_dynamic(_state: &Self::DynamicState, _last_run_tick: u32) -> bool {
        true
    }

    #[inline]
    fn set_dynamic_state(_table: &Table) {}

    #[inline]
    fn dangling() {}
}

/// Matches only entities that do not have the component `T`.
///
/// This filter uses tuple syntax and can match multiple components at a time, such as `With<(A, B)>`.
///
/// Merging multiple components into a single `With` statement should be preferred over two separate ones since
/// it can be optimised into a single comparison for two components at once, while two separate statements will always
/// require two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Without<T: ComponentBundle> {
    /// The bits of the components that the archetype table should not have.
    signature: Signature,
    _marker: PhantomData<T>,
}

impl<T: ComponentBundle> Filter for Without<T> {
    type DynamicState = ();

    const IS_ARCHETYPAL: bool = true;

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

    fn apply_archetypal(&self, sig: &Signature) -> bool {
        sig.is_disjoint(&self.signature)
    }

    fn apply_dynamic(_state: &Self::DynamicState, _last_run_tick: u32) -> bool {
        true
    }

    #[inline]
    fn set_dynamic_state(_table: &Table) {}

    #[inline]
    fn dangling() {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added<T: Component> {
    signature: Signature,
    _marker: PhantomData<T>,
}

impl<T: Component> Filter for Added<T> {
    /// A base pointer to the start of the change tracker.
    type DynamicState = <T as ComponentBundle>::TrackerPtrs;

    const IS_ARCHETYPAL: bool = false;

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
    fn apply_archetypal(&self, sig: &Signature) -> bool {
        sig.contains(&self.signature)
    }

    #[inline]
    fn apply_dynamic(state: &Self::DynamicState, last_run_tick: u32) -> bool {
        state.filter::<T>(last_run_tick)
    }

    #[inline]
    fn set_dynamic_state(table: &Table) -> Self::DynamicState {
        T::get_added_tracker_ptrs(table)
    }

    #[inline]
    fn dangling() -> Self::DynamicState {
        T::dangling_tracker_ptrs()
    }
}

/// Queries using a `Changed` filter will always return everything the first time the system runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed<T: Component> {
    signature: Signature,
    _marker: PhantomData<T>,
}

impl<T: Component> Filter for Changed<T> {
    /// A base pointer to the start of the change tracker.
    type DynamicState = <T as ComponentBundle>::TrackerPtrs;

    const IS_ARCHETYPAL: bool = false;

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
    fn apply_archetypal(&self, sig: &Signature) -> bool {
        sig.contains(&self.signature)
    }

    #[inline]
    fn apply_dynamic(state: &Self::DynamicState, last_run_tick: u32) -> bool {
        state.filter::<T>(last_run_tick)
    }

    #[inline]
    fn set_dynamic_state(table: &Table) -> Self::DynamicState {
        T::get_changed_tracker_ptrs(table)
    }

    #[inline]
    fn dangling() -> Self::DynamicState {
        T::dangling_tracker_ptrs()
    }
}
