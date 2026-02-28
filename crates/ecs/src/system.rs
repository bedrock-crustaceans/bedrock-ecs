use futures::stream::FuturesUnordered;
use futures::StreamExt;
use parking_lot::RwLock;
use std::{
    marker::PhantomData,
    sync::Arc
};
use crate::{assert_dyn_compatible, scheduler::{SystemDescriptor, SystemParamDescriptor}, sealed, World};

pub unsafe trait System: Send + Sync {
    /// Returns a type describing the system.
    fn descriptor(&self) -> SystemDescriptor;

    /// # Safety
    ///
    /// Before running a system you must ensure that the Rust reference aliasing guarantees are upheld.
    /// Any systems requiring mutable access to a component must have unique access.
    fn call(&self, world: &Arc<World>);

    /// Runs any preparations before a system's first run.
    /// This is for example used to register all active event readers.
    fn init(&self, _world: &Arc<World>) {}

    /// Destroys any associated data
    fn destroy(&self, _world: &Arc<World>) {}
}

assert_dyn_compatible!(System);

/// Wrapper around a system function pointer to be able to store the function's params.
pub struct FnContainer<P: SystemParams, F: ParameterizedSystem<P>> {
    pub id: usize,
    pub system: F,
    pub state: P::ArcState,
    pub _marker: PhantomData<P>,
}

pub trait SystemParams {
    type ArcState: Send + Sync;

    fn state(world: &Arc<World>) -> Self::ArcState;
}

impl<P: SystemParam> SystemParams for P {
    type ArcState = Arc<P::State>;

    fn state(world: &Arc<World>) -> Self::ArcState {
        P::state(world)
    }
}

impl<P1: SystemParam, P2: SystemParam> SystemParams for (P1, P2) {
    type ArcState = (Arc<P1::State>, Arc<P2::State>);

    fn state(world: &Arc<World>) -> Self::ArcState {
        (P1::state(world), P2::state(world))
    }
}

impl<P1, P2, P3> SystemParams for (P1, P2, P3)
where
    P1: SystemParam,
    P2: SystemParam,
    P3: SystemParam,
{
    type ArcState = (Arc<P1::State>, Arc<P2::State>, Arc<P3::State>);

    fn state(world: &Arc<World>) -> Self::ArcState {
        (P1::state(world), P2::state(world), P3::state(world))
    }
}

unsafe impl<P, F: ParameterizedSystem<P>> System for FnContainer<P, F>
where
    P: SystemParam
{
    fn descriptor(&self) -> SystemDescriptor {
        SystemDescriptor {
            id: self.id,
            deps: vec![P::descriptor()],
        }
    }

    fn call(&self, world: &Arc<World>) {
        self.system.call(world, &self.state);
    }

    fn init(&self, world: &Arc<World>) {
        P::init(world, &self.state);
    }
}

unsafe impl<P1, P2, F: ParameterizedSystem<(P1, P2)>> System for FnContainer<(P1, P2), F>
where
    P1: SystemParam,
    P2: SystemParam
{
    fn descriptor(&self) -> SystemDescriptor {
        SystemDescriptor {
            id: self.id,
            deps: vec![P1::descriptor(), P2::descriptor()],
        }
    }

    fn call(&self, world: &Arc<World>) {
        self.system.call(world, &self.state);
    }

    fn init(&self, world: &Arc<World>) {
        P1::init(world, &self.state.0);
        P2::init(world, &self.state.1);
    }
}

unsafe impl<P1, P2, P3, F: ParameterizedSystem<(P1, P2, P3)>> System
    for FnContainer<(P1, P2, P3), F>
where
    P1: SystemParam,
    P2: SystemParam,
    P3: SystemParam
{
    fn descriptor(&self) -> SystemDescriptor {
        SystemDescriptor {
            id: self.id,
            deps: vec![P1::descriptor(), P2::descriptor(), P3::descriptor()],
        }
    }

    fn call(&self, world: &Arc<World>) {
        self.system.call(world, &self.state);
    }

    fn init(&self, world: &Arc<World>) {
        P1::init(world, &self.state.0);
        P2::init(world, &self.state.1);
        P3::init(world, &self.state.2);
    }
}

