use generic_array::{ArrayLength, GenericArray};
use smallvec::{SmallVec, smallvec};
use std::ops::Add;
use std::{
    any::TypeId, collections::HashMap, iter::FusedIterator, marker::PhantomData, ptr::NonNull,
};

use rustc_hash::FxHashMap;

use crate::entity::GenerationId;
use crate::graph::{AccessDesc, AccessType};
use crate::system::SystemMeta;
use crate::table_iterator::{ColumnIter, ColumnIterMut, EntityIter};
use crate::{
    archetype::Archetypes,
    component::{Component, ComponentId, ComponentRegistry},
    entity::Entity,
    filter::FilterBundle,
    param::{self, Param},
    sealed::Sealed,
    signature::Signature,
    world::World,
};

/// A collection of types that can be queried.
///
/// This is implemented for tuples of types that implement [`ParamRef`].
/// In other words, this represents collection of component references or entities that appear
/// inside a `Query<...>`.
///
/// # Safety:
///
/// The `access` method must correctly return the types this query uses.
/// Incorrect implementation will lead to reference aliasing and inevitable UB.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid query type", 
    label = "invalid query",
    // note = "only `Entity`, `&T` and `&mut T` where `T: Component` or tuples thereof can be used in queries",
    note = "components in a query must be wrapped in a reference, e.g. `&{Self}` or `&mut {Self}`",
    note = "if `{Self}` is a component, do not forget to implement the `Component` trait"
)]
pub unsafe trait QueryBundle: Sized {
    #[cfg(feature = "generics")]
    /// The amount of resources that this query accesses.
    type AccessCount: ArrayLength + Add;

    /// The item that the query outputs. This is what is actually given to the system when ran.
    type Output<'a>
    where
        Self: 'a;

    #[cfg(feature = "generics")]
    /// The type of iterator over the columns. Every collection size has a different iterator type
    /// specialised for its size. These iterators are [`IteratorBundle1`], [`IteratorBundle2`], ...
    type Iter<'a>: HoppingIterator<'a, Self> + Iterator<Item = Self::Output<'a>>
    where
        Self: 'a;

    #[cfg(not(feature = "generics"))]
    /// The type of iterator over the columns. Every collection size has a different iterator type
    /// specialised for its size. These iterators are [`IteratorBundle1`], [`IteratorBundle2`], ...
    type Iter<'a>: HoppingIterator<'a> + Iterator<Item = Self::Output<'a>>
    where
        Self: 'a;

    /// The amount of items in this bundle.
    const LEN: usize;

    /// Returns the signature of this query. This signature does not include possible filters.
    fn signature(reg: &mut ComponentRegistry) -> Signature;

    #[cfg(feature = "generics")]
    /// A list of resources that this query wants to access. This is forwarded to the scheduler
    /// to avoid conflicts and schedule optimally.
    fn access(reg: &mut ComponentRegistry) -> GenericArray<AccessDesc, Self::AccessCount>;

    #[cfg(feature = "generics")]
    /// Finds all required columns from a lookup table.
    ///
    /// When the query cache updates, it will very quickly collect all tables that contain the desired components.
    /// It however is not aware of the columns. This function then figures out which columns are useful
    /// and in which order they should be queried.
    fn cache_columns(lookup: &FxHashMap<TypeId, usize>) -> GenericArray<usize, Self::AccessCount>;

    #[cfg(not(feature = "generics"))]
    /// A list of resources that this query wants to access. This is forwarded to the scheduler
    /// to avoid conflicts and schedule optimally.
    fn access(reg: &mut ComponentRegistry) -> SmallVec<[AccessDesc; param::INLINE_SIZE]>;

    #[cfg(not(feature = "generics"))]
    /// Finds all required columns from a lookup table.
    ///
    /// When the query cache updates, it will very quickly collect all tables that contain the desired components.
    /// It however is not aware of the columns. This function then figures out which columns are useful
    /// and in which order they should be queried.
    fn cache_columns(lookup: &FxHashMap<TypeId, usize>) -> SmallVec<[usize; param::INLINE_SIZE]>;
}

