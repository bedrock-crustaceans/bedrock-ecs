use std::{
    cell::UnsafeCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use crate::{scheduler::SystemParamDescriptor, sealed, SystemParam, World};

pub struct LocalInner<S: Send + Sync + Default>(UnsafeCell<S>);

unsafe impl<S: Send + Sync + Default> Send for LocalInner<S> {}
unsafe impl<S: Send + Sync + Default> Sync for LocalInner<S> {}

pub struct Local<S: Send + Sync + Default> {
    state: Arc<LocalInner<S>>,
    _marker: PhantomData<S>,
}

impl<S: Send + Sync + Default> SystemParam for Local<S> {
    type State = LocalInner<S>;

    fn descriptor() -> SystemParamDescriptor {
        SystemParamDescriptor::State
    }

    fn fetch<T: sealed::Sealed>(_world: &Arc<World>, state: &Arc<Self::State>) -> Self {
        Local {
            state: Arc::clone(state),
            _marker: PhantomData,
        }
    }

    fn state(_world: &Arc<World>) -> Arc<Self::State> {
        Arc::new(LocalInner(UnsafeCell::new(S::default())))
    }
}

impl<S: Send + Sync + Default> Deref for Local<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        // SAFETY: A state is unique to each system and therefore can only be referenced by that singular system.
        unsafe { &*self.state.0.get() }
    }
}

impl<S: Send + Sync + Default> DerefMut for Local<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: A state is unique to each system and therefore can only be referenced by that singular system.
        unsafe { &mut *self.state.0.get() }
    }
}