pub trait SystemParam: Send + Sync {
    type State: Send + Sync;

    fn descriptor() -> SystemParamDescriptor;

    #[doc(hidden)]
    fn fetch<S: sealed::Sealed>(world: &Arc<World>, state: &Arc<Self::State>) -> Self;

    /// Creates a new state.
    fn state(world: &Arc<World>) -> Arc<Self::State>;
    /// Initializes the parameter for first-time use.
    fn init(_world: &Arc<World>, _state: &Arc<Self::State>) {}
    /// Deinitializes the parameter for when the system is removed from the ECS.
    fn destroy(_world: &Arc<World>, _state: &Arc<Self::State>) {}
}

impl SystemParam for () {
    type State = ();

    fn descriptor() -> SystemParamDescriptor {
        SystemParamDescriptor::Unit
    }

    fn fetch<S: sealed::Sealed>(_world: &Arc<World>, _state: &Arc<Self::State>) -> Self {}

    fn state(_world: &Arc<World>) -> Arc<Self::State> {
        Arc::new(())
    }
}

pub trait ParameterizedSystem<P: SystemParams>: Send + Sync + Sized {
    fn into_container(self, id: usize, world: &Arc<World>) -> FnContainer<P, Self> {
        FnContainer {
            id,
            system: self,
            state: P::state(world),
            _marker: PhantomData,
        }
    }

    fn call(&self, world: &Arc<World>, state: &P::ArcState);
}

impl<F, P> ParameterizedSystem<P> for F
where
    F: Fn(P) + Send + Sync,
    P: SystemParam
{
    fn call(&self, world: &Arc<World>, state: &Arc<P::State>) {
        let p = P::fetch::<sealed::Sealer>(world, state);
        self(p);
    }
}

impl<F, P1, P2> ParameterizedSystem<(P1, P2)> for F
where
    F: Fn(P1, P2) + Send + Sync,
    P1: SystemParam,
    P2: SystemParam
{
    fn call(&self, world: &Arc<World>, state: &<(P1, P2) as SystemParams>::ArcState) {
        let p1 = P1::fetch::<sealed::Sealer>(world, &state.0);
        let p2 = P2::fetch::<sealed::Sealer>(world, &state.1);
        self(p1, p2);
    }
}

impl<F, P1, P2, P3> ParameterizedSystem<(P1, P2, P3)> for F
where
    F: Fn(P1, P2, P3) + Send + Sync,
    P1: SystemParam,
    P2: SystemParam,
    P3: SystemParam
{
    fn call(&self, world: &Arc<World>, state: &<(P1, P2, P3) as SystemParams>::ArcState) {
        let p1 = P1::fetch::<sealed::Sealer>(world, &state.0);
        let p2 = P2::fetch::<sealed::Sealer>(world, &state.1);
        let p3 = P3::fetch::<sealed::Sealer>(world, &state.2);
        self(p1, p2, p3);
    }
}

#[derive(Default)]
pub struct Systems {
    storage: RwLock<Vec<Arc<dyn System + Send + Sync>>>,
}

impl Systems {
    pub fn new() -> Systems {
        Systems::default()
    }

    pub fn reserve(&self, n: usize) {
        self.storage.write().reserve(n);
    }

    pub fn push(&self, world: &Arc<World>, system: Arc<dyn System + Send + Sync>) {
        // Initialise system state.
        system.init(world);
        self.storage.write().push(system);
    }

    pub async fn call(&self, world: &Arc<World>) {
        let lock = self.storage.read();
        for sys_index in 0..self.storage.read().len() {
            let world = Arc::clone(world);

            lock[sys_index].call(&world);
        }
    }
}

impl Drop for Systems {
    fn drop(&mut self) {
        let lock = self.storage.write();
        for system in lock.iter() {
            todo!("run System::destroy");
        }
    }
}
