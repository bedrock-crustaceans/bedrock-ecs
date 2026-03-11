use std::{ops::{Deref, DerefMut}};

use smallvec::SmallVec;

use crate::{param::{self, Param}, sealed::Sealed, world::World};
use crate::graph::AccessDesc;

pub struct LocalState<T: Default + Send + 'static>(T);

pub struct Local<'s, T: Default + Send + 'static>(&'s mut T);

unsafe impl<'s, T: Default + Send> Param for Local<'s, T> {
    type State = LocalState<T>;
    type Output<'w> = Local<'w, T>;

    fn access(_world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        SmallVec::new()
    }

    fn init(_world: &mut World) -> LocalState<T> { LocalState(T::default())  }

    fn destroy(_state: &mut LocalState<T>) {}

    fn fetch<'w, S: Sealed>(_world: &'w World, state: &'w mut LocalState<T>) -> Local<'w, T> {
        Local(&mut state.0)
    }
}

impl<'s, T: Default + Send> Deref for Local<'s, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0
    }
}

impl<'s, T: Default + Send> DerefMut for Local<'s, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.0
    }
}