use smallvec::SmallVec;

use crate::{sealed::Sealed, world::World};
use crate::graph::{AccessDesc};

pub const INLINE_SIZE: usize = 8;

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

    fn access(world: &mut World) -> SmallVec<[AccessDesc; INLINE_SIZE]>;

    #[doc(hidden)]
    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut Self::State) -> Self::Output<'w>;

    fn init(world: &mut World) -> Self::State;

    fn destroy(state: &mut Self::State);
}

pub unsafe trait ParamBundle {
    type State;

    fn access(world: &mut World) -> SmallVec<[AccessDesc; INLINE_SIZE]>;
    fn init(world: &mut World) -> Self::State;
}

unsafe impl Param for () {
    type State = ();
    type Output<'w> = ();

    fn access(_world: &mut World) -> SmallVec<[AccessDesc; INLINE_SIZE]> {
        SmallVec::new()
    }

    fn fetch<'w, S: Sealed>(_world: &'w World, _state: &'w mut Self::State) -> Self::Output<'w> {}

    fn init(_world: &mut World) {}
    fn destroy(_state: &mut Self::State) {}
}

macro_rules! impl_bundle {
    ($($gen:ident),*) => {
        #[allow(unused_parens)]
        unsafe impl<$($gen: Param),*> ParamBundle for ($($gen),*) {
            type State = ($($gen::State),*);

            fn access(world: &mut World) -> SmallVec<[AccessDesc; INLINE_SIZE]> {
                let mut access = SmallVec::new();
                
                $(
                    access.extend($gen::access(world));
                )*

                access
            }

            fn init(world: &mut World) -> Self::State {
                ($($gen::init(world)),*)
            }
        }
    }
}

impl_bundle!(A);
impl_bundle!(A, B);
impl_bundle!(A, B, C);
impl_bundle!(A, B, C, D);
impl_bundle!(A, B, C, D, E);
impl_bundle!(A, B, C, D, E, F);
impl_bundle!(A, B, C, D, E, F, G);
impl_bundle!(A, B, C, D, E, F, G, H);
impl_bundle!(A, B, C, D, E, F, G, H, I);
impl_bundle!(A, B, C, D, E, F, G, H, I, J);