/// An iterator that can jump from table to table.
///
/// These iterators usually contain multiple subiterators that iterate over the columns in each table.
#[cfg(feature = "generics")]
pub trait HoppingIterator<'t, Q: QueryBundle>: Sized {
    /// Creates a new iterator over the given cache.
    fn new(world: &'t World, cache: &'t [TableCache<Q::AccessCount>]) -> Self;

    // /// Estimates the total amount of components remaining, including remaining tables.
    // /// This estimate does not apply filters and will therefore always overestimate.
    // ///
    // /// Note that this iterator does not implement [`ExactSizeIterator`] due to the fact that
    // /// computing the length isn't a simple operation. The query needs to look through all of the
    // /// future tables and compute their lengths. Therefore this method has a performance cost.
    // fn estimate_len(&self) -> usize;

    /// Returns the length of the iterator of the *current* table.
    ///
    /// A hopping iterator jumps between tables and this function returns the remaining
    /// components in the current table, *not* the total amount of components.
    fn current_len(&self) -> usize;
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
    // /// future tables and compute their lengths. Therefore this method has a performance cost.
    // fn estimate_len(&self) -> usize;

    /// Returns the length of the iterator of the *current* table.
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
            /// An iterator over several columns at the same time
            #[allow(unused_parens)]
            pub struct [< IteratorBundle $count >]<'w, Q: QueryBundle, $($gen: ParamRef + Send),*> {
                world: &'w World,
                /// The remaining cached tables that this iterator will hop to.
                cache: std::slice::Iter<'w, TableCache<Q::AccessCount>>,
                /// The subiterators of this iterator.
                iters: ($($gen::Iter<'w>),*),
                /// Ensures that the type parameters live for at least `'w`.
                _marker: PhantomData<&'w ($($gen),*)>
            }

            impl<'w, Q: QueryBundle, $($gen: ParamRef + Send),*> [< IteratorBundle $count >]<'w, Q, $($gen),*> {
                /// Creates an empty iterator that always returns `None`. This exists because
                /// [`std::iter::empty()`] returns a concrete [`Empty`] type that is incompatible with the trait.
                ///
                /// [`Empty`]: std::iter::Empty
                pub fn empty(world: &'w World) -> Self {
                    Self {
                        world,
                        cache: [].iter(),
                        iters: ($($gen::Iter::empty(world)),*),
                        _marker: PhantomData
                    }
                }
            }

            impl<'w, Q: QueryBundle, $($gen: ParamRef + Send),*> HoppingIterator<'w, Q> for [< IteratorBundle $count >]<'w, Q, $($gen),*> {
                fn new(world: &'w World, cache: &'w [TableCache<Q::AccessCount>]) -> Self {
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

                #[allow(unused)]
                fn current_len(&self) -> usize {
                    let ($($gen),*) = &self.iters;
                    iter_len!($($gen),*)
                }
            }

            #[allow(unused_parens)]
            impl<'t, Q: QueryBundle, $($gen: ParamRef + Send),*> Iterator for [< IteratorBundle $count >]<'t, Q, $($gen),*> {
                type Item = <($($gen),*) as QueryBundle>::Output<'t>;

                #[allow(non_snake_case, unused)]
                fn next(&mut self) -> Option<Self::Item> {
                    let ($($gen),*) = &mut self.iters;
                    if iter_len!($($gen),*) == 0 {
                        // Attempt to jump to the next table in cache
                        let cache = self.cache.next()?;

                        tracing::trace!("jumping to table {} in cache", cache.table);

                        let mut offset = 0;
                        self.iters = (
                            $(
                                {
                                    let it = $gen::iter(self.world, cache.table, cache.cols[offset]);
                                    offset += 1;
                                    it
                                }
                            ),*
                        );
                    }

                    // Have to reborrow to ensure that the line above can modify `self.iters`.
                    let ($($gen),*) = &mut self.iters;
                    Some((
                        $(
                            unsafe { $gen.next().unwrap_unchecked() }
                        ),*
                    ))
                }
            }

            impl<'t, Q: QueryBundle, $($gen: ParamRef + Send),*> FusedIterator for [< IteratorBundle $count >]<'t, Q, $($gen),*> {}

            #[allow(unused_parens)]
            #[diagnostic::do_not_recommend]
            unsafe impl<$($gen: ParamRef + Send),*> QueryBundle for ($($gen),*) {
                type AccessCount = generic_array::typenum::[< U $count >];
                type Output<'t> = ($($gen::Output<'t>),*) where Self: 't;
                type Iter<'t> = [< IteratorBundle $count >]<'t, ($($gen),*), $($gen),*> where Self: 't;

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
                fn cache_columns(lookup: &FxHashMap<TypeId, usize>) -> GenericArray<usize, Self::AccessCount> {
                    GenericArray::from(
                        ($(
                            (if $gen::IS_ENTITY {
                                usize::MAX
                            } else {
                                $gen::cache_column(lookup)
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

                #[allow(non_snake_case)]
                fn next(&mut self) -> Option<Self::Item> {
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
                fn cache_columns(lookup: &FxHashMap<TypeId, usize>) -> SmallVec<[usize; param::INLINE_SIZE]> {
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

/// Extension trait that adds the `empty` method to construct an empty iterator.
/// This is used by the query iterators when there are no more remaining components.
pub trait EmptyableIterator<'w, T>: Sized + Iterator<Item = T> + ExactSizeIterator {
    /// Creates an empty iterator that always returns `None`. This exists because
    /// [`std::iter::empty()`] returns a concrete [`Empty`] type that is incompatible with the trait.
    ///
    /// [`Empty`]: std::iter::Empty
    fn empty(world: &'w World) -> Self;
}

/// A reference that can be used in a query. This is either [`Entity`], or a mutable/immutable reference
/// to a type implementing [`Component`].
///
/// # Safety
///
/// Implementors of this trait should uphold the following conditions:
/// - `Unref` must be the exact type you would get if you were to remove the reference, i.e. if `Self = &T` then
/// `Self::Unref` must be `T`.
///
/// - `Output<'w>` must equal `Self` but with its lifetime bound to `'w`. Incorrect lifetimes will lead to use after
/// free situations.
///
/// - `Iter<'t>` must be an iterator that only returns mutable references if `Self`'s access descriptor also
/// indicates it requires mutable access.
///
/// - `IS_ENTITY` must only be set to true when implementing this trait for [`Entity`].
///
/// - `access` must return the correct descriptor, indicating which resources this parameter uses.
/// Incorrect descriptors will cause undefined behaviour through mutable reference aliasing.
///
/// - `component_id` must return the correct ID for `Self::Unref`. Incorrect component IDs will cause the query
/// cache to read the wrong columns, which means data is interpreted with the incorrect type.
///
/// [`Component`]: crate::component::Component
/// [`Entity`]: crate::entity::Entity
pub unsafe trait ParamRef: Send {
    /// The type you would get if you were to remove the reference attached to `Self`.
    type Unref: 'static;

    /// The type that is returned by the query. This is equal to `Self` but with a restricted lifetime
    /// to ensure that the queried types do not outlive the query and world itself.
    type Output<'w>: 'w;

    /// Iterator used to iterate over columns of type `Self`.
    type Iter<'t>: EmptyableIterator<'t, Self::Output<'t>>;

    /// Whether this parameter is an entity.
    const IS_ENTITY: bool;

    /// Returns the resource that this parameter accessess.
    fn access(reg: &mut ComponentRegistry) -> AccessDesc;

    /// Returns the component ID of this type.
    ///
    /// # Panics
    ///
    /// This function panics when `Self` is an entity since entities do not have a component ID.
    fn component_id(reg: &mut ComponentRegistry) -> ComponentId;

    /// Returns column index that `Self` is contained in.
    ///
    /// # Panics
    ///
    /// This function panics when `Self` is an entity since entities are not stored in columns.
    /// It also panics if the column is not found.
    fn cache_column(map: &FxHashMap<TypeId, usize>) -> usize;

    /// Returns an iterator over the column in the given table.
    ///
    /// If `Self` is an entity then this returns an iterator over the entities in the table.
    fn iter<'t>(world: &'t World, table: usize, col: usize) -> Self::Iter<'t>;
}

unsafe impl ParamRef for Entity<'_> {
    type Unref = Entity<'static>;
    type Output<'w> = Entity<'w>;
    type Iter<'t> = EntityIter<'t>;

    const IS_ENTITY: bool = true;

    fn access(_reg: &mut ComponentRegistry) -> AccessDesc {
        AccessDesc {
            ty: AccessType::Entity,
            exclusive: false,
        }
    }

    fn component_id(_reg: &mut ComponentRegistry) -> ComponentId {
        unreachable!("attempt to lookup component ID of entity");
    }

    fn cache_column(_map: &FxHashMap<TypeId, usize>) -> usize {
        unreachable!("attempt to lookup column index of entity");
    }

    fn iter(world: &World, table: usize, _col: usize) -> EntityIter {
        let table = world.archetypes.get_by_index(table);
        table.iter_entities(world)
    }
}

unsafe impl<T: Component + Send + Sync> ParamRef for &T {
    type Unref = T;
    type Output<'w> = &'w T;
    type Iter<'t> = ColumnIter<'t, T>;

    const IS_ENTITY: bool = false;

    fn access(reg: &mut ComponentRegistry) -> AccessDesc {
        AccessDesc {
            ty: AccessType::Component(reg.get_or_assign::<T>()),
            exclusive: false,
        }
    }

    fn component_id(reg: &mut ComponentRegistry) -> ComponentId {
        reg.get_or_assign::<T>()
    }

    fn cache_column(map: &FxHashMap<TypeId, usize>) -> usize {
        let col = *map.get(&TypeId::of::<T>()).expect(&format!(
            "table column lookup failed for component {}",
            std::any::type_name::<T>()
        ));

        col
    }

    fn iter(world: &World, table: usize, col: usize) -> ColumnIter<'_, T> {
        let table = world.archetypes.get_by_index(table);
        let col = table.column(col);

        col.iter()
    }
}

unsafe impl<T: Component + Send + Sync> ParamRef for &mut T {
    type Unref = T;
    type Output<'w> = &'w mut T;
    type Iter<'t> = ColumnIterMut<'t, T>;

    const IS_ENTITY: bool = false;

    fn access(reg: &mut ComponentRegistry) -> AccessDesc {
        AccessDesc {
            ty: AccessType::Component(reg.get_or_assign::<T>()),
            exclusive: true,
        }
    }

    fn component_id(reg: &mut ComponentRegistry) -> ComponentId {
        reg.get_or_assign::<T>()
    }

    fn cache_column(map: &FxHashMap<TypeId, usize>) -> usize {
        let col = *map.get(&TypeId::of::<T>()).expect(&format!(
            "table column lookup failed for component {}",
            std::any::type_name::<T>()
        ));

        col
    }

    fn iter<'t>(world: &'t World, table: usize, col: usize) -> ColumnIterMut<'t, T> {
        let table = world.archetypes.get_by_index(table);
        let col = table.column(col);

        col.iter_mut()
    }
}

pub struct Query<'w, Q: QueryBundle, F: FilterBundle = ()> {
    world: &'w World,
    cache: &'w mut QueryMeta<Q, F>,
}

impl<'w, Q: QueryBundle, F: FilterBundle> Query<'w, Q, F> {
    /// Creates a new query.
    ///
    /// A new query is created every time a system is ran while the
    /// [`QueryCache`] is persistent across runs by storing it in the system state.
    pub(crate) fn new(world: &'w World, state: &'w mut QueryMeta<Q, F>) -> Query<'w, Q, F> {
        // Update the plan cache
        state.update(&world.archetypes);

        Query {
            world,
            cache: state,
        }
    }

    /// Returns the metadata associated with this query.
    pub fn meta(&self) -> &QueryMeta<Q, F> {
        &self.cache
    }

    #[inline]
    pub fn iter(&self) -> Q::Iter<'_> {
        self.cache.iter(self.world)
    }
}

