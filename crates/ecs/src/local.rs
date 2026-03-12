use std::{ops::{Deref, DerefMut}};
use generic_array::GenericArray;
use generic_array::typenum::U0;
use smallvec::SmallVec;

use crate::{param::{self, Param}, sealed::Sealed, world::World};
use crate::graph::AccessDesc;

pub struct LocalState<T: Default + Send + 'static>(T);

pub struct Local<'s, T: Default + Send + 'static>(&'s mut T);

unsafe impl<'s, T: Default + Send> Param for Local<'s, T> {
    #[cfg(feature = "generics")]
    type AccessCount = U0;
    type State = LocalState<T>;
    type Output<'w> = Local<'w, T>;

    #[cfg(feature = "generics")]
    fn access(_world: &mut World) -> GenericArray<AccessDesc, U0> {
        // Safety:
        // This is safe because the array has no items and therefore does not require initialization.
        // I use this method instead of `GenericArray::default` because `AccessDesc` does not
        // implement `Default` and the other methods either include heap allocation or iterators.
        unsafe { GenericArray::assume_init(GenericArray::uninit()) }
    }

    #[cfg(not(feature = "generics"))]
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        SmallVec::new()
    }

    fn fetch<'w, S: Sealed>(_world: &'w World, state: &'w mut LocalState<T>) -> Local<'w, T> {
        Local(&mut state.0)
    }

    fn init(_world: &mut World) -> LocalState<T> { LocalState(T::default())  }
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