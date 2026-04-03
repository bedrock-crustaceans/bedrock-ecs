//! Implements the query iterator functionality.
//!
use std::any::TypeId;
use std::iter::FusedIterator;
use std::marker::PhantomData;

use nonmax::NonMaxUsize;

use generic_array::GenericArray;
use rustc_hash::FxHashMap;

use crate::archetype::Signature;
use crate::component::ComponentRegistry;
use crate::query::{Filter, QueryBundle, QueryData, QueryState, QueryType, TableCache};
use crate::scheduler::AccessDesc;
use crate::table::{ColumnRow, Table};
use crate::world::World;

/// The query does not care what the type is or what it can do, it just needs to access the item
/// at the given position.
pub trait RandomAccessArray {
    type Item;

    /// Returns the item at the given index.
    unsafe fn get_unchecked(&self, index: usize) -> Self::Item;

    /// The *total length* of this iterator.
    fn len(&self) -> usize;
}

/// An iterator that can jump from table to table.
///
/// These iterators usually contain multiple subiterators that iterate over the columns in each table.
#[cfg(feature = "generics")]
pub trait HoppingIterator<'t, Q: QueryBundle, F: Filter>: Sized {
    // /// Creates an iterator that returns a single item.
    // ///
    // /// This is used to implement [`Query::get`].
    // ///
    // /// [`Query::get`]: crate::query::Query::get
    // fn once(table: &'t Table, row: ColumnRow) -> Self;

    /// Creates a new iterator over the given cache.
    fn from_cache(world: &'t World, meta: &'t QueryState<Q, F>) -> Self;
}

/// An iterator that can jump from table to table.
///
/// These iterators usually contain multiple subiterators that iterate over the columns in each table.
#[cfg(not(feature = "generics"))]
pub trait HoppingIterator<'t>: Sized {
    /// Creates a new iterator over the given cache.
    fn new(world: &'t World, cache: &'t [TableCache]) -> Self;

    // /// Estimates the total amount of components remaining, including remaining tables.
    // /// This estimate does not apply filters and will therefore always overestimate.
    // ///
    // /// Note that this iterator does not implement [`ExactSizeIterator`] due to the fact that
    // /// computing the length isn't a simple operation. The query needs to look through all of the
    // /// future tables and compute their lengths. Therefore, this method has a performance cost.
    // fn estimate_len(&self) -> usize;

    /// Returns the length of the iterator of the *current* table *without* filters.
    ///
    /// A hopping iterator jumps between tables and this function returns the remaining
    /// components in the current table, *not* the total amount of components.
    fn current_len(&self) -> usize;
}

/// Returns the remaining length of the iterator.
/// Since all columns have the same length, the tail does not have to be checked.
macro_rules! get_head {
    ($head:expr $(, $tail:expr)* $(,)?) => {
        $head
    };
}