unsafe impl<Q: QueryBundle + 'static, F: FilterBundle + 'static> Param for Query<'_, Q, F> {
    #[cfg(feature = "generics")]
    type AccessCount = Q::AccessCount;
    type State = QueryMeta<Q, F>;
    type Output<'w> = Query<'w, Q, F>;

    #[cfg(feature = "generics")]
    fn access(world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount> {
        Q::access(&mut world.archetypes.registry)
    }

    #[cfg(not(feature = "generics"))]
    fn access(world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        Q::access(&mut world.archetypes.registry)
    }

    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut QueryMeta<Q, F>) -> Query<'w, Q, F> {
        Query::new(world, state)
    }

    fn init(world: &mut World, _meta: &SystemMeta) -> QueryMeta<Q, F> {
        QueryMeta::new(&mut world.archetypes)
    }
}

/// A collection of columns in a table.
#[cfg(feature = "generics")]
#[derive(Debug)]
pub struct TableCache<N: ArrayLength> {
    /// The index of the table in the archetypes container.
    pub table: usize,
    /// The columns within this table that should be queried.
    pub cols: GenericArray<usize, N>,
}

/// A collection of columns in a table.
#[cfg(not(feature = "generics"))]
#[derive(Debug)]
pub struct TableCache {
    /// The table that contains the components.
    pub table: usize,
    /// The columns from this table that contain the components for this query.
    pub cols: SmallVec<[usize; param::INLINE_SIZE]>,
}

