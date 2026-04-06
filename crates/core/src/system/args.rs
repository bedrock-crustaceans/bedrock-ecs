use std::mem::MaybeUninit;
use std::ops::Add;

use generic_array::typenum::{FoldAdd, U0, Unsigned};
use generic_array::{ArrayLength, GenericArray};

use crate::scheduler::AccessDesc;
use crate::sealed::Sealed;
use crate::system::SystemMeta;
use crate::world::World;

#[cfg(not(feature = "generics"))]
pub const INLINE_SIZE: usize = 8;

/// Implemented by all types that can used as system arguments in a system.
///
/// # Safety
///
/// The `access` method must correctly return the types it accesses. Incorrect implementation will
/// lead to reference aliasing and immediate UB.
#[diagnostic::on_unimplemented(message = "{Self} is not a valid system argument")]
pub unsafe trait SysArg {
    /// The amount of resources that this system argument needs to access.
    ///
    /// Most system arguments will only access one, but a query requesting `n` components will access `n` for example.
    /// One for each query.
    #[cfg(feature = "generics")]
    type AccessCount: ArrayLength + Add;

    /// The internal state of this system argument. This state is saved by the system and is persistent
    /// across calls, unlike the system argument itself.
    type State;

    /// The output of the system argument. This controls what the system receives.
    ///
    /// The main use of this is for types containing lifetimes.
    /// These will have arbitrary lifetimes when declared as system arguments in the systems. Using this output type
    /// the lifetimes will be bounded to the world.
    type Output<'w>;

    /// Declares which resources this system argument requires. This is used by the scheduler to schedule non-conflicting systems
    /// in parallel.
    #[cfg(feature = "generics")]
    fn access(world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount>;

    /// Declares which resources this system argument requires. This is used by the scheduler to schedule non-conflicting systems
    /// in parallel.
    #[cfg(not(feature = "generics"))]
    fn access(world: &mut World) -> SmallVec<[AccessDesc; INLINE_SIZE]>;

    /// Fetches this system argument. Right before executing a system, the ECS will fetch all system arguments and pass them
    /// to the system.
    fn before_update<'w>(world: &'w World, state: &'w mut Self::State) -> Self::Output<'w>;

    /// Called right after running the system. This can be used for clean up.
    fn after_update(world: &World, state: &mut Self::State);

    /// Initialises the state of this system argument.
    fn init(world: &mut World, meta: &SystemMeta) -> Self::State;
}

/// A collection of system arguments.
///
/// A system collects all of its system arguments into a tuple that implements this trait.
/// This allows easy access to all of the system arguments at once.
///
/// # Safety
///
/// - `AccessCount` must return the exact amount of resources accessed by all system arguments combined.
///   For example, if the bundle consists of two system arguments that each access one resource, then `AccessCount` must
///   equal two. Setting this incorrectly will either cause uninitialised memory to be read, or a buffer overflow.
///
/// - `access` must correctly return each of the components that the system arguments use, including
///   correct mutability information. Incorrect access descriptors will give wrong information to the scheduler
///   and cause mutable reference aliasing.
pub unsafe trait SysArgGroup {
    /// The amount of resources that this system argument collection requires.
    /// This is used to compute, at compile time, how large the system metadata must be.
    #[cfg(feature = "generics")]
    type AccessCount: ArrayLength;

    /// A collection of all individual system argument states in the collection.
    type State;

    /// Returns the resources that this collection of system arguments accesses.
    #[cfg(feature = "generics")]
    fn access(world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount>;

    /// Returns the resources that this collection of system arguments accesses.
    #[cfg(not(feature = "generics"))]
    fn access(world: &mut World) -> SmallVec<[AccessDesc; INLINE_SIZE]>;

    /// Initializes the internal states of all system arguments in this collection.
    fn init(world: &mut World, meta: &SystemMeta) -> Self::State;
}

unsafe impl SysArgGroup for () {
    #[cfg(feature = "generics")]
    type AccessCount = U0;

    type State = ();

    #[cfg(feature = "generics")]
    fn access(_world: &mut World) -> GenericArray<AccessDesc, U0> {
        // Safety:
        // This is safe because the array has no items and therefore does not require initialization.
        // I use this method instead of `GenericArray::default` because `AccessDesc` does not
        // implement `Default` and the other methods either include heap allocation or iterators.
        unsafe { GenericArray::assume_init(GenericArray::<AccessDesc, U0>::uninit()) }
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
            unsafe impl<$($gen: SysArg),*> SysArgGroup for ($($gen),*)
            where
                // Ensure that we can sum the array of lengths.
                crate::create_tarray!($($gen::AccessCount),*): FoldAdd,
                <crate::create_tarray!($($gen::AccessCount),*) as FoldAdd>::Output: ArrayLength
            {
                /// The total amount of resources accessed by this system argument collection.
                /// This is computed by collecting the individual counts from each system argument into a
                /// typenum array and then summing them all.
                type AccessCount = <crate::create_tarray!($($gen::AccessCount),*) as FoldAdd>::Output;

                /// The states of all the system arguments, this is simply a tuple of all of them.
                type State = ($($gen::State),*);

                #[allow(unused)]
                fn access(world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount> {
                    let mut array = MaybeUninit::<GenericArray<AccessDesc, Self::AccessCount>>::uninit();

                    let mut offset = 0;
                    let dest_ptr = array.as_mut_ptr().cast::<AccessDesc>();
                    $(
                        let part = $gen::access(world);

                        // Safety: This is safe because the size of `array` is the sum of all `AccessCount`s, thus
                        // the pointer will not reach out of bounds. Additionally, these pointers do not overlap since
                        // they are two different stack variables created by different means.
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
                    tracing::instrument(name = "SysArgGroup::init", skip_all)
                )]
                fn init(world: &mut World, meta: &SystemMeta) -> Self::State {
                    tracing::trace!("initialising {} system system argument state(s)", Self::AccessCount::USIZE);

                    // Run init on all system arguments in the bundle.
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
        unsafe impl<$($gen: SysArg),*> SysArgGroup for ($($gen),*) {
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
