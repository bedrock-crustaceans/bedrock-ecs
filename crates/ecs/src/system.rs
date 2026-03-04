#[cfg(debug_assertions)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{cell::UnsafeCell, marker::PhantomData};
use std::any::TypeId;
use crate::{archetype::Archetypes, entity::Entities, param::{Param, ParamBundle}, sealed::Sealer, world::World};
use crate::graph::AccessDesc;

pub trait System {
    /// This function takes a self parameter to make the `System` trait dyn-compatible.
    fn access(&self) -> Vec<AccessDesc>;
    fn call(&self, world: &World);
}

pub trait ParametrizedSystem<G: ParamBundle>: Sized {
    const SEND: bool;

    fn into_container(self, id: usize) -> FnContainer<G, Self> {
        FnContainer {
            #[cfg(debug_assertions)]
            counter: AtomicUsize::new(0),
            id,
            system: self,
            state: UnsafeCell::new(G::init()),
            _marker: PhantomData
        }
    }

    fn call(&self, world: &World, state: &mut G::State);
}

pub struct FnContainer<P: ParamBundle, F: ParametrizedSystem<P>> {
    #[cfg(debug_assertions)]
    pub counter: AtomicUsize,
    pub id: usize,
    pub system: F,
    pub state: UnsafeCell<P::State>,
    pub _marker: PhantomData<P>
}

#[derive(Default)]
pub struct Systems {
    storage: Vec<Box<dyn System>>
}

impl<P, F> System for FnContainer<P, F> 
where
    P: Param,
    F: ParametrizedSystem<P>,
{
    fn access(&self) -> Vec<AccessDesc> {
        P::access()
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

impl<P1: Param, P2: Param, F: ParametrizedSystem<(P1, P2)>> System for FnContainer<(P1, P2), F> {
    fn access(&self) -> Vec<AccessDesc> {
        let mut p1 = P1::access();
        p1.extend(P2::access());

        p1
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

impl<F: Fn(P::Item<'_>), P: Param> ParametrizedSystem<P> for F {
    const SEND: bool = P::SEND;

    fn call(&self, world: &World, state: &mut P::State) {
        let p = P::fetch::<Sealer>(world, state);
        self(p);
    }
}

impl<F: Fn(P1::Item<'_>, P2::Item<'_>), P1: Param, P2: Param> ParametrizedSystem<(P1, P2)> for F {
    const SEND: bool = P1::SEND && P2::SEND;

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

    pub fn push<P, S: IntoSystem<P>>(&mut self, system: S) {
        let system = system.into_system();
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
    fn into_system(self) -> Box<dyn System>;
}

#[diagnostic::do_not_recommend]
impl<F, P> IntoSystem<P> for F
where
    P: Param + 'static,
    F: Fn(P) + 'static,
    F: ParametrizedSystem<P>
{
    fn into_system(self) -> Box<dyn System> {
        Box::new(self.into_container(0))
    }
}

#[diagnostic::do_not_recommend]
impl<F, P1, P2> IntoSystem<(P1, P2)> for F
where
    P1: Param + 'static,
    P2: Param + 'static,
    F: Fn(P1, P2) + 'static,
    F: ParametrizedSystem<(P1, P2)>
{
    fn into_system(self) -> Box<dyn System> {
        Box::new(self.into_container(0))
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