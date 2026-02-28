use std::{marker::PhantomData, ops::{Deref, DerefMut}};

use crate::{param::{Param, ParamDesc}, sealed::Sealed, world::World};

pub struct LocalState<T: Default + Send + 'static>(T);

pub struct Local<'s, T: Default + Send + 'static>(&'s mut T);

impl<'s, T: Default + Send> Param for Local<'s, T> {
    type State = LocalState<T>;
    type Item<'w> = Local<'w, T>;

    const SEND: bool = true;

    fn desc() -> ParamDesc {
        ParamDesc::Local
    }

    fn init() -> LocalState<T> { LocalState(T::default())  }

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