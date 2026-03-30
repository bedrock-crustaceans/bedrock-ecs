use std::fmt::Debug;
use std::marker::PhantomData;

#[cfg(not(feature = "generics"))]
use smallvec::SmallVec;

use crate::archetype::{Archetypes, Signature};
use crate::component::ComponentBundle;
use crate::table::Changes;

/// The possible methods of filtering used by queries.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FilterMethod {
    /// Coarse filters are used only when updating the query cache (i.e. when the archetype generation is increased
    /// due to a newly added table.) Coarse filters are only able to filter out entire tables and do not support
    /// entity-level filtering.
    ///
    /// The obvious examples of coarse filters are [`With`] and [`Without`], which simply match the table signature to check
    /// whether the table contains the filtered components.
    Coarse,
    /// Dynamic filters are invoked per entity during iteration. Unlike coarse filters, these allow filtering for specific entities
    /// within the table.
    ///
    /// The main use of these types of filters is in the [`Added`] and [`Changed`] filters.
    ///
    /// Dynamic filters can also have an associated coarse filter, in fact most do. The aforementioned [`Added`] and [`Changed`]
    /// traits for example filter out tables that do not contain the components they are targeting.
    Dynamic,
}

impl FilterMethod {
    /// Whether this enum equals [`Dynamic`].
    ///
    /// [`Dynamic`]: FilterMethod::Dynamic.
    ///
    /// This function is separate from the [`PartialEq`] trait to allow filter methods
    /// to be compared at compile time. Traits cannot currently have const functions.
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
        if v { Self::Dynamic } else { Self::Coarse }
    }
}

/// Implements the filtering functionality in queries.
///
/// This allows queries to return only a subset of entities that match some predicate.
///
/// Examples of coarse filters are [`With`], [`Without`] while [`Changed`] and [`Added`] are examples of
/// dynamic filters.
pub trait Filter {
    /// Which filtering method this filter uses.
    ///
    /// Please note that dynamic filters can impact performance since the [`apply_dynamic`]
    /// method must be called for every entity that is iterated over. If the query only uses
    /// coarse filters, this entire check is removed by the Rust compiler.
    ///
    /// See [`FilterMethod`] for more information.
    ///
    /// [`apply_dynamic`]: Filter::apply_dynamic
    const METHOD: FilterMethod;

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

    /// Applies the coarse filter, returning whether the table matches the filter.
    ///
    /// Before a query fetches the requested data, it will cache the tables it intends to access.
    /// These tables are found by performing a bitwise AND of the query signature and table signature.
    /// Coarse filters are only able to discard entire archetype tables, dynamic filters should be used to filter
    /// individual entities within a table.
    ///
    /// If this returns false, the table is ignored, otherwise the table is added to the query cache.
    ///
    /// Examples of coarse filters are [`With`] and [`Without`].
    /// However, nearly all dynamic filters also have a coarse part.
    /// [`Changed`] filters for tables that include its component for example.
    ///
    /// See [`FilterMethod`] for more information.
    fn apply_coarse(&self, archetype: &Signature) -> bool;

    /// Applies dynamic filters. If this function returns `true`, the component will be yielded
    /// by the iterator. Otherwise this component and any other components belonging to the same entity in the query
    /// will be skipped.
    ///
    /// Dynamic filters run during iteration and therefore incur a slight cost.
    ///
    /// See [`FilterMethod`] for more information.
    fn apply_dynamic(changes: Changes, last_tick: u32) -> bool;
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
    const METHOD: FilterMethod = FilterMethod::Coarse;
    const TRIVIAL: bool = true;

    fn init(_archetypes: &mut Archetypes) {}

    fn apply_coarse(&self, _archetype: &Signature) -> bool {
        true
    }

    fn apply_dynamic(_changes: Changes, _last_tick: u32) -> bool {
        true
    }
}

/// A tuple of [`Filter`]s.
///
/// This enables using multiple togethers. A standalone tuple such as `(With<T>, Without<U>)` will perform
/// a logical AND, requiring all filters to match in order to yield the entity.
///
/// This is also used to implement the logical expressions such as [`Not`], [`Or`], [`Xor`], etc.
pub trait FilterBundle {
    /// The filter method required to apply this filter bundle. If _any_ of the filters in the bundle
    /// are dynamic, this will be set to dynamic.
    const METHOD: FilterMethod;

    /// Initializes the state of all filters in this bundle.
    fn init(archetypes: &mut Archetypes) -> Self;

