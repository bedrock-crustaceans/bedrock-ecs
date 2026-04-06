use generic_array::GenericArray;
use generic_array::typenum::U0;
use std::ops::{Deref, DerefMut};

#[cfg(not(feature = "generics"))]
use smallvec::SmallVec;

#[cfg(not(feature = "generics"))]
use crate::SysArg;
use crate::scheduler::AccessDesc;

use crate::sealed::Sealed;
use crate::system::{SysArg, SystemMeta};
use crate::world::World;

/// Simple container around the state of a [`Local`].
pub struct LocalState<T: Default + Send + 'static>(T);

/// A piece of data local to the system. This is useful to store data that persists between
/// ticks but is only used by one system.
///
/// This does not overlap with any other data, and can therefore be scheduled in parallel with any other system.
pub struct Local<'s, T: Default + Send + 'static>(&'s mut T);

unsafe impl<T: Default + Send> SysArg for Local<'_, T> {
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
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; SysArg::INLINE_SIZE]> {
        SmallVec::new()
    }

    fn before_update<'w>(_world: &'w World, state: &'w mut LocalState<T>) -> Local<'w, T> {
        Local(&mut state.0)
    }

    fn after_update(_world: &World, _state: &mut Self::State) {}

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "Local::init", skip_all)
    )]
    fn init(_world: &mut World, _meta: &SystemMeta) -> LocalState<T> {
        tracing::trace!(
            "created internal state for `Local<{}>`",
            std::any::type_name::<T>()
        );
        LocalState(T::default())
    }
}

impl<T: Default + Send> Deref for Local<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0
    }
}

impl<T: Default + Send> DerefMut for Local<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.0
    }
}
