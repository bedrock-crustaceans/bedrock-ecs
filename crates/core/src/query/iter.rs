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

/// Implements all query iteration traits but cannot be instantiated.
pub struct Impossible<T> {
    _marker: PhantomData<T>,
}

impl<T> Iterator for Impossible<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        unimplemented!("cannot call `Impossible::next`");
    }
}

impl<T> ExactSizeIterator for Impossible<T> {}

impl<'w, T> EmptyableIterator<'w, T> for Impossible<T> {
    fn empty(_world: &'w World) -> Impossible<T> {
        unimplemented!("cannot create an `Impossible` iterator");
    }
}

/// Extension trait that adds the `empty` method to construct an empty iterator.
/// This is used by the query iterators when there are no more remaining components.
pub trait EmptyableIterator<'w, T>: Sized + Iterator<Item = T> + ExactSizeIterator {
    /// Creates an empty iterator that always returns `None`. This exists because
    /// [`std::iter::empty()`] returns a concrete [`Empty`] type that is incompatible with the trait.
    ///
    /// [`Empty`]: std::iter::Empty
    fn empty(world: &'w World) -> Self;
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

    // /// Estimates the total amount of components remaining, including remaining tables.
    // /// This estimate does not apply filters and will therefore always overestimate.
    // ///
    // /// Note that this iterator does not implement [`ExactSizeIterator`] due to the fact that
    // /// computing the length isn't a simple operation. The query needs to look through all of the
    // /// future tables and compute their lengths. Therefore, this method has a performance cost.
    // fn estimate_len(&self) -> usize;

    /// Returns the length of the iterator of the *current* table.
    ///
    /// A hopping iterator jumps between tables and this function returns the remaining
    /// components in the current table, *not* the total amount of components.
    fn local_len(&self) -> usize;
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
macro_rules! iter_len {
    ($head:ident $(, $tail:expr)* $(,)?) => {
        $head.len()
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
                /// The subiterators of this iterator.
                iters: ($($gen::Iter<'w, FA>),*),
                /// The current tick.
                current_tick: u32,
                /// The previous tick that this iterator was used in.
                last_tick: u32,
                /// Ensures that the type parameters live for at least `'w`.
                _marker: PhantomData<&'w ($($gen),*)>
            }

            impl<'w, Q: QueryBundle, FA: Filter, $($gen: QueryData),*> [< IteratorBundle $count >]<'w, Q, FA, $($gen),*> {
                /// Creates an empty iterator that always returns `None`. This exists because
                /// [`std::iter::empty()`] returns a concrete [`Empty`] type that is incompatible with the trait.
                ///
                /// [`Empty`]: std::iter::Empty
                pub fn empty(world: &'w World) -> Self {
                    Self {
                        world,
                        current_tick: 0,
                        last_tick: 0,
                        cache: [].iter(),
                        iters: ($($gen::Iter::empty(world)),*),
                        _marker: PhantomData
                    }
                }

                /// The length of the full iterator if it were unfiltered.
                fn unfiltered_len(&self) -> usize {
                    let cache = self.cache.as_slice();

                    // Compute lengths of all remaining tables...
                    let full = cache.iter().map(|c| self.world.archetypes.get_by_index(c.table).len()).sum::<usize>();

                    // and add the remaining length of the current table.
                    full + self.local_len()
                }
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
                    let iters = ($(
                        {
                            let it = $gen::iter(world, first_cache.table, first_cache.cols[counter], meta.last_tick, meta.current_tick);
                            counter += 1;
                            it
                        }
                    ),*);

                    Self {
                        world,
                        cache,
                        iters,
                        last_tick: meta.last_tick,
                        current_tick: meta.current_tick,
                        _marker: PhantomData
                    }
                }

                #[allow(unused, non_snake_case)]
                fn local_len(&self) -> usize {
                    let ($($gen),*) = &self.iters;
                    iter_len!($($gen),*)
                }
            }

            #[allow(unused_parens)]
            impl<'t, Q: QueryBundle, FA: Filter, $($gen: QueryData),*> Iterator for [< IteratorBundle $count >]<'t, Q, FA, $($gen),*> {
                type Item = <($($gen),*) as QueryBundle>::Output<'t>;

                #[allow(non_snake_case, unused)]
                fn next(&mut self) -> Option<Self::Item> {
                    if FA::METHOD.is_dynamic() {
                        loop {
                            let ($($gen),*) = &mut self.iters;

                            // Current iterator has ended, get the next one.
                            if iter_len!($($gen),*) == 0 {
                                // Attempt to jump to the next table in cache
                                let cache = self.cache.next()?;

                                let mut offset = 0;
                                self.iters = (
                                    $(
                                        {
                                            let it = $gen::iter(self.world, cache.table, cache.cols[offset], self.last_tick, self.current_tick);
                                            offset += 1;
                                            it
                                        }
                                    ),*
                                );
                            } else {
                                // Have to reborrow to ensure that the line above can modify `self.iters`.
                                let ($($gen),*) = &mut self.iters;
                                while iter_len!($($gen),*) > 0 {
                                    // Advance all iterators because some columns might filter while others don't. We want the indices to stay matched up
                                    let next = ($($gen.next()),*);
                                    let ($($gen),*) = next;

                                    // If all iterators returned a result, we found a new item
                                    if $($gen.is_some())&&* {
                                        return Some(($($gen.unwrap()),*))
                                    }
                                }
                            }
                        }
                    } else {
                        // Use a more efficient iterator when the query does not make use of dynamic filters.
                        let ($($gen),*) = &mut self.iters;
                        if iter_len!($($gen),*) == 0 {
                            // Attempt to jump to the next table in cache
                            let cache = self.cache.next()?;

                            let mut offset = 0;
                            self.iters = (
                                $(
                                    {
                                        let it = $gen::iter(self.world, cache.table, cache.cols[offset], self.last_tick, self.current_tick);
                                        offset += 1;
                                        it
                                    }
                                ),*
                            );
                        }

                        // Have to reborrow to ensure that the line above can modify `self.iters`.
                        let ($($gen),*) = &mut self.iters;
                        // Advance all iterators because some columns might filter while others don't. We want the indices to stay matched up
                        let next = ($($gen.next()),*);
                        let ($($gen),*) = next;

                        Some(($($gen?),*))
                    }
                }

                #[inline]
                fn size_hint(&self) -> (usize, Option<usize>) {
                    let upper_bound = self.unfiltered_len();

                    if FA::TRIVIAL {
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