/// The metadata of the query.
///
/// This caches the locations of desired components in the database. It also keeps track of the state of the
/// filters, if the query has any.
pub struct QueryMeta<Q: QueryBundle, F: FilterBundle> {
    #[cfg(feature = "generics")]
    cache: SmallVec<[TableCache<Q::AccessCount>; 8]>,
    #[cfg(not(feature = "generics"))]
    cache: SmallVec<[TableCache; param::INLINE_SIZE]>,

    /// The state of the filters being used by this query.
    filter_state: F,
    /// The generation of the cache. If this does not equal the archetype generation, the cache should
    /// be rebuilt.
    generation: u64,
    /// The archetype bitset of this query. This is used to quickly discard tables that do not match the query.
    archetype: Signature,
    _marker: PhantomData<(Q, F)>,
}

impl<Q: QueryBundle, F: FilterBundle> QueryMeta<Q, F> {
    /// Creates a new query cache. This is only called when a system is first constructed.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "QueryCache::new", fields(query = std::any::type_name::<Q>(), filter = std::any::type_name::<F>()), skip_all)
    )]
    pub(crate) fn new(archetypes: &mut Archetypes) -> QueryMeta<Q, F> {
        let archetype = Q::signature(&mut archetypes.registry);
        let filter_state = F::init(archetypes);

        let mut cached = SmallVec::new();
        archetypes.cache_tables::<Q, F>(&archetype, &filter_state, &mut cached);

        tracing::trace!("cached {} archetype tables", cached.len());

        QueryMeta {
            filter_state,
            generation: archetypes.generation(),
            archetype,
            cache: cached,
            _marker: PhantomData,
        }
    }

    /// Updates the cache if required. If the cache and archetype generations match, this function does
    /// nothing.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "QueryCache::update", fields(query = std::any::type_name::<Q>(), filter = std::any::type_name::<F>()), skip_all)
    )]
    pub(crate) fn update(&mut self, archetypes: &Archetypes) {
        if self.generation != archetypes.generation() {
            self.cache.clear();
            archetypes.cache_tables::<Q, F>(&self.archetype, &self.filter_state, &mut self.cache);

            tracing::trace!(
                "refreshing archetype table cache ({} -> {}), {} tables cached",
                self.generation,
                archetypes.generation(),
                self.cache.len()
            );

            self.generation = archetypes.generation();
        }
    }

    /// Creates an iterator that iterates over this cache.
    #[inline]
    pub(crate) fn iter<'w>(&'w self, world: &'w World) -> Q::Iter<'w> {
        Q::Iter::new(world, &self.cache)
    }

    /// Returns the amount of components that this query requests.
    #[inline]
    pub const fn count(&self) -> usize {
        Q::LEN
    }

    /// Returns the amount of filters this query has.
    #[inline]
    pub const fn count_filters(&self) -> usize {
        F::LEN
    }

    /// The current generation of the cache. This corresponds to the archetype generation if the cache is up
    /// to date. If the generations do not match, the cache is refreshed on the next system call.
    ///
    /// If this is accessed inside of a system, it should always be up to date.
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The archetype of this query in bitset form.
    #[inline]
    pub fn archetype(&self) -> &Signature {
        &self.archetype
    }

    /// The contents of the table cache. These are the tables and columns that the query will
    /// iterate over when calling [`Query::iter`].
    #[inline]
    #[cfg(feature = "generics")]
    pub fn cache(&self) -> &[TableCache<Q::AccessCount>] {
        &self.cache
    }

    /// The contents of the table cache. These are the tables and columns that the query will
    /// iterate over when calling [`Query::iter`].
    #[inline]
    #[cfg(not(feature = "generics"))]
    pub fn cache(&self) -> &[TableCache] {
        &self.cache
    }

    /// Returns the filter state.
    #[inline]
    pub fn filters(&self) -> &F {
        &self.filter_state
    }
}

#[diagnostic::do_not_recommend]
impl<'q, 'w, Q: QueryBundle, F: FilterBundle> IntoIterator for &'q Query<'w, Q, F> {
    type Item = Q::Output<'q>;
    type IntoIter = Q::Iter<'q>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
