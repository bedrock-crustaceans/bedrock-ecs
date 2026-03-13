use std::marker::PhantomData;
use generic_array::{arr, ArrayLength, GenericArray};
use generic_array::typenum::{Unsigned, U0, U1, U2, U3, U4, U5};
#[cfg(not(feature = "generics"))]
use smallvec::SmallVec;
use crate::{component::Component};
use crate::archetype::Archetypes;
use crate::component::ComponentId;

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
    fn desc(&self) -> FilterDesc;
}

pub trait FilterBundle: Sized {
    type Arity: ArrayLength;

    const LEN: usize = Self::Arity::USIZE;

    fn init(archetypes: &mut Archetypes) -> Self;

    #[cfg(feature = "generics")]
    fn desc(&self) -> GenericArray<FilterDesc, Self::Arity>;

    #[cfg(not(feature = "generics"))]
    fn desc(&self) -> SmallVec<[FilterDesc; 2]>;
}

impl FilterBundle for () {
    type Arity = U0;

    fn init(_archetypes: &mut Archetypes) -> Self {}

    #[cfg(feature = "generics")]
    fn desc(&self) -> GenericArray<FilterDesc, U0> {
        GenericArray::default()
    }

    #[cfg(not(feature = "generics"))]
    fn desc(&self) -> SmallVec<[FilterDesc; 2]> {
        SmallVec::new()
    }
}

macro_rules! impl_bundle {
    ($count:expr, $($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens)]
            impl<$($gen:Filter),*> FilterBundle for ($($gen),*) {
                type Arity = [< U $count >];

                const LEN: usize = $count;

                fn init(archetypes: &mut Archetypes) -> Self {
                    ($($gen::init(archetypes)),*)
                }

                #[cfg(feature = "generics")]
                fn desc(&self) -> GenericArray<FilterDesc, Self::Arity> {
                    #[allow(non_snake_case)]
                    let ($($gen),*) = &self;
                    GenericArray::from(($($gen::desc($gen),)*))
                }

                #[cfg(not(feature = "generics"))]
                fn desc(&self) -> SmallVec<[FilterDesc; 2]> {
                    #[allow(non_snake_case)]
                    let ($($gen),*) = &self;
                    smallvec::smallvec![$($gen::desc($gen)),*]
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

pub struct With<T: Component> {
    id: ComponentId,
    _marker: PhantomData<T>
}

impl<T: Component> Filter for With<T> {
    fn init(archetypes: &mut Archetypes) -> Self {
        With {
            id: archetypes.registry.get_or_assign::<T>(),
            _marker: PhantomData
        }
    }

    fn desc(&self) -> FilterDesc {
        FilterDesc::With(self.id)
    }
}

pub struct Without<T: Component> {
    id: ComponentId,
    _marker: PhantomData<T>
}

impl<T: Component> Filter for Without<T> {
    fn init(archetypes: &mut Archetypes) -> Self {
        Without {
            id: archetypes.registry.get_or_assign::<T>(),
            _marker: PhantomData
        }
    }

    fn desc(&self) -> FilterDesc {
        FilterDesc::Without(self.id)
    }
}

pub struct Added<T: Component> {
    id: ComponentId,
    _marker: PhantomData<T>
}

impl<T: Component> Filter for Added<T> {
    fn init(archetypes: &mut Archetypes) -> Self {
        Added {
            id: archetypes.registry.get_or_assign::<T>(),
            _marker: PhantomData
        }
    }

    fn desc(&self) -> FilterDesc {
        FilterDesc::Added(self.id)
    }
}


pub struct Removed<T: Component> {
    id: ComponentId,
    _marker: PhantomData<T>
}

impl<T: Component> Filter for Removed<T> {
    fn init(archetypes: &mut Archetypes) -> Self {
        Removed {
            id: archetypes.registry.get_or_assign::<T>(),
            _marker: PhantomData
        }
    }

    fn desc(&self) -> FilterDesc {
        FilterDesc::Removed(self.id)
    }
}

pub struct Changed<T: Component> {
    id: ComponentId,
    _marker: PhantomData<T>
}

impl<T: Component> Filter for Changed<T> {
    fn init(archetypes: &mut Archetypes) -> Self {
        Changed {
            id: archetypes.registry.get_or_assign::<T>(),
            _marker: PhantomData
        }
    }

    fn desc(&self) -> FilterDesc {
        FilterDesc::Changed(self.id)
    }
}