#[cfg(feature = "generics")]
macro_rules! impl_bundle {
    ($count:expr, $($gen:ident),*) => {
        paste::paste! {
            #[doc = concat!("An iterator that can iterate over ", stringify!($count), " components at a time")]
            #[allow(unused_parens)]
            pub struct [< IteratorBundle $count >]<'w, Q: QueryBundle, FA: Filter, $($gen: QueryData),*> {
                world: &'w World,
                /// The remaining cached tables that this iterator will hop to.
                cache: std::slice::Iter<'w, TableCache<Q::AccessCount>>,
                /// The subarrays that this iterator will iterate over.
                /// These do not have to be columns, anything that implements [`RandomAccessArray`]
                /// works.
                sub: ($($gen::Iter<'w, FA>),*),
                /// The current index in the subarrays.
                index: usize,
                /// The total length of the first subarray.
                ///
                /// Every subarray has the same length.
                len: usize,
                /// The current tick.
                current_tick: u32,
                /// The previous tick that this iterator was used in.
                last_tick: u32,
                /// Ensures that the type parameters live for at least `'w`.
                _marker: PhantomData<&'w ($($gen),*)>
            }

            impl<'w, Q: QueryBundle, FA: Filter, $($gen: QueryData),*> HoppingIterator<'w, Q, FA> for [< IteratorBundle $count >]<'w, Q, FA, $($gen),*> {
                fn from_cache(world: &'w World, meta: &'w QueryState<Q, FA>) -> Self {
                    #[cfg(debug_assertions)]
                    {
                        for cached in &meta.cache {
                            let table = world.archetypes.get_by_index(cached.table);
                            let cols = table.columns();
                            debug_assert!(
                                // Do not use the `len` method here it tries to acquire read access and we already have write access.
                                cols.iter().all(|c| c.len == cols[0].len),
                                "not all columns are of equal length"
                            );
                        }
                    }

                    let mut cache = meta.cache.iter();
                    let Some(first_cache) = cache.next() else {
                        // There are no cached tables, just return an empty iterator.
                        return Self::empty(world)
                    };

                    let mut counter = 0;
                    #[allow(unused)]
                    let sub = ($(
                        {
                            let it = $gen::iter(world, first_cache.table, first_cache.cols[counter], meta.last_tick, meta.current_tick);
                            counter += 1;
                            it
                        }
                    ),*);

                    let ($($gen),*) = &sub;
                    // Find the length of the first subarray. All subarrays have the same length.
                    let len = get_head!($($gen),*).len();

                    Self {
                        world,
                        cache,
                        sub,
                        index: 0,
                        len,
                        last_tick: meta.last_tick,
                        current_tick: meta.current_tick,
                        _marker: PhantomData
                    }
                }
            }

            impl<'w, Q: QueryBundle, FA: Filter, $($gen: QueryData),*> [< IteratorBundle $count >]<'w, Q, FA, $($gen),*> {
                /// Creates an empty iterator that always returns `None`. This exists because
                /// [`std::iter::empty()`] returns a concrete [`Empty`] type that is incompatible with the trait.
                ///
                /// [`Empty`]: std::iter::Empty
                pub fn empty(world: &'w World) -> Self {
                    todo!();
                    // Self {
                    //     world,
                    //     current_tick: 0,
                    //     last_tick: 0,
                    //     cache: [].iter(),
                    //     iters: ($($gen::Iter::empty(world)),*),
                    //     _marker: PhantomData
                    // }
                }

                /// The length of the full iterator if it were unfiltered.
                fn unfiltered_len(&self) -> usize {
                    let cache = self.cache.as_slice();

                    // Compute lengths of all remaining tables...
                    let full = cache.iter().map(|c| self.world.archetypes.get_by_index(c.table).len()).sum::<usize>();

                    // and add the remaining length of the current table.
                    full + self.len
                }

                /// Jumps to the next table, returning whether the jump was successful
                /// or whether the end of the query has been reached.
                fn jump(&mut self) -> bool {
                    if let Some(cache) = self.cache.next() {
                        let mut offset = 0;
                        self.sub = (
                            $(
                                {
                                    let it = $gen::iter(self.world, cache.table, cache.cols[offset], self.last_tick, self.current_tick);
                                    offset += 1;
                                    it
                                }
                            ),*
                        );

                        return true
                    }

                    false
                }

                /// Returns the next entity, while applying the query's dynamic filter.
                #[inline]
                fn next_filtered(&mut self) -> Option<<Self as Iterator>::Item> {
                    todo!()
                }

                /// Returns the next entity, without performing any filtering.
                ///
                /// This bypasses the overhead of dynamic filtering when it is not enabled.
                #[inline]
                fn next_unfiltered(&mut self) -> Option<<Self as Iterator>::Item> {
                    todo!();
                }
            }

            #[allow(unused_parens)]
            impl<'t, Q: QueryBundle, FA: Filter, $($gen: QueryData),*> Iterator for [< IteratorBundle $count >]<'t, Q, FA, $($gen),*> {
                type Item = <($($gen),*) as QueryBundle>::Output<'t>;

                #[allow(non_snake_case, unused)]
                fn next(&mut self) -> Option<Self::Item> {
                    // using `const` to force the compiler to eliminate this bounds check.
                    if const { FA::METHOD.is_dynamic() } {
                        self.next_filtered()
                    } else {
                        self.next_unfiltered()
                    }
                }

                #[inline]
                fn size_hint(&self) -> (usize, Option<usize>) {
                    let upper_bound = self.unfiltered_len();

                    if const { FA::TRIVIAL } {
                        // If this query performs no filtering, we know the exact size.
                        (upper_bound, Some(upper_bound))
                    } else {
                        // Otherwise it has a size ranging from zero to the maximum of the query.
                        (0, Some(upper_bound))
                    }
                }
            }

            impl<'t, Q: QueryBundle, $($gen: QueryData),*> ExactSizeIterator for [< IteratorBundle $count >]<'t, Q, (), $($gen),*> {
                #[inline]
                fn len(&self) -> usize {
                    self.unfiltered_len()
                }
            }

            impl<'t, Q: QueryBundle, FA: Filter, $($gen: QueryData),*> FusedIterator for [< IteratorBundle $count >]<'t, Q, FA, $($gen),*> {}

            #[allow(unused_parens)]
            #[diagnostic::do_not_recommend]
            unsafe impl<$($gen: QueryData),*> QueryBundle for ($($gen),*) {
                type AccessCount = generic_array::typenum::[< U $count >];
                type Output<'t> = ($($gen::Output<'t>),*) where
                    Self: 't,
                    ($($gen),*): 't;
                type Iter<'t, FA: Filter> = [< IteratorBundle $count >]<'t, ($($gen),*), FA, $($gen),*> where Self: 't;

                const LEN: usize = (&[$(stringify!($gen)),*] as &[&str]).len();

                fn signature(reg: &mut ComponentRegistry) -> Signature {
                    let mut sig = Signature::new();

                    $(
                        if $gen::TY == QueryType::Component {
                            let id = $gen::component_id(reg);
                            sig.set(*id);
                        }
                    )*

                    sig
                }

                fn get<'t, T: Filter>(world: &'t World, state: &'t QueryState<Self, T>, table: &'t Table, row: ColumnRow) -> Option<Self::Output<'t>> where Self: 't {
                    Some(($(
                        {
                            let col = match $gen::TY {
                                QueryType::Component => Some($gen::map_column(&table)),
                                _ => None
                            };

                            $gen::get(
                                world,
                                state,
                                table,
                                row,
                                col
                            )?
                        }
                    ),*))
                }

                #[cfg_attr(
                    feature = "tracing",
                    tracing::instrument(name = "QueryBundle::access", fields(size = $count), skip_all)
                )]
                fn access(reg: &mut ComponentRegistry) -> GenericArray<AccessDesc, Self::AccessCount> {
                    GenericArray::from(
                        ($(
                           $gen::access(reg),
                        )*)
                    )
                }

                #[cfg_attr(
                    feature = "tracing",
                    tracing::instrument(name = "QueryBundle::cache_columns", fields(size = $count), skip_all)
                )]
                fn map_columns(table: &Table) -> GenericArray<Option<NonMaxUsize>, Self::AccessCount> {
                    GenericArray::from(
                        ($(
                            (match $gen::TY {
                                QueryType::Component => Some($gen::map_column(table)),
                                QueryType::Entity | QueryType::Has => None,
                            }),
                        )*)
                    )
                }
            }
        }
    }
}

