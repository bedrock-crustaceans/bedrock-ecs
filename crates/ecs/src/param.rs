use crate::graph::AccessDesc;
use crate::system::SystemMeta;
use crate::{sealed::Sealed, world::World};
use generic_array::typenum::{FoldAdd, U0, Unsigned};
use generic_array::{ArrayLength, GenericArray};
use std::mem::MaybeUninit;
use std::ops::Add;

#[cfg(not(feature = "generics"))]
pub const INLINE_SIZE: usize = 8;

/// Implemented by all types that can used as parameters in a system.
///
/// # Safety
///
/// The `access` method must correctly return the types it accesses. Incorrect implementation will
/// lead to reference aliasing and immediate UB.
#[diagnostic::on_unimplemented(message = "{Self} is not a valid system parameter")]
pub unsafe trait Param {
    #[cfg(feature = "generics")]
    type AccessCount: ArrayLength + Add;

    type State;
    type Output<'w>;

    #[cfg(feature = "generics")]
    fn access(world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount>;

    #[cfg(not(feature = "generics"))]
    fn access(world: &mut World) -> SmallVec<[AccessDesc; INLINE_SIZE]>;

    #[doc(hidden)]
    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut Self::State) -> Self::Output<'w>;

    fn init(world: &mut World, meta: &SystemMeta) -> Self::State;
}

/// A collection of parameters.
///
/// A system collects all of its parameters into a tuple that implements this trait.
/// This allows easy access to all of the parameters at once.
///
/// # Safety
///
/// - `AccessCount` must return the exact amount of resources accessed by all parameters combined.
/// For example, if the bundle consists of two parameters that each access one resource, then `AccessCount` must
/// equal two. Setting this incorrectly will either cause uninitialised memory to be read, or a buffer overflow.
///
/// - `access` must correctly return each of the components that the parameters use, including
/// correct mutability information. Incorrect access descriptors will give wrong information to the scheduler
/// and cause mutable reference aliasing.
pub unsafe trait ParamBundle {
    /// The amount of resources that this parameter collection requires.
    /// This is used to compute, at compile time, how large the system metadata must be.
    #[cfg(feature = "generics")]
    type AccessCount: ArrayLength;

    /// A collection of all individual parameter states in the collection.
    type State;

    /// Returns the resources that this collection of parameters accesses.
    #[cfg(feature = "generics")]
    fn access(world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount>;

    /// Returns the resources that this collection of parameters accesses.
    #[cfg(not(feature = "generics"))]
    fn access(world: &mut World) -> SmallVec<[AccessDesc; INLINE_SIZE]>;

    /// Initializes the internal states of all parameters in this collection.
    fn init(world: &mut World, meta: &SystemMeta) -> Self::State;
}

unsafe impl ParamBundle for () {
    #[cfg(feature = "generics")]
    type AccessCount = U0;

    type State = ();

    #[cfg(feature = "generics")]
    fn access(_world: &mut World) -> GenericArray<AccessDesc, U0> {
        // Safety:
        // This is safe because the array has no items and therefore does not require initialization.
        // I use this method instead of `GenericArray::default` because `AccessDesc` does not
        // implement `Default` and the other methods either include heap allocation or iterators.
        unsafe { GenericArray::assume_init(GenericArray::uninit()) }
    }

    #[cfg(not(feature = "generics"))]
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; INLINE_SIZE]> {
        SmallVec::new()
    }

    fn init(_world: &mut World, _meta: &SystemMeta) {}
}

#[cfg(feature = "generics")]
macro_rules! impl_bundle {
    ($count:expr, $($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens)]
            unsafe impl<$($gen: Param),*> ParamBundle for ($($gen),*)
            where
                // Ensure that we can sum the array of lengths.
                crate::create_tarray!($($gen::AccessCount),*): FoldAdd,
                <crate::create_tarray!($($gen::AccessCount),*) as FoldAdd>::Output: ArrayLength
            {
                /// The total amount of resources accessed by this parameter collection.
                /// This is computed by collecting the individual counts from each parameter into a
                /// typenum array and then summing them all.
                type AccessCount = <crate::create_tarray!($($gen::AccessCount),*) as FoldAdd>::Output;

                /// The states of all the parameters, this is simply a tuple of all of them.
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

                    // Safety:
                    //
                    // This is safe because we have initialised all members of the array, assuming
                    // `Self::AccessCount` is implemented correctly, which is a condition of implementing the trait.
                    unsafe {
                        array.assume_init()
                    }
                }

                #[cfg_attr(
                    feature = "tracing",
                    tracing::instrument(name = "ParamBundle::init", skip_all)
                )]
                fn init(world: &mut World, meta: &SystemMeta) -> Self::State {
                    tracing::trace!("initialising {} system parameter state(s)", Self::AccessCount::USIZE);

                    ($($gen::init(world, meta)),*)
                }
            }
        }
    }
}

#[cfg(not(feature = "generics"))]
macro_rules! impl_bundle {
    ($count:expr, $($gen:ident),*) => {
        #[allow(unused_parens)]
        unsafe impl<$($gen: Param),*> ParamBundle for ($($gen),*) {
            type State = ($($gen::State),*);

            fn access(world: &mut World) -> SmallVec<[AccessDesc; INLINE_SIZE]> {
                let mut access = SmallVec::with_capacity($count);

                $(
                    access.extend($gen::access(world));
                )*

                access
            }

            fn init(world: &mut World, meta: &SystemMeta) -> Self::State {
                ($($gen::init(world, meta)),*)
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
