use std::any::TypeId;
use std::cell::UnsafeCell;

use generic_array::GenericArray;

use crate::scheduler::AccessDesc;
use crate::sealed::Sealer;
use crate::system::{Param, ParamBundle};
#[cfg(debug_assertions)]
use crate::util::debug::BorrowEnforcer;
use crate::world::World;

#[derive(Debug)]
pub struct SystemMeta {
    pub(crate) id: SystemId,
    pub(crate) name: &'static str,
}

impl SystemMeta {
    /// Returns the unique identifier of the system.
    #[inline]
    pub fn id(&self) -> SystemId {
        self.id
    }

    /// Returns the name of the system.
    ///
    /// This name is determined on a best-effort basis. It may not always be entirely accurate.
    #[inline]
    pub fn name(&self) -> &'static str {
        self.name
    }
}

// `System` must implement `Sync` such that rayon can run them on other threads.
pub trait System: Sync {
    /// Attempts to determine the name of this system.
    fn name(&self) -> &'static str;
    /// Returns the resources that this system accesses.
    fn access(&self) -> &[AccessDesc];
    /// Runs the system.
    unsafe fn call(&self, world: &World);
}

pub trait ParametrizedSystem<P: ParamBundle>: Sized {
    fn into_container(self, world: &mut World, id: SystemId) -> SystemContainer<P, Self> {
        let mut name = std::any::type_name::<Self>();
        if !name.contains('{') {
            name = name.split("::").last().unwrap_or(name);
        }

        let access = P::access(world);
        let meta = SystemMeta { id, name };

        let state = P::init(world, &meta);

        SystemContainer {
            meta,
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
    meta: SystemMeta,
    system: F,
    state: UnsafeCell<P::State>,

    #[cfg(debug_assertions)]
    enforcer: BorrowEnforcer,

    #[cfg(feature = "generics")]
    access: GenericArray<AccessDesc, P::AccessCount>,
    #[cfg(not(feature = "generics"))]
    access: SmallVec<[AccessDesc; param::INLINE_SIZE]>,
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
        impl<$($gen),*, F> System for SystemContainer<($($gen),*), F>
        where
            $($gen: Param),*,
            ($($gen),*): ParamBundle,
            F: ParametrizedSystem<($($gen),*)>
        {
            #[inline]
            fn name(&self) -> &'static str {
                self.meta.name
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
            }
        }

        impl<$($gen),*, F: Fn($($gen::Output<'_>),*)> ParametrizedSystem<($($gen),*)> for F
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

        impl<$($gen),*, F> IntoSystem<($($gen),*)> for F
        where
            $($gen: Param),*,
            F: ParametrizedSystem<($($gen),*)>,
            F: Fn($($gen),*)
        {
            fn into_system(self, world: &mut World, id: SystemId) -> Box<dyn System> {
                Box::new(self.into_container(world, id))
            }
        }
    }
}

impl_system!(A, B, C);

impl<P, F> System for SystemContainer<P, F>
where
    P: Param,
    F: ParametrizedSystem<P>,
{
    #[inline]
    fn name(&self) -> &'static str {
        self.meta.name
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

impl<P1: Param, P2: Param, F: ParametrizedSystem<(P1, P2)>> System for SystemContainer<(P1, P2), F>
where
    (P1, P2): ParamBundle,
{
    #[inline]
    fn name(&self) -> &'static str {
        self.meta.name
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

impl<F: Fn(P1::Output<'_>, P2::Output<'_>) + Sync, P1: Param, P2: Param>
    ParametrizedSystem<(P1, P2)> for F
where
    (P1, P2): ParamBundle<State = (P1::State, P2::State)>,
{
    fn call(&self, world: &World, state: &mut <(P1, P2) as ParamBundle>::State) {
        let p1 = P1::fetch::<Sealer>(world, &mut state.0);
        let p2 = P2::fetch::<Sealer>(world, &mut state.1);

        self(p1, p2);
    }
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid system",
    label = "this system has invalid parameters",
    note = "check the parameters of the system, are they all valid?",
    note = "examples of valid parameters are `Query`, `Local`, `Res`, etc..."
)]
pub trait IntoSystem<P> {
    fn into_system(self, world: &mut World, id: SystemId) -> Box<dyn System>;
}

#[diagnostic::do_not_recommend]
impl<F, P> IntoSystem<P> for F
where
    P: Param + 'static,
    F: Fn(P) + 'static,
    F: ParametrizedSystem<P>,
{
    fn into_system(self, world: &mut World, id: SystemId) -> Box<dyn System> {
        Box::new(self.into_container(world, id))
    }
}

#[diagnostic::do_not_recommend]
impl<F, P1, P2> IntoSystem<(P1, P2)> for F
where
    P1: Param + 'static,
    P2: Param + 'static,
    (P1, P2): ParamBundle,
    F: Fn(P1, P2) + 'static,
    F: ParametrizedSystem<(P1, P2)>,
{
    fn into_system(self, world: &mut World, id: SystemId) -> Box<dyn System> {
        Box::new(self.into_container(world, id))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SystemId(pub(crate) TypeId);

impl SystemId {
    pub const fn of<P, F: IntoSystem<P> + 'static>() -> SystemId {
        SystemId(TypeId::of::<F>())
    }
}