    /// Applies coarse part of all filters in this collection and returns an iterators over the results.
    ///
    /// This iterator is ingested by the logical expressions such as [`Not`], [`Or`], etc. If you simply want to
    /// perform a logical AND, use [`apply_coarse`] instead.
    ///
    /// [`apply_coarse`]: Self::apply_coarse
    fn apply_coarse_iterable(&self, archetype: &Signature) -> impl FilterIterable;

    /// Applies the dynamic filter of all filters in this collection and returns an iterator over the
    /// results.
    ///
    /// This iterator is ingested by the logical expressions such as [`Not`], [`Or`], etc. If you simply want to
    /// perform a logical AND, use [`apply_dynamic`] instead.
    ///
    /// [`apply_dynamic`]: Self::apply_dynamic
    fn apply_dynamic_iterable(changes: Changes, last_tick: u32) -> impl FilterIterable;

    /// Apply all coarse filters and AND them together.
    fn apply_coarse(&self, signature: &Signature) -> bool;

    /// Apply all dynamic filters and AND them together.
    fn apply_dynamic(changes: Changes, last_tick: u32) -> bool;
}

/// Implements [`FilterBundle`] for several sizes of tuples.
macro_rules! impl_filter_bundle {
    ($($gen:ident),*) => {
        #[allow(non_snake_case)] // Because we use `$gen` as a variable name to avoid having to create custom identifiers.
        #[allow(unused_parens)] // Using this macro on a single type will result in `(A)`, this suppresses that warning.
        impl<$($gen:Filter),*> FilterBundle for ($($gen),*) {
            // Set method to dynamic (true) if at least one of the filters in the bundle is dynamic.
            // Otherwise it is set to coarse.
            const METHOD: FilterMethod = FilterMethod::from_bool($($gen::METHOD.to_bool())||*);

            #[inline]
            fn init(archetypes: &mut Archetypes) -> Self {
                // `Self` is a tuple of filters, initialise them all.
                ($($gen::init(archetypes)),*)
            }

            #[inline]
            fn apply_coarse(&self, sig: &Signature) -> bool {
                // Since `self` is a tuple, we destructure it like this...
                let ($($gen),*) = &self;
                // ...and then apply all filters and combine them.
                $($gen.apply_coarse(sig))&&*
            }

            #[inline]
            fn apply_coarse_iterable(&self, sig: &Signature) -> impl FilterIterable {
                // Since `self` is a tuple, we destructure it like this...
                let ($($gen),*) = &self;
                // ...and then return an array of the results.
                [$($gen.apply_coarse(sig)),*]
            }

            #[inline]
            fn apply_dynamic(changes: Changes, last_tick: u32) -> bool {
                // This does not take `self`, so we do not need to destructure.
                // Just call the method on every filter in this bundle.
                $($gen::apply_dynamic(changes, last_tick))&&*
            }

            #[inline]
            fn apply_dynamic_iterable(changes: Changes, last_tick: u32) -> impl FilterIterable {
                // This does not take `self`, so we do not need to destructure.
                // Just call the method on every filter in this bundle and collect into an array.
                [$($gen::apply_dynamic(changes, last_tick)),*]
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
    const METHOD: FilterMethod = B::METHOD;

    #[inline]
    fn init(archetypes: &mut Archetypes) -> Xor<B> {
        Xor(B::init(archetypes))
    }

    #[inline]
    fn apply_dynamic(changes: Changes, last_tick: u32) -> bool {
        if B::METHOD.is_dynamic() {
            // Only apply dynamic filters if at least one of the contained filters is dynamic.
            let out = B::apply_dynamic(changes, last_tick);
            let truthy = out.iter().map(|b| u8::from(b)).sum::<u8>();
            truthy % 2 == 1
        } else {
            // Explicitly return true. This makes it easier for the compiler to see this code can be compiled away,
            // but also prevents the issue where if no filters are dynamic, they will all return true and the xor filter
            // would return false for every single entity it encounters.
            true
        }
    }

    #[inline]
    fn apply_coarse(&self, archetypes: &Signature) -> bool {
        let out = self.0.apply_coarse(archetypes);
        let truthy = out.iter().map(|b| u8::from(b)).sum::<u8>();
        truthy % 2 == 1
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
            // Skip the filter application altogether if there are no dynamic filters.
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
///
/// This filter uses tuple syntax and also supports nesting multiple logical filters.
///
/// If multiple filters are used, such as `Not<(With<T>, With<U>)>` this filter will return
/// `true` as long as at least one of the contained filters returns `false`.
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
            // Skip the filter application altogether if there are no dynamic filters.
            true
        }
    }

    #[inline]
    fn apply_coarse(&self, archetypes: &Signature) -> bool {
        let out = self.0.apply_coarse_iterable(archetypes);
        out.iter().any(|b| !b)
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
        // This filter only filters at the table level.
        true
    }
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
