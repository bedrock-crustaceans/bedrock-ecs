use std::any::TypeId;
use std::cell::UnsafeCell;
use std::sync::atomic::Ordering;

use generic_array::GenericArray;

use crate::scheduler::AccessDesc;
use crate::sealed::Sealer;
use crate::system::{Param, ParamBundle};
#[cfg(debug_assertions)]
use crate::util::debug::BorrowEnforcer;
use crate::world::World;

#[derive(Debug)]
pub struct SystemMeta {
    /// For the Rust-only side of the ECS, using a static str is fine here,
    /// but for systems coming from WebAssembly the names are loaded at runtime.
    pub(crate) name: String,
    pub(crate) last_ran: u32,
}

impl SystemMeta {
    #[inline]
    pub fn last_ran(&self) -> u32 {
        self.last_ran
    }

    /// Returns the name of the system.
    ///
    /// This name is determined on a best-effort basis. It may not always be entirely accurate.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }
}

// `System` must implement `Sync` such that rayon can run them on other threads.
pub trait System: Sync {
    /// Returns the resources that this system accesses.
    fn access(&self) -> &[AccessDesc];

    fn meta(&self) -> &SystemMeta;
    /// Executes the system.
    ///
    /// # Safety
    ///
    /// The caller must manually uphold Rust's aliasing guarantees in regards to system resource access.
    unsafe fn call(&self, world: &World);
}

pub trait ParametrizedSystem<P: ParamBundle>: Sized {
    fn into_container(self, world: &mut World) -> SystemContainer<P, Self> {
        let mut name = std::any::type_name::<Self>();
        if !name.contains('{') {
            name = name.split("::").last().unwrap_or(name);
        }

        let access = P::access(world);
        let meta = SystemMeta {
            last_ran: world.current_tick,
            name: name.to_owned(),
        };

        let state = P::init(world, &meta);

        SystemContainer {
            meta: UnsafeCell::new(meta),
            system: self,
            access,
            state: UnsafeCell::new(state),

            #[cfg(debug_assertions)]
            enforcer: BorrowEnforcer::new(),
        }
    }

    fn call(&self, world: &World, state: &mut P::State);
}

/// Wraps the system and all of its metadata.
///
/// This is where the the states and metadata are stored.
pub struct SystemContainer<P: ParamBundle, F: ParametrizedSystem<P>> {
    pub(crate) meta: UnsafeCell<SystemMeta>,
    pub(crate) system: F,
    pub(crate) state: UnsafeCell<P::State>,

    #[cfg(debug_assertions)]
    pub(crate) enforcer: BorrowEnforcer,

    #[cfg(feature = "generics")]
    pub(crate) access: GenericArray<AccessDesc, P::AccessCount>,
    #[cfg(not(feature = "generics"))]
    pub(crate) access: SmallVec<[AccessDesc; param::INLINE_SIZE]>,
}

unsafe impl<P, F> Send for SystemContainer<P, F>
where
    P: ParamBundle,
    F: ParametrizedSystem<P>,
{
}

unsafe impl<P, F> Sync for SystemContainer<P, F>
where
    P: ParamBundle,
    F: ParametrizedSystem<P>,
{
}

macro_rules! impl_system {
    ($($gen:ident),*) => {
        impl<$($gen),*, Sys> System for SystemContainer<($($gen),*), Sys>
        where
            $($gen: Param),*,
            ($($gen),*): ParamBundle,
            Sys: ParametrizedSystem<($($gen),*)>
        {
            #[inline]
            fn meta(&self) -> &SystemMeta {
                unsafe { &*self.meta.get().cast_const() }
            }

            fn access(&self) -> &[AccessDesc] {
                &self.access
            }

            unsafe fn call(&self, world: &World) {
                #[cfg(debug_assertions)]
                let _guard = self.enforcer.write();

                // SAFETY:
                // This is safe because every system has a unique state. At the same time a system
                // can be used on only one thread at a time.
                let state = unsafe { &mut *self.state.get() };
                self.system.call(world, state);

                let meta = unsafe { &mut *self.meta.get() };
                meta.last_ran = world.current_tick;
            }
        }

        impl<$($gen),*, Sys: Fn($($gen::Output<'_>),*)> ParametrizedSystem<($($gen),*)> for Sys
        where
            $($gen: Param),*,
            ($($gen),*): ParamBundle<State = ($($gen::State),*)>
        {
            #[allow(non_snake_case)]
            fn call(&self, world: &World, state: &mut <($($gen),*) as ParamBundle>::State) {
                let ($($gen),*) = state;
                self($(
                    $gen::fetch::<Sealer>(world, $gen)
                ),*)
            }
        }

        impl<$($gen),*, Sys> IntoSystem<($($gen),*)> for Sys
        where
            $($gen: Param + 'static),*,
            ($($gen),*): ParamBundle,
            Sys: ParametrizedSystem<($($gen),*)>,
            Sys: Fn($($gen),*) + 'static
        {
            fn into_system(self, world: &mut World) -> impl System + 'static {
                self.into_container(world)
            }

            fn into_boxed_system(self, world: &mut World) -> Box<dyn System> {
                Box::new(self.into_container(world))
            }
        }
    }
}

impl_system!(A, B);
impl_system!(A, B, C);
impl_system!(A, B, C, D);
impl_system!(A, B, C, D, E);
impl_system!(A, B, C, D, E, F);
impl_system!(A, B, C, D, E, F, G);
impl_system!(A, B, C, D, E, F, G, H);
impl_system!(A, B, C, D, E, F, G, H, I);
impl_system!(A, B, C, D, E, F, G, H, I, J);

impl<P, F> System for SystemContainer<P, F>
where
    P: Param,
    F: ParametrizedSystem<P>,
{
    #[inline]
    fn meta(&self) -> &SystemMeta {
        unsafe { &*self.meta.get().cast_const() }
    }

    fn access(&self) -> &[AccessDesc] {
        &self.access
    }

    unsafe fn call(&self, world: &World) {
        // SAFETY:
        // This is safe because every system has a unique state. At the same time a system
        // can be used on only one thread at a time.
        let state = unsafe { &mut *self.state.get() };
        self.system.call(world, state);
    }
}

impl<F: Fn(P::Output<'_>) + Sync, P: Param> ParametrizedSystem<P> for F {
    fn call(&self, world: &World, state: &mut P::State) {
        let p = P::fetch::<Sealer>(world, state);
        self(p);
    }
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid system",
    label = "this system has invalid parameters",
    note = "check the parameters of the system, are they all valid?",
    note = "examples of valid parameters are `Query`, `Local`, `Res`, etc..."
)]
pub trait IntoSystem<P> {
    fn into_system(self, world: &mut World) -> impl System + 'static;
    fn into_boxed_system(self, world: &mut World) -> Box<dyn System>;
}

#[diagnostic::do_not_recommend]
impl<F, P> IntoSystem<P> for F
where
    P: Param + 'static,
    F: Fn(P) + 'static,
    F: ParametrizedSystem<P>,
{
    // static lifetime is required to tell Rust that the returned system does not borrow the world.
    fn into_system(self, world: &mut World) -> impl System + 'static {
        self.into_container(world)
    }

    fn into_boxed_system(self, world: &mut World) -> Box<dyn System> {
        Box::new(self.into_container(world))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SystemId(pub(crate) u32);
