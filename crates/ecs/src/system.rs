#[cfg(debug_assertions)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{cell::UnsafeCell, marker::PhantomData};

use crate::{param::{Param, ParamDesc, ParamGroup}, sealed::Sealer, world::World};

pub struct SystemDescriptor {
    pub id: usize,
    pub deps: Vec<ParamDesc>
}

pub trait System {
    /// This function takes a self parameter to make the `System` trait dyn-compatible.
    fn desc(&self) -> SystemDescriptor;
    fn call(&self, world: &World);
}

pub trait ParametrizedSystem<G: ParamGroup>: Sized {
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

pub struct FnContainer<P: ParamGroup, F: ParametrizedSystem<P>> {
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
    fn desc(&self) -> SystemDescriptor {
        SystemDescriptor {
            id: self.id,
            deps: vec![P::desc()]
        }
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
    fn desc(&self) -> SystemDescriptor {
        SystemDescriptor {
            id: self.id,
            deps: vec![P1::desc(), P2::desc()]
        }
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
    fn call(&self, world: &World, state: &mut P::State) {
        let p = P::fetch::<Sealer>(world, state);
        self(p);
    }
}

impl<F: Fn(P1::Item<'_>, P2::Item<'_>), P1: Param, P2: Param> ParametrizedSystem<(P1, P2)> for F {
    fn call(&self, world: &World, state: &mut <(P1, P2) as ParamGroup>::State) {
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

    pub fn push<P: ParamGroup + 'static, S: ParametrizedSystem<P> + 'static>(&mut self, system: S) where FnContainer<P, S>: System {
        self.storage.push(Box::new(system.into_container(0)));
    }

    pub fn call(&self, world: &World) {
        for sys in &self.storage {
            sys.call(world);
        }
    }
}