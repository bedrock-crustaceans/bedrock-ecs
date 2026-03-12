#[cfg(debug_assertions)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{cell::UnsafeCell, marker::PhantomData};
use std::any::TypeId;
use generic_array::GenericArray;
use smallvec::SmallVec;

use crate::{param::{Param, ParamBundle}, sealed::Sealer, world::World};
use crate::graph::AccessDesc;

pub trait System: Sync {
    /// Attempts to determine the name of this system.
    fn name(&self) -> String;
    fn access(&self) -> &[AccessDesc];
    fn call(&self, world: &World);
}

pub trait ParametrizedSystem<P: ParamBundle>: Sized + Sync {
    fn into_container(self, world: &mut World, id: usize) -> FnContainer<P, Self> {
        let access = P::access(world);
        let state = P::init(world);

        FnContainer {
            #[cfg(debug_assertions)]
            counter: AtomicUsize::new(0),
            id,
            system: self,
            access,
            state: UnsafeCell::new(state)
        }
    }

    fn call(&self, world: &World, state: &mut P::State);
}

pub struct FnContainer<P: ParamBundle, F: ParametrizedSystem<P>> {
    #[cfg(debug_assertions)]
    pub counter: AtomicUsize,
    pub id: usize,
    pub system: F,
    pub access: GenericArray<AccessDesc, P::AccessCount>,
    pub state: UnsafeCell<P::State>,
}

unsafe impl<P, F> Send for FnContainer<P, F> 
where
    P: ParamBundle,
    F: ParametrizedSystem<P> {}

unsafe impl<P, F> Sync for FnContainer<P, F> 
where
    P: ParamBundle,
    F: ParametrizedSystem<P> {}

#[derive(Default)]
pub struct Systems {
    storage: Vec<Box<dyn System>>
}

impl<P, F> System for FnContainer<P, F> 
where
    P: Param,
    F: ParametrizedSystem<P>,
{
    fn name(&self) -> String {
        let type_name = std::any::type_name::<F>();
        let split = type_name.split("::").last().unwrap_or("unknown");

        split.to_owned()
    }

    fn access(&self) -> &[AccessDesc] {
        &self.access
    }

    fn call(&self, world: &World) {
        #[cfg(debug_assertions)]
        {
            let counter = self.counter.fetch_add(1, Ordering::SeqCst);
            assert_eq!(counter, 0, "Attempt to access system state twice");
        }

        // SAFETY:
        // This is safe because every system has a unique state. At the same time a system
        // can be used on only one thread at a time.
        let state = unsafe { &mut *self.state.get() };
        self.system.call(world, state);

        #[cfg(debug_assertions)]
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

impl<P1: Param, P2: Param, F: ParametrizedSystem<(P1, P2)>> System for FnContainer<(P1, P2), F>
where
    (P1, P2): ParamBundle
{
    fn name(&self) -> String {
        todo!()
    }

    fn access(&self) -> &[AccessDesc] {
        &self.access
    }
    
    fn call(&self, world: &World) {
        #[cfg(debug_assertions)]
        {
            let counter = self.counter.fetch_add(1, Ordering::SeqCst);
            assert_eq!(counter, 0, "Attempt to access system state twice");
        }

        // SAFETY:
        // This is safe because every system has a unique state. At the same time a system
        // can be used on only one thread at a time.
        let state = unsafe { &mut *self.state.get() };
        self.system.call(world, state);

        #[cfg(debug_assertions)]
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

impl<F: Fn(P::Output<'_>) + Sync, P: Param> ParametrizedSystem<P> for F {
    fn call(&self, world: &World, state: &mut P::State) {
        let p = P::fetch::<Sealer>(world, state);
        self(p);
    }
}

impl<F: Fn(P1::Output<'_>, P2::Output<'_>) + Sync, P1: Param, P2: Param> ParametrizedSystem<(P1, P2)> for F
where
    (P1, P2): ParamBundle<State = (P1::State, P2::State)>
{

    fn call(&self, world: &World, state: &mut <(P1, P2) as ParamBundle>::State) {
        let p1 = P1::fetch::<Sealer>(world, &mut state.0);
        let p2 = P2::fetch::<Sealer>(world, &mut state.1);

        self(p1, p2);
    }
}

impl Systems {
    pub fn new() -> Systems {
        Systems::default()
    }

    pub fn reserve(&mut self, n: usize) {
        self.storage.reserve(n);
    }

    pub fn push<P, S: IntoSystem<P>>(&mut self, world: &mut World, system: S) {
        let system = system.into_system(world);
        let desc= system.access();

        println!("System desc: {desc:?}");

        self.storage.push(system);
    }

    pub fn call(&self, world: &World) {
        for sys in &self.storage {
            sys.call(world);
        }
    }
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid system",
    label = "this system has invalid parameters",
    note = "check the parameters of the system, are they all valid?",
    note = "examples of valid parameters are `Query`, `Local`, `Res`, etc..."
)]
pub trait IntoSystem<P> {
    fn into_system(self, world: &mut World) -> Box<dyn System>;
}

#[diagnostic::do_not_recommend]
impl<F, P> IntoSystem<P> for F
where
    P: Param + 'static,
    F: Fn(P) + 'static,
    F: ParametrizedSystem<P>
{
    fn into_system(self, world: &mut World) -> Box<dyn System> {
        Box::new(self.into_container(world, 0))
    }
}

#[diagnostic::do_not_recommend]
impl<F, P1, P2> IntoSystem<(P1, P2)> for F
where
    P1: Param + 'static,
    P2: Param + 'static,
    (P1, P2): ParamBundle,
    F: Fn(P1, P2) + 'static,
    F: ParametrizedSystem<(P1, P2)>
{
    fn into_system(self, world: &mut World) -> Box<dyn System> {
        Box::new(self.into_container(world, 0))
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