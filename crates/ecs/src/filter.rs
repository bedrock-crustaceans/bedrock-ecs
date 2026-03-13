use std::marker::PhantomData;
use generic_array::{arr, ArrayLength, GenericArray};
use generic_array::typenum::{FoldAdd, U0, U1, U2, U3, U4, U5, Unsigned};
#[cfg(not(feature = "generics"))]
use smallvec::SmallVec;
use crate::bitset::BitSet;
use crate::{component::Component};
use crate::archetype::Archetypes;
use crate::component::{ComponentId, ComponentRegistry};

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum FilterDesc {
    #[default]
    None,
    With(ComponentId),
    Without(ComponentId),
    Added(ComponentId),
    Changed(ComponentId),
    Removed(ComponentId)
}

pub trait Filter {
    fn init(archetypes: &mut Archetypes) -> Self;

    fn apply_static_filter(&self, archetype: &BitSet) -> bool;
}

pub trait FilterBundle: Sized {
    fn init(archetypes: &mut Archetypes) -> Self;

    fn apply_static_filters(&self, archetype: &BitSet) -> bool;
}

impl FilterBundle for () {
    fn init(_archetypes: &mut Archetypes) -> Self {}

    fn apply_static_filters(&self, _archetype: &BitSet) -> bool {
        true
    }
}

macro_rules! impl_bundle {
    ($count:expr, $($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens)]
            impl<$($gen:Filter),*> FilterBundle for ($($gen),*) {
                fn init(archetypes: &mut Archetypes) -> Self {
                    ($($gen::init(archetypes)),*)
                }

                fn apply_static_filters(&self, archetype: &BitSet) -> bool {
                    #[allow(non_snake_case)]
                    let ($($gen),*) = self;
                    $($gen.apply_static_filter(archetype))&&+
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

pub trait FilterComponentBundle {
    /// Converts this bundle to a bitset to compare against archetype tables.
    fn to_bitset(reg: &mut ComponentRegistry) -> BitSet;
}

macro_rules! impl_filter_bundle {
    ($($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens)]
            impl<$($gen:Component),*> FilterComponentBundle for ($($gen),*) {
                fn to_bitset(reg: &mut ComponentRegistry) -> BitSet {
                    let mut set = BitSet::new();
                    $(
                        let id = reg.get_or_assign::<$gen>();
                        set.set(*id);
                    )*
                    set
                }
            }
        }
    }
}

impl_filter_bundle!(A);
impl_filter_bundle!(A, B);
impl_filter_bundle!(A, B, C);
impl_filter_bundle!(A, B, C, D);
impl_filter_bundle!(A, B, C, D, E);

pub struct With<T: FilterComponentBundle> {
    set: BitSet,
    _marker: PhantomData<T>
}

impl<T: FilterComponentBundle> Filter for With<T> {
    fn init(archetypes: &mut Archetypes) -> Self {
        tracing::trace!("constructing filter state for `{}`", std::any::type_name::<Self>());
        With {
            set: T::to_bitset(&mut archetypes.registry),
            _marker: PhantomData
        }
    }

    fn apply_static_filter(&self, archetype: &BitSet) -> bool {
        archetype.is_subset(&self.set)
    }
}

pub struct Without<T: FilterComponentBundle> {
    set: BitSet,
    _marker: PhantomData<T>
}

impl<T: FilterComponentBundle> Filter for Without<T> {
    fn init(archetypes: &mut Archetypes) -> Self {
        tracing::trace!("constructing filter state for `{}`", std::any::type_name::<Self>());
        Without {
            set: T::to_bitset(&mut archetypes.registry),
            _marker: PhantomData
        }
    }

    fn apply_static_filter(&self, archetype: &BitSet) -> bool {
        archetype.is_disjoint(&self.set)
    }
}

pub struct Added<T: FilterComponentBundle> {
    set: BitSet,
    _marker: PhantomData<T>
}

impl<T: FilterComponentBundle> Filter for Added<T> {
    fn init(archetypes: &mut Archetypes) -> Self {
        tracing::trace!("constructing filter state for `{}`", std::any::type_name::<Self>());
        Added {
            set: T::to_bitset(&mut archetypes.registry),
            _marker: PhantomData
        }
    }

    fn apply_static_filter(&self, archetype: &BitSet) -> bool {
        archetype.is_subset(&self.set)
    }
}
pub struct Removed<T: FilterComponentBundle> {
    set: BitSet,
    _marker: PhantomData<T>
}

impl<T: FilterComponentBundle> Filter for Removed<T> {
    fn init(archetypes: &mut Archetypes) -> Self {
        tracing::trace!("constructing filter state for `{}`", std::any::type_name::<Self>());
        Removed {
            set: T::to_bitset(&mut archetypes.registry),
            _marker: PhantomData
        }
    }

    fn apply_static_filter(&self, archetype: &BitSet) -> bool {
        archetype.is_disjoint(&self.set)
    }
}

pub struct Changed<T: FilterComponentBundle> {
    set: BitSet,
    _marker: PhantomData<T>
}

impl<T: FilterComponentBundle> Filter for Changed<T> {
    fn init(archetypes: &mut Archetypes) -> Self {
        tracing::trace!("constructing filter state for `{}`", std::any::type_name::<Self>());
        Changed {
            set: T::to_bitset(&mut archetypes.registry),
            _marker: PhantomData
        }
    }

    fn apply_static_filter(&self, archetype: &BitSet) -> bool {
        archetype.is_subset(&self.set)
    }
}