#[cfg(not(feature = "generics"))]
macro_rules! impl_bundle {
    ($count:expr, $($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens)]
            pub struct [< IteratorBundle $count >]<'w, $($gen: ParamRef + Send),*> {
                world: &'w World,
                cache: std::slice::Iter<'w, TableCache>,
                iters: ($($gen::Iter<'w>),*),
                _marker: PhantomData<&'w ($($gen),*)>
            }

            impl<'w, $($gen: ParamRef + Send),*> [< IteratorBundle $count >]<'w, $($gen),*> {
                pub fn empty(world: &'w World) -> Self {
                    Self {
                        world,
                        cache: [].iter(),
                        iters: ($($gen::Iter::empty(world)),*),
                        _marker: PhantomData
                    }
                }
            }

            impl<'w, $($gen: ParamRef + Send),*> HoppingIterator<'w> for [< IteratorBundle $count >]<'w, $($gen),*> {
                fn new(world: &'w World, cache: &'w [TableCache]) -> Self {
                    #[cfg(debug_assertions)]
                    {
                        for cached in cache {
                            let table = world.archetypes.get_by_index(cached.table);
                            let cols = table.columns();
                            debug_assert!(
                                cols.iter().all(|c| c.len() == cols[0].len()),
                                "not all columns are of equal length"
                            );
                        }
                    }

                    let mut cache = cache.iter();
                    let Some(first_cache) = cache.next() else {
                        // There are no cached tables, just return an empty iterator.
                        return Self::empty(world)
                    };

                    tracing::trace!("starting iterator at table {}", first_cache.table);

                    let mut counter = 0;
                    #[allow(unused)]
                    let iters = ($(
                        {
                            let table = first_cache.table;
                            let col = first_cache.cols[counter];

                            let it = $gen::iter(world, first_cache.table, first_cache.cols[counter]);
                            counter += 1;
                            it
                        }
                    ),*);

                    Self {
                        world,
                        cache,
                        iters,
                        _marker: PhantomData
                    }
                }

                #[allow(unused, non_snake_case)]
                fn current_len(&self) -> usize {
                    let ($($gen),*) = &self.iters;
                    iter_len!($($gen),*)
                }
            }

            #[allow(unused_parens)]
            impl<'t, $($gen: ParamRef + Send),*> Iterator for [< IteratorBundle $count >]<'t, $($gen),*> {
                type Item = <($($gen),*) as QueryBundle>::Output<'t>;

                fn next(&mut self) -> Option<Self::Item> {
                    #[allow(non_snake_case)]
                    let ($($gen),*) = &mut self.iters;

                    Some(($(
                        $gen.next()?
                    ),*))
                }
            }

            impl<'t, $($gen: ParamRef + Send),*> FusedIterator for [< IteratorBundle $count >]<'t, $($gen),*> {}

            #[allow(unused_parens)]
            #[diagnostic::do_not_recommend]
            unsafe impl<$($gen: ParamRef + Send),*> QueryBundle for ($($gen),*) {
                type Output<'t> = ($($gen::Output<'t>),*) where Self: 't;
                type Iter<'t> = [< IteratorBundle $count >]<'t, $($gen),*> where Self: 't;

                const LEN: usize = (&[$(stringify!($gen)),*] as &[&str]).len();

                fn signature(reg: &mut ComponentRegistry) -> Signature {
                    let mut sig = Signature::new();

                    $(
                        if !$gen::IS_ENTITY {
                            let id = $gen::component_id(reg);
                            sig.set(*id);
                        }
                    )*

                    sig
                }

                #[cfg_attr(
                    feature = "tracing",
                    tracing::instrument(name = "QueryBundle::access", fields(size = $count), skip_all)
                )]
                fn access(reg: &mut ComponentRegistry) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
                    smallvec![
                        $(
                            $gen::access(reg)
                        ),*
                    ]
                }

                #[cfg_attr(
                    feature = "tracing",
                    tracing::instrument(name = "QueryBundle::cache_columns", fields(size = $count), skip_all)
                )]
                fn cache_columns(table: &Table) -> SmallVec<[usize; param::INLINE_SIZE]> {
                    const COUNT: usize = (&[$( stringify!($gen) ),*] as &[&str]).len();

                    let mut cache = SmallVec::with_capacity(COUNT);
                    $(
                        if !$gen::IS_ENTITY {
                            cache.push($gen::cache_column(lookup));
                        }
                    )*
                    cache
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
