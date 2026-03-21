use std::any::{Any, TypeId};
use std::ops::{Deref, DerefMut};

#[cfg(feature = "generics")]
use generic_array::GenericArray;
use generic_array::typenum::U1;

use crate::resource::ResourceRegistry;
use crate::scheduler::{AccessDesc, AccessType};
use crate::sealed::Sealed;
use crate::system::{Param, SystemMeta};
use crate::world::World;

#[cfg(not(feature = "generics"))]
use smallvec::SmallVec;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ResourceId(pub(crate) TypeId);

impl ResourceId {
    pub const fn of<R: Resource>() -> ResourceId {
        ResourceId(TypeId::of::<R>())
    }
}

pub trait Resource: Send + Sync + 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

/// A collection of [`Resource`]s.
pub trait ResourceBundle {
    /// Inserts all [`Resource`]s into the given [`Resources`].
    fn insert_into(self, resources: &mut ResourceRegistry);
    /// Whether the given [`Resources`] contains all [`Resource`]s in this bundle.
    fn contains_all(resources: &ResourceRegistry) -> bool;
}

macro_rules! impl_bundle {
    ($count:expr, $($gen:ident),*) => {
        #[allow(unused_parens)]
        impl<$($gen: Resource),*> ResourceBundle for ($($gen),*) {
            fn insert_into(self, resources: &mut ResourceRegistry) {
                #[allow(non_snake_case)]
                let ($($gen),*) = self;

                resources.reserve($count);
                $(
                    resources.insert($gen);
                )*
            }

            fn contains_all(resources: &ResourceRegistry) -> bool {
                $(
                    resources.storage.contains_key(&ResourceId::of::<$gen>())
                )&&*
            }
        }
    }
}

impl_bundle!(1, A);
impl_bundle!(2, A, B);
impl_bundle!(3, A, B, C);
impl_bundle!(4, A, B, C, D);
impl_bundle!(5, A, B, C, D, E);

/// Obtains a shared reference to a global [`Resource`].
///
/// The system will panic if the given resource does not exist, so make sure to create it before
/// attempting to use the resource.
pub struct Res<'s, R: Resource>(&'s R);

impl<R: Resource> Deref for Res<'_, R> {
    type Target = R;

    fn deref(&self) -> &R {
        self.0
    }
}

pub struct ResState {
    system: &'static str,
    resource: &'static str,
}

unsafe impl<R: Resource> Param for Res<'_, R> {
    #[cfg(feature = "generics")]
    type AccessCount = U1;
    type Output<'s> = Res<'s, R>;

    // Keep track of system and resource name for logging purposes.
    type State = ResState;

    #[cfg(feature = "generics")]
    #[inline]
    fn access(_world: &mut World) -> GenericArray<AccessDesc, U1> {
        GenericArray::from((AccessDesc {
            ty: AccessType::Resource(ResourceId::of::<R>()),
            exclusive: false,
        },))
    }

    #[cfg(not(feature = "generics"))]
    #[inline]
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; 4]> {
        smallvec![AccessDesc {
            ty: AccessType::Resource(ResourceId::of::<R>()),
            exclusive: false
        }]
    }

    fn init(world: &mut World, meta: &SystemMeta) -> ResState {
        let full_name = std::any::type_name::<R>();
        // Attempt to extract type name.
        let res_name = full_name.split("::").last().unwrap_or(full_name);

        if world.resources.contains::<R>() {
            return ResState {
                resource: res_name,
                system: meta.name,
            };
        }

        // Check whether the resource exists and warn otherwise
        tracing::warn!(
            "`Res<{}>` is used by system `{}` but the resource does not exist. Attempting to call this system will cause a panic.",
            res_name,
            meta.name
        );

        ResState {
            resource: res_name,
            system: meta.name,
        }
    }

    #[inline]
    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut ResState) -> Res<'w, R> {
        let Some(res) = world.resources.get::<R>() else {
            tracing::error!(
                "System `{}` attempted to access `Res<{}>` which does not exist",
                state.system,
                state.resource
            );

            panic!("cannot access non-existent resource");
        };

        Res(res)
    }
}

pub struct ResMut<'s, R: Resource>(&'s mut R);

impl<R: Resource> Deref for ResMut<'_, R> {
    type Target = R;

    fn deref(&self) -> &R {
        self.0
    }
}

impl<R: Resource> DerefMut for ResMut<'_, R> {
    fn deref_mut(&mut self) -> &mut R {
        self.0
    }
}

unsafe impl<R: Resource> Param for ResMut<'_, R> {
    #[cfg(feature = "generics")]
    type AccessCount = U1;
    type Output<'s> = ResMut<'s, R>;

    // Keep track of system and resource name for logging purposes.
    type State = ResState;

    #[cfg(feature = "generics")]
    #[inline]
    fn access(_world: &mut World) -> GenericArray<AccessDesc, U1> {
        GenericArray::from((AccessDesc {
            ty: AccessType::Resource(ResourceId::of::<R>()),
            exclusive: false,
        },))
    }

    #[cfg(not(feature = "generics"))]
    #[inline]
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; 4]> {
        smallvec![AccessDesc {
            ty: AccessType::Resource(ResourceId::of::<R>()),
            exclusive: false
        }]
    }

    fn init(world: &mut World, meta: &SystemMeta) -> ResState {
        let full_name = std::any::type_name::<R>();
        // Attempt to extract type name.
        let res_name = full_name.split("::").last().unwrap_or(full_name);

        if world.resources.contains::<R>() {
            return ResState {
                resource: res_name,
                system: meta.name,
            };
        }

        // Check whether the resource exists and warn otherwise
        tracing::warn!(
            "`ResMut<{}>` is used by system `{}` but the resource does not exist. Attempting to call this system will cause a panic.",
            res_name,
            meta.name
        );

        ResState {
            resource: res_name,
            system: meta.name,
        }
    }

    #[inline]
    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut ResState) -> ResMut<'w, R> {
        let Some(ptr) = world.resources.get_ptr::<R>() else {
            tracing::error!(
                "System `{}` attempted to access `ResMut<{}>` which does not exist",
                state.system,
                state.resource
            );

            panic!(
                "cannot access non-existent resource, ensure you've called `World::insert_resources`"
            );
        };

        // Safety: This is safe because `ptr` points to a value of type `R` that is properly aligned.
        // It is also `NonNull` and the scheduler ensures that this is the only system with access to this resource.
        ResMut(unsafe { &mut *ptr.as_ptr() })
    }
}
