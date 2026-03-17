use std::{any::TypeId, fmt, ops::Deref};

use rustc_hash::FxHashMap;

use crate::signature::Signature;

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

/// Maintains a consistent mapping from component type IDs to unique integers.
///
/// This is used to reduce the size of component IDs from random 128-bit type id hashes to
/// smaller consecutive 64-bit IDs.
#[derive(Debug, Default)]
pub struct ComponentRegistry {
    /// The map from type IDs to component IDs.
    mapping: FxHashMap<TypeId, usize>,
    /// The next ID to be assigned to a component.
    next_id: usize,
}

impl ComponentRegistry {
    /// Creates a new registry.
    pub fn new() -> ComponentRegistry {
        ComponentRegistry {
            mapping: FxHashMap::default(),
            next_id: 0,
        }
    }

    /// Returns the component's ID or `None` if it has not been registered.
    pub fn get<T: Component>(&self) -> Option<ComponentId> {
        let ty_id = TypeId::of::<T>();
        self.mapping.get(&ty_id).copied().map(ComponentId::from)
    }

    /// Returns the component's ID if it exists, or assigns and returns a new one if it does not.
    pub fn get_or_assign<T: Component>(&mut self) -> ComponentId {
        let ty_id = TypeId::of::<T>();

        let id = self.mapping.get(&ty_id).copied().unwrap_or_else(|| {
            self.mapping.insert(ty_id, self.next_id);
            self.next_id += 1;
            self.next_id - 1
        });

        ComponentId(id)
    }
}
