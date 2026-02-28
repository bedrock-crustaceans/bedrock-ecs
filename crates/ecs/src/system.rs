use std::marker::PhantomData;

use crate::{param::{Param, ParamDesc, ParamGroup}, sealed::Sealer};

pub struct SystemDescriptor {
    pub id: usize,
    pub deps: Vec<ParamDesc>
}

pub trait System {
    /// This function takes a self parameter to make the `System` trait dyn-compatible.
    fn desc(&self) -> SystemDescriptor;
    fn call(&mut self);
}

pub trait ParametrizedSystem<G: ParamGroup>: Sized {
    fn into_container(self, id: usize) -> FnContainer<G, Self> {
        FnContainer {
            id,
            system: self,
            state: G::init(),
            _marker: PhantomData
        }
    }

    fn call(&self, state: &mut G::State);
}

pub struct FnContainer<P: ParamGroup, F: ParametrizedSystem<P>> {
    pub id: usize,
    pub system: F,
    pub state: P::State,
    pub _marker: PhantomData<P>
}

#[derive(Default)]
pub struct Systems {
    storage: Vec<Box<dyn System>>
}

impl<P: Param, F: ParametrizedSystem<P>> System for FnContainer<P, F> {
    fn desc(&self) -> SystemDescriptor {
        SystemDescriptor {
            id: self.id,
            deps: vec![P::desc()]
        }
    }

    fn call(&mut self) {
        self.system.call(&mut self.state);
    }
}

impl<P1: Param, P2: Param, F: ParametrizedSystem<(P1, P2)>> System for FnContainer<(P1, P2), F> {
    fn desc(&self) -> SystemDescriptor {
        SystemDescriptor {
            id: self.id,
            deps: vec![P1::desc(), P2::desc()]
        }
    }
    
    fn call(&mut self) {
        self.system.call(&mut self.state);
    }
}

impl<F: Fn(P), P: Param> ParametrizedSystem<P> for F {
    fn call(&self, state: &mut P::State) {
        let p = P::fetch::<Sealer>(state);
        self(p);
    }
}

impl<F: Fn(P1, P2), P1: Param, P2: Param> ParametrizedSystem<(P1, P2)> for F {
    fn call(&self, state: &mut <(P1, P2) as ParamGroup>::State) {
        let p1 = P1::fetch::<Sealer>(&mut state.0);
        let p2 = P2::fetch::<Sealer>(&mut state.1);

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

    pub fn push<P: ParamGroup, S: ParametrizedSystem<P> + 'static>(&mut self, system: S) where FnContainer<P, S>: System {
        self.storage.push(Box::new(system.into_container(0)));
    }

    pub fn call(&mut self) {
        for sys in &mut self.storage {
            sys.call();
        }
    }
}