use std::any::TypeId;

use smallvec::SmallVec;

use crate::{sealed::Sealed, world::World};
use crate::graph::{AccessDesc, AccessType};

/// # Safety
///
/// The `access` must correctly return the types it accesses. Incorrect implementation will
/// lead to reference aliasing and immediate UB.
#[diagnostic::on_unimplemented(
    message = "{Self} is not a valid system parameter"
)]
pub unsafe trait Param {
    type State;
    type Output<'w>;

    fn access() -> Vec<AccessDesc>;

    #[doc(hidden)]
    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut Self::State) -> Self::Output<'w>;

    fn init() -> Self::State;

    fn destroy(state: &mut Self::State);
}

pub trait ParamBundle {
    type State;

    fn init() -> Self::State;
}

unsafe impl Param for () {
    type State = ();
    type Output<'w> = ();

    fn access() -> Vec<AccessDesc> {
        Vec::new()
    }

    fn fetch<'w, S: Sealed>(_world: &'w World, _state: &'w mut Self::State) -> Self::Output<'w> {}

    fn init() {}
    fn destroy(_state: &mut Self::State) {}
}

macro_rules! impl_bundle {
    ($($gen:ident),*) => {
        #[allow(unused_parens)]
        impl<$($gen: Param),*> ParamBundle for ($($gen),*) {
            type State = ($($gen::State),*);

            fn init() -> Self::State {
                ($($gen::init()),*)
            }
        }
    }
}

impl_bundle!(A);
impl_bundle!(A, B);
impl_bundle!(A, B, C);
impl_bundle!(A, B, C, D);
impl_bundle!(A, B, C, D, E);