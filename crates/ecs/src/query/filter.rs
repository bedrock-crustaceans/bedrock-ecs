use std::marker::PhantomData;

#[cfg(not(feature = "generics"))]
use smallvec::SmallVec;

use crate::archetype::{Archetypes, Signature};
use crate::component::ComponentBundle;
use crate::table::ChangeTracker;

/// Implements the filtering functionality in queries.
pub trait Filter {
    /// Initialises the filter state.
    ///
    /// With most filters this just creates a bitset used to match with the archetype tables.
    fn init(archetypes: &mut Archetypes) -> Self;

    /// Applies the static filter, returning whether the table should be accepted.
    ///
    /// Before a query fetches the requested data, it will cache the tables it intends to access.
    /// These tables are found by performing a bitwise and of the query bitset and archetype bitset.
    ///
    /// If this fails, the table is ignored, otherwise we continue to the filtering stage.
    /// This is when the static filters are applied, these are filters that can be applied to whole archetype
    /// tables without needing any tick-specific or entity-specific information.
    ///
    /// Examples of fully static filters are [`With`] and [`Without`]. Other dynamic filters also have
    /// a static part. The [`Changed`] filter statically filters for tables that include its component for example.
    /// This does not require any runtime info.
    fn apply_static_filter(&self, archetype: &Signature) -> bool;

    fn apply_dynamic_filter(tracker: &ChangeTracker, current_tick: u32) -> bool;
}

/// A collection of filters.
pub trait FilterBundle: Sized {
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

    fn apply_dynamic_filters(_tracker: &ChangeTracker, current_tick: u32) -> bool;
}

impl FilterBundle for () {
    const LEN: usize = 0;

    #[inline]
    fn init(_archetypes: &mut Archetypes) -> Self {}

    /// The empty filter always returns true.
    #[inline]
    fn apply_static_filters(&self, _archetype: &Signature) -> bool {
        true
    }

    #[inline]
    fn apply_dynamic_filters(_tracker: &ChangeTracker, _current_tick: u32) -> bool {
        true
    }
}

macro_rules! impl_bundle {
    ($count:expr, $($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens)]
            impl<$($gen:Filter),*> FilterBundle for ($($gen),*) {
                const LEN: usize = $count;

                #[inline]
                fn init(archetypes: &mut Archetypes) -> Self {
                    ($($gen::init(archetypes)),*)
                }

                #[inline]
                fn apply_static_filters(&self, archetype: &Signature) -> bool {
                    #[allow(non_snake_case)]
                    let ($($gen),*) = self;
                    $($gen.apply_static_filter(archetype))&&+
                }

                #[inline]
                fn apply_dynamic_filters(tracker: &ChangeTracker, current_tick: u32) -> bool {
                    $($gen::apply_dynamic_filter(tracker, current_tick))&&+
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
    fn apply_static_filter(&self, sig: &Signature) -> bool {
        sig.contains(&self.signature)
    }

    #[inline]
    fn apply_dynamic_filter(_tracker: &ChangeTracker, _current_tick: u32) -> bool {
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

    fn apply_static_filter(&self, sig: &Signature) -> bool {
        sig.is_disjoint(&self.signature)
    }

    #[inline]
    fn apply_dynamic_filter(_tracker: &ChangeTracker, _current_tick: u32) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added<T: ComponentBundle> {
    signature: Signature,
    _marker: PhantomData<T>,
}

impl<T: ComponentBundle> Filter for Added<T> {
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
    fn apply_static_filter(&self, sig: &Signature) -> bool {
        sig.contains(&self.signature)
    }

    #[inline]
    fn apply_dynamic_filter(tracker: &ChangeTracker, id: usize, last_ran: u32) -> bool {
        unsafe { *tracker.added[id].get() } >= last_ran
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed<T: ComponentBundle> {
    signature: Signature,
    _marker: PhantomData<T>,
}

impl<T: ComponentBundle> Filter for Changed<T> {
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
    fn apply_static_filter(&self, sig: &Signature) -> bool {
        sig.contains(&self.signature)
    }

    #[inline]
    fn apply_dynamic_filter(tracker: &ChangeTracker, current_tick: u32) -> bool {}
}
