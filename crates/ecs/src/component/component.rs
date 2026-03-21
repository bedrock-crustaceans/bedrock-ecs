use std::any::TypeId;
use std::fmt;
use std::ops::Deref;

use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::archetype::Signature;
use crate::component::ComponentRegistry;
use crate::table::{Column, Table};

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
pub trait Component: 'static {}

/// A collection of components used in a filter. This trait makes it possible to use tuples
/// of components inside of filters rather than just a single component.
///
/// It enables filters such as `With<(Health, Transform)>`.
pub trait ComponentBundle: 'static {
    const LEN: usize;

    /// Converts this bundle to a signature to compare against archetype tables. If a component had not been
    /// registered, this method will register it.
    ///
    /// If you do not have mutable access to the component registry, try [`try_get_signature`].
    ///
    /// In the signature, the indices corresponding the components of this bundle are set to 1.
    ///
    /// [`try_get_signature`]: Self::try_get_signature
    fn get_or_assign_signature(reg: &mut ComponentRegistry) -> Signature;

    /// Converts this bundle to a signature to compare against archetype tables. If a component had not been
    /// this function will return `None`.
    ///
    /// The advantage of this method is that you do not need mutable access to the component registry.
    ///
    /// In the signature, the indices corresponding the components of this bundle are set to 1.
    fn try_get_signature(reg: &ComponentRegistry) -> Option<Signature>;

    /// Creates a new table for this bundle of components. This table can be inserted into the archetypes container.
    ///
    /// # Safety
    ///
    /// The given `signature` must be the exact signature of `Self`. This signature should be obtained using
    /// [`try_get_signature`] or [`get_or_assign_signature`].
    unsafe fn into_table(signature: Signature) -> Table;

    /// Insert this bundle into an existing table.
    fn insert_into(self, storage: &mut [Column]);
}

/// Implements [`ComponentBundle`] for tuples of varying arities.
macro_rules! impl_component_bundle {
    ($($gen:ident),*) => {
        paste::paste! {
            #[allow(unused)]
            impl<$($gen:Component),*> ComponentBundle for ($($gen),*) {
                const LEN: usize = (&[$( stringify!($gen) ),*] as &[&str]).len();

                fn get_or_assign_signature(reg: &mut ComponentRegistry) -> Signature {
                    let mut set = Signature::new();
                    $(
                        let id = reg.get_or_assign::<$gen>();
                        set.set(*id);
                    )*
                    set
                }

                fn try_get_signature(reg: &ComponentRegistry) -> Option<Signature> {
                    let mut set = Signature::new();
                    $(
                        let id = reg.get::<$gen>()?;
                        set.set(*id);
                    )*
                    Some(set)
                }

                #[cfg_attr(
                    feature = "tracing",
                    tracing::instrument(name = "ComponentBundle::into_table", fields(bundle = std::any::type_name::<Self>()), skip_all)
                )]
                #[allow(unused)]
                unsafe fn into_table(signature: Signature) -> Table {
                    let mut lookup = FxHashMap::with_capacity_and_hasher(Self::LEN, FxBuildHasher::default());
                    let mut counter = 0;
                    $(
                        lookup.insert(TypeId::of::<$gen>(), counter);
                        counter += 1;
                    )*

                    Table {
                        signature,
                        entities: Vec::new(),
                        entity_lookup: FxHashMap::default(),
                        lookup,
                        columns: vec![
                            $(
                                Column::new::<$gen>()
                            ),*
                        ]
                    }
                }

                #[cfg_attr(
                    feature = "tracing",
                    tracing::instrument(name = "ComponentBundle::insert_into", fields(bundle = std::any::type_name::<Self>()), skip_all)
                )]
                #[allow(unused)]
                fn insert_into(self, storage: &mut [Column]) {
                    let ($([<$gen:lower>]),*) = self;
                    let mut counter = 0;
                    $(
                        storage[counter].push([<$gen:lower>]);
                        counter += 1;
                    )*
                }
            }
        }
    }
}

impl_component_bundle!();
impl_component_bundle!(A);
impl_component_bundle!(A, B);
impl_component_bundle!(A, B, C);
impl_component_bundle!(A, B, C, D);
impl_component_bundle!(A, B, C, D, E);
