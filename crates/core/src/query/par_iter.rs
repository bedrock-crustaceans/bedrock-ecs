macro_rules! impl_bundle {
    ($count:literal, $($gen:ident),*) => {
        paste::paste! {
            // #[doc = concat("A parallel iterator that can iterate over ", stringify!($count), " components at a time")]
            // #[allow(unused_parens)]
            // pub struct [< ParIteratorBundle $count >]<Q: QueryGroup, FA: Filter, $($gen:QueryData),*> {
            //     world: &'w World,
            //     cache: std::slice::Iter<'w, TableCache<Q::AccessCount>>,
            //     iters
            //     current_tick: u32,
            //     last_tick: u32,
            //     _marker: PhantomData<&'w
            // }

            // #[doc = concat!("An iterator that can iterate over ", stringify!($count), " components at a time")]
            // #[allow(unused_parens)]
            // pub struct [< IteratorBundle $count >]<'w, Q: QueryGroup, FA: Filter, $($gen: QueryData),*> {
            //     world: &'w World,
            //     /// The remaining cached tables that this iterator will hop to.
            //     cache: std::slice::Iter<'w, TableCache<Q::AccessCount>>,
            //     /// The subiterators of this iterator.
            //     iters: ($($gen::Iter<'w, FA>),*),
            //     /// The current tick.
            //     current_tick: u32,
            //     /// The previous tick that this iterator was used in.
            //     last_tick: u32,
            //     /// Ensures that the type system arguments live for at least `'w`.
            //     _marker: PhantomData<&'w ($($gen),*)>
            // }

            // impl<'w, Q: QueryGroup, FA: Filter, $($gen: QueryData),*> [< IteratorBundle $count >]<'w, Q, FA, $($gen),*> {
            //     /// Creates an empty iterator that always returns `None`. This exists because
            //     /// [`std::iter::empty()`] returns a concrete [`Empty`] type that is incompatible with the trait.
            //     ///
            //     /// [`Empty`]: std::iter::Empty
            //     pub fn empty(world: &'w World) -> Self {
            //         Self {
            //             world,
            //             current_tick: 0,
            //             last_tick: 0,
            //             cache: [].iter(),
            //             iters: ($($gen::Iter::empty(world)),*),
            //             _marker: PhantomData
            //         }
            //     }
            // }
        }
    };
}
