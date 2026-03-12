use std::ops::Add;
use std::mem::MaybeUninit;
use generic_array::{ArrayLength, GenericArray};
use generic_array::typenum::{U0, FoldAdd, Unsigned};

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
    type AccessCount: ArrayLength + Add;

    type State;
    type Output<'w>;

    fn access(world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount>;

    #[doc(hidden)]
    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut Self::State) -> Self::Output<'w>;

    fn init(world: &mut World) -> Self::State;
}

pub unsafe trait ParamBundle {
    type AccessCount: ArrayLength;

    type State;

    fn access(world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount>;
    fn init(world: &mut World) -> Self::State;
}

unsafe impl ParamBundle for () {
    type AccessCount = U0;

    type State = ();

    fn access(_world: &mut World) -> GenericArray<AccessDesc, U0> {
        // Safety:
        // This is safe because the array has no items and therefore does not require initialization.
        // I use this method instead of `GenericArray::default` because `AccessDesc` does not
        // implement `Default` and the other methods either include heap allocation or iterators.
        unsafe { GenericArray::assume_init(GenericArray::uninit()) }
    }

    fn init(_world: &mut World) {}
}

macro_rules! create_tarray {
    ($head:ty) => {
        generic_array::typenum::TArr<$head, generic_array::typenum::ATerm>
    };
    ($head:ty, $($tail:ty),*) => {
        generic_array::typenum::TArr<$head, create_tarray!($($tail),*)>
    }
}

macro_rules! impl_bundle {
    ($count:expr, $($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens)]
            unsafe impl<$($gen: Param),*> ParamBundle for ($($gen),*)
            where
                create_tarray!($($gen::AccessCount),*): FoldAdd,
                <create_tarray!($($gen::AccessCount),*) as FoldAdd>::Output: ArrayLength
            {
                type AccessCount = <create_tarray!($($gen::AccessCount),*) as FoldAdd>::Output;
                type State = ($($gen::State),*);

                #[allow(unused)]
                fn access(world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount> {
                    let mut array = MaybeUninit::<GenericArray<AccessDesc, Self::AccessCount>>::uninit();

                    let mut offset = 0;
                    let dest_ptr = array.as_mut_ptr() as *mut AccessDesc;
                    $(
                        let part = $gen::access(world);
                        unsafe {
                            std::ptr::copy_nonoverlapping(
                                part.as_ptr(),
                                dest_ptr.add(offset),
                                $gen::AccessCount::USIZE
                            );
                            offset += $gen::AccessCount::USIZE;
                        }
                    )*

                    unsafe {
                        array.assume_init()
                    }
                }

                fn init(world: &mut World) -> Self::State {
                    ($($gen::init(world)),*)
                }
            }
        }
    }
}

impl_bundle!(1, A);
impl_bundle!(2, A, B);
impl_bundle!(3, A, B, C);
impl_bundle!(4, A, B, C, D);
impl_bundle!(5, A, B, C, D, E);
impl_bundle!(6, A, B, C, D, E, F);
impl_bundle!(7, A, B, C, D, E, F, G);
impl_bundle!(8, A, B, C, D, E, F, G, H);
impl_bundle!(9, A, B, C, D, E, F, G, H, I);
impl_bundle!(10, A, B, C, D, E, F, G, H, I, J);