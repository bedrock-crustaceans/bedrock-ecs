use std::fmt;
use std::ops::Deref;

use crate::archetype::Signature;
use crate::component::ComponentRegistry;

/// A component ID.
///
/// This is a unique ID that is assigned to every component type.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct ComponentId(pub(crate) usize);

impl fmt::Display for ComponentId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for ComponentId {
    type Target = usize;

    fn deref(&self) -> &usize {
        &self.0
    }
}

impl From<usize> for ComponentId {
    fn from(v: usize) -> Self {
        Self(v)
    }
}

/// A marker trait indicating that the implementor can be used as a component.
pub trait Component: std::fmt::Debug + 'static {}

/// A collection of components used in a filter. This trait makes it possible to use tuples
/// of components inside of filters rather than just a single component.
///
/// It enables filters such as `With<(Health, Transform)>`.
pub trait ComponentBundle {
    /// Converts this bundle to a signature to compare against archetype tables. If a component had not been
    /// registered, this method will register it.
    ///
    /// In the signature, the indices corresponding the components of this bundle are set to 1.
    fn get_or_assign_signature(reg: &mut ComponentRegistry) -> Signature;

    /// Converts this bundle to a signature to compare against archetype tables. If a component had not been
    /// this function will return `None`.
    ///
    /// In the signature, the indices corresponding the components of this bundle are set to 1.
    fn get_signature(reg: &ComponentRegistry) -> Option<Signature>;
}

macro_rules! impl_filter_bundle {
    ($($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens)]
            impl<$($gen:Component),*> ComponentBundle for ($($gen),*) {
                fn get_or_assign_signature(reg: &mut ComponentRegistry) -> Signature {
                    let mut set = Signature::new();
                    $(
                        let id = reg.get_or_assign::<$gen>();
                        set.set(*id);
                    )*
                    set
                }

                fn get_signature(reg: &ComponentRegistry) -> Option<Signature> {
                    let mut set = Signature::new();
                    $(
                        let id = reg.get::<$gen>()?;
                        set.set(*id);
                    )*
                    Some(set)
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
