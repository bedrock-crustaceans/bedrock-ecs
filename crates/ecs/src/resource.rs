use std::{
    any::{Any, TypeId},
    cell::UnsafeCell,
    ops::{Add, Deref, DerefMut},
};

#[cfg(feature = "generics")]
use generic_array::GenericArray;
use generic_array::{ArrayLength, typenum::U1};
use rustc_hash::FxHashMap;

use crate::{
    Signature,
    graph::{AccessDesc, AccessType},
    sealed::Sealed,
    system::SystemMeta,
};

#[cfg(not(feature = "generics"))]
use smallvec::SmallVec;

use crate::Param;
#[cfg(feature = "generics")]
use crate::World;

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

pub trait ResourceBundle {
    fn insert_into(self, resources: &mut Resources);
}

macro_rules! impl_bundle {
    ($count:expr, $($gen:ident),*) => {
        #[allow(unused_parens)]
        impl<$($gen: Resource),*> ResourceBundle for ($($gen),*) {
            fn insert_into(self, resources: &mut Resources) {
                #[allow(non_snake_case)]
                let ($($gen),*) = self;

                resources.reserve($count);
                $(
                    resources.insert($gen);
                )*
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

impl<'s, R: Resource> Deref for Res<'s, R> {
    type Target = R;

    fn deref(&self) -> &R {
        self.0
    }
}

pub struct ResState {
    system: &'static str,
    resource: &'static str
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
        GenericArray::from((
            AccessDesc {
                ty: AccessType::Resource(ResourceId::of::<R>()),
                exclusive: false
            },
        ))
    }

    #[cfg(not(feature = "generics"))]
    #[inline]
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; 4]> {
        smallvec![
            AccessDesc {
                ty: AccessType::Resource(ResourceId::of::<R>()),
                exclusive: false
            }
        ]
    }

    fn init(world: &mut World, meta: &SystemMeta) -> ResState {
        let full_name = std::any::type_name::<R>();
        // Attempt to extract type name.
        let res_name = full_name.split("::").last().unwrap_or(full_name);

        if world.resources.contains::<R>() {
            return ResState {
                resource: res_name,
                system: meta.name
            }
        }

        // Check whether the resource exists and warn otherwise
        tracing::warn!(
            "`Res<{}>` is used by system `{}` but the resource does not exist. Attempting to call this system will cause a panic.",
            res_name,
            meta.name
        );

        ResState {
            resource: res_name,
            system: meta.name
        }
    }

    #[inline]
    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut ResState) -> Res<'w, R> {
        let Some(res) = world.resources.get::<R>() else {
            tracing::error!(
                "System `{}` attempted to access `Res<{}>` which does not exist",
                state.system, state.resource 
            );

            panic!("cannot access non-existent resource");
        };

        Res(res)
    }
}

pub struct ResMut<'s, R: Resource>(&'s mut R);

impl<'s, R: Resource> Deref for ResMut<'s, R> {
    type Target = R;

    fn deref(&self) -> &R {
        self.0
    }
}

impl<'s, R: Resource> DerefMut for ResMut<'s, R> {
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
        GenericArray::from((
            AccessDesc {
                ty: AccessType::Resource(ResourceId::of::<R>()),
                exclusive: false
            },
        ))
    }

    #[cfg(not(feature = "generics"))]
    #[inline]
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; 4]> {
        smallvec![
            AccessDesc {
                ty: AccessType::Resource(ResourceId::of::<R>()),
                exclusive: false
            }
        ]
    }

    fn init(world: &mut World, meta: &SystemMeta) -> ResState {
        let full_name = std::any::type_name::<R>();
        // Attempt to extract type name.
        let res_name = full_name.split("::").last().unwrap_or(full_name);

        if world.resources.contains::<R>() {
            return ResState {
                resource: res_name,
                system: meta.name
            }
        }

        // Check whether the resource exists and warn otherwise
        tracing::warn!(
            "`ResMut<{}>` is used by system `{}` but the resource does not exist. Attempting to call this system will cause a panic.",
            res_name,
            meta.name
        );

        ResState {
            resource: res_name,
            system: meta.name
        }
    }

    #[inline]
    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut ResState) -> ResMut<'w, R> {
        let Some(res) = (unsafe { world.resources.get_mut_unchecked::<R>() }) else {
            tracing::error!(
                "System `{}` attempted to access `ResMut<{}>` which does not exist",
                state.system, state.resource 
            );

            panic!("cannot access non-existent resource");
        };

        ResMut(res)
    }
}

#[derive(Default)]
pub struct Resources {
    pub(crate) storage: FxHashMap<ResourceId, UnsafeCell<Box<dyn Resource>>>,
}

impl Resources {
    /// Creates a new container for resources.
    #[inline]
    pub fn new() -> Resources {
        Resources::default()
    }

    /// Inserts a resource into the container.
    pub fn insert<R: Resource>(&mut self, resource: R) {
        let id = ResourceId::of::<R>();
        self.storage.insert(id, UnsafeCell::new(Box::new(resource)));
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.storage.reserve(additional);
    }

    /// Does the container have these resources?
    #[inline]
    pub fn contains<R: Resource>(&self) -> bool {
        self.storage.contains_key(&ResourceId::of::<R>())
    }

    pub fn get<R: Resource>(&self) -> Option<&R> {
        let id = ResourceId::of::<R>();
        let cell = self.storage.get(&id)?;

        let res = unsafe { &*cell.get().cast_const() };

        res.as_any().downcast_ref::<R>()
    }

    pub fn get_mut<R: Resource>(&mut self) -> Option<&mut R> {
        let id = ResourceId::of::<R>();
        self.storage
            .get_mut(&id)?
            .get_mut() // Take resource out of unsafe cell.
            .as_any_mut()
            .downcast_mut::<R>()
    }

    // Safety: This should only be called if you will have guaranteed unique access to this resource.
    pub(crate) unsafe fn get_mut_unchecked<R: Resource>(&self) -> Option<&mut R> {
        let id = ResourceId::of::<R>();
        let cell = self.storage.get(&id)?;
        let boxed = unsafe { &mut *cell.get() };

        boxed.as_any_mut().downcast_mut::<R>()
    }

    pub fn remove<R: Resource>(&mut self) -> Option<Box<R>> {
        let id = ResourceId::of::<R>();
        let cell = self.storage.remove(&id)?;

        cell.into_inner().into_any().downcast::<R>().ok()
    }
}
