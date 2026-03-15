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

pub trait ResourceBundle: Send + Sync + 'static {
    type AccessCount: ArrayLength + Add;

    #[cfg(feature = "generics")]
    fn access(world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount>;

    #[cfg(not(feature = "generics"))]
    fn access(world: &mut World) -> SmallVec<[AccessDesc; 4]>;

    fn contains(resources: &Resources) -> bool;
}

macro_rules! impl_bundle {
    ($count:literal, $($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens)]
            impl<$($gen:Resource),*> ResourceBundle for ($($gen),*) {
                #[cfg(feature = "generics")]
                type AccessCount = generic_array::typenum::[< U $count >];

                #[cfg(feature = "generics")]
                fn access(_world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount> {
                    GenericArray::from(
                        ($(
                            // This is done inside the macro instead of defining an `access`
                            // method in order to make the `Resource` trait dyn compatible.
                            //
                            // `ResourceId::of` requires `Self` to be `Sized` but that would make
                            // `Resource` dyn incompatible.
                            AccessDesc {
                                ty: AccessType::Resource(ResourceId::of::<$gen>()),
                                exclusive: false
                            },
                        )*)
                    )
                }

                #[cfg(not(feature = "generics"))]
                fn access(_world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
                    smallvec::smallvec![
                        $(
                            $gen::access()
                        ),*
                    ]
                }

                fn contains(resources: &Resources) -> bool {
                    $(resources.storage.contains_key(&ResourceId::of::<$gen>()))&&*
                }
            }
        }
    }
}

impl_bundle!(1, A);
impl_bundle!(2, A, B);

/// Obtains a shared reference to a global [`Resource`].
///
/// The system will panic if the given resource does not exist, so make sure to create it before
/// attempting to use the resource.
pub struct Res<'s, R: ResourceBundle> {
    inner: &'s R,
}

impl<'s, R: ResourceBundle> Deref for Res<'s, R> {
    type Target = R;

    fn deref(&self) -> &R {
        self.inner
    }
}

unsafe impl<R: ResourceBundle> Param for Res<'_, R> {
    #[cfg(feature = "generics")]
    type AccessCount = R::AccessCount;
    type Output<'s> = Res<'s, R>;
    type State = ();

    #[cfg(feature = "generics")]
    fn access(world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount> {
        R::access(world)
    }

    #[cfg(not(feature = "generics"))]
    fn access(world: &mut World) -> SmallVec<[AccessDesc; 4]> {
        R::access(world)
    }

    fn init(world: &mut World, meta: &SystemMeta) {
        if R::contains(&world.resources) {
            return;
        }

        let full_name = std::any::type_name::<R>();

        // Check whether the resource exists and warn otherwise
        tracing::warn!(
            "`Res<{}>` is used by system `{}` but resource(s) do not exist. Attempting to access these before they are initialized will cause a panic.",
            full_name,
            meta.name
        );
    }

    fn fetch<'w, S: Sealed>(world: &'w World, _state: &'w mut ()) -> Res<'w, R> {
        todo!()
    }
}

pub struct ResMut<'s, R: ResourceBundle> {
    inner: &'s mut R,
}

impl<'s, R: ResourceBundle> Deref for ResMut<'s, R> {
    type Target = R;

    fn deref(&self) -> &R {
        self.inner
    }
}

impl<'s, R: ResourceBundle> DerefMut for ResMut<'s, R> {
    fn deref_mut(&mut self) -> &mut R {
        self.inner
    }
}

#[derive(Default)]
pub struct Resources {
    pub(crate) storage: FxHashMap<ResourceId, UnsafeCell<Box<dyn Resource>>>,
}

impl Resources {
    /// Creates a new container for resources.
    pub fn new() -> Resources {
        Resources::default()
    }

    /// Inserts a resource into the container.
    pub fn insert<R: Resource>(&mut self, resource: R) {
        let id = ResourceId::of::<R>();
        self.storage.insert(id, UnsafeCell::new(Box::new(resource)));
    }

    /// Does the container have these resources?
    pub fn contains<R: ResourceBundle>(&self) -> bool {
        R::contains(self)
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

    pub fn remove<R: Resource>(&mut self) -> Option<Box<R>> {
        let id = ResourceId::of::<R>();
        let cell = self.storage.remove(&id)?;

        cell.into_inner().into_any().downcast::<R>().ok()
    }
}
