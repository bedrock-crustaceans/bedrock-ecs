use std::{ops::{Deref, DerefMut}};
use generic_array::GenericArray;
use generic_array::typenum::U0;

#[cfg(not(feature = "generics"))]
use smallvec::SmallVec;

use crate::{param::{self, Param}, sealed::Sealed, world::World};
use crate::graph::AccessDesc;

pub struct LocalState<T: Default + Send + 'static>(T);

/// A piece of data local to the system. This is useful to store data that persists between
/// ticks but is only used by one system.
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

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "Local::init", skip_all)
    )]
    fn init(_world: &mut World) -> LocalState<T> { 
        tracing::trace!("created internal state for `Local<{}>`", std::any::type_name::<T>());
        LocalState(T::default())  
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