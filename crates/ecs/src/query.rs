use std::{any::TypeId, collections::HashMap, iter::FusedIterator, marker::PhantomData, ptr::NonNull};
use std::ops::Add;
use generic_array::{ArrayLength, GenericArray};
use generic_array::typenum::Unsigned;
use smallvec::{SmallVec, smallvec};

use crate::table_iterator::{ColumnIter, ColumnIterMut, EntityIter};
use crate::{archetype::{ArchetypeComponents, ArchetypeId, ArchetypeIter, Archetypes}, bitset::BitSet, component::{Component, ComponentId, ComponentRegistry}, entity::{Entity}, filter::FilterBundle, param::{self, Param}, sealed::Sealed, world::World};
use crate::graph::{AccessDesc, AccessType};

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
    type AccessCount: ArrayLength + Add;
    /// The item that the query outputs.
    type Output<'a> where Self: 'a;

    #[cfg(feature = "generics")]
    /// The iterators over the columns.
    type Iter<'a>: ChasingIterator<'a, Self> + Iterator<Item = Self::Output<'a>> where Self: 'a;

    #[cfg(not(feature = "generics"))]
    /// The iterators over the columns.
    type Iter<'a>: ChasingIterator<'a> + Iterator<Item = Self::Output<'a>> where Self: 'a;

    /// The amount of items in this bundle.
    const LEN: usize;

    fn archetype(reg: &mut ComponentRegistry) -> BitSet;

    #[cfg(feature = "generics")]
    fn access(reg: &mut ComponentRegistry) -> GenericArray<AccessDesc, Self::AccessCount>;
    #[cfg(feature = "generics")]
    fn cache_layout(lookup: &HashMap<TypeId, usize>) -> GenericArray<usize, Self::AccessCount>;
    
    #[cfg(not(feature = "generics"))]
    fn access(reg: &mut ComponentRegistry) -> SmallVec<[AccessDesc; param::INLINE_SIZE]>;
    #[cfg(not(feature = "generics"))]
    fn cache_layout(lookup: &HashMap<TypeId, usize>) -> SmallVec<[usize; param::INLINE_SIZE]>;
}

#[cfg(feature = "generics")]
pub trait ChasingIterator<'t, Q: QueryBundle>: Sized {
    fn new(world: &'t World, cache: &'t [CachedTable<Q::AccessCount>]) -> Self;
}

macro_rules! iter_is_empty {
    ($head:ident $(, $tail:expr)* $(,)?) => {
        // is_empty is unstable
        $head.len() == 0
    }
}

#[cfg(feature = "generics")]
macro_rules! impl_bundle {
    ($count:expr, $($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens)]
            pub struct [< IteratorBundle $count >]<'w, Q: QueryBundle, $($gen: ParamRef + Send),*> {
                world: &'w World,
                cache: std::slice::Iter<'w, CachedTable<Q::AccessCount>>,
                iters: ($($gen::Iter<'w>),*),
                _marker: PhantomData<&'w ($($gen),*)>
            }

            impl<'w, Q: QueryBundle, $($gen: ParamRef + Send),*> [< IteratorBundle $count >]<'w, Q, $($gen),*> {
                pub fn empty(world: &'w World) -> Self {
                    Self {
                        world,
                        cache: [].iter(),
                        iters: ($($gen::Iter::empty(world)),*),                        
                        _marker: PhantomData
                    }
                }
            }

            impl<'w, Q: QueryBundle, $($gen: ParamRef + Send),*> ChasingIterator<'w, Q> for [< IteratorBundle $count >]<'w, Q, $($gen),*> {
                fn new(world: &'w World, cache: &'w [CachedTable<Q::AccessCount>]) -> Self {
                    #[cfg(debug_assertions)]
                    {
                        for cached in cache {
                            let table = world.archetypes.table(cached.table);
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
            }

            #[allow(unused_parens)]
            impl<'t, Q: QueryBundle, $($gen: ParamRef + Send),*> Iterator for [< IteratorBundle $count >]<'t, Q, $($gen),*> {
                type Item = <($($gen),*) as QueryBundle>::Output<'t>;

                #[allow(non_snake_case, unused)]
                fn next(&mut self) -> Option<Self::Item> {
                    let ($($gen),*) = &mut self.iters;
                    if iter_is_empty!($($gen),*) {
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

                fn archetype(reg: &mut ComponentRegistry) -> BitSet {
                    let mut bitset = BitSet::new();

                    $(
                        if !$gen::IS_ENTITY {
                            let id = $gen::component_id(reg);
                            bitset.set(*id);    
                        }
                    )*
                    
                    bitset
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
                    tracing::instrument(name = "QueryBundle::cache_layout", fields(size = $count), skip_all)
                )]
                fn cache_layout(lookup: &HashMap<TypeId, usize>) -> GenericArray<usize, Self::AccessCount> {
                    GenericArray::from(
                        ($(
                            (if $gen::IS_ENTITY {
                                usize::MAX
                            } else {
                                $gen::lookup(lookup)
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
            pub struct [< IteratorBundle $count >]<'t, $($gen: ParamRef + Send),*> {
                archetypes: &'t Archetypes,
                cache: std::slice::Iter<'t, CachedTable>,
                iters: ($($gen::Iter<'t>),*),
                _marker: PhantomData<&'t ($($gen),*)>
            }

            impl<'t, $($gen: ParamRef + Send),*> QueryIterable<'t> for [< IteratorBundle $count >]<'t, $($gen),*> {
                fn new(archetypes: &'t Archetypes, cache: &'t [CachedTable]) -> Self {
                    let iters = ($(
                        $gen::iter()
                    ),*);

                    Self {
                        archetypes,
                        cache: cache.iter(),
                        iters,
                        _marker: PhantomData
                    }
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

                fn archetype(reg: &mut ComponentRegistry) -> BitSet {
                    let mut bitset = BitSet::new();

                    $(
                        if !$gen::IS_ENTITY {
                            let id = $gen::component_id(reg);
                            bitset.set(*id);
                        }
                    )*

                    bitset
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
                    tracing::instrument(name = "QueryBundle::cache_layout", fields(size = $count), skip_all)
                )]
                fn cache_layout(lookup: &HashMap<TypeId, usize>) -> SmallVec<[usize; param::INLINE_SIZE]> {
                    const COUNT: usize = (&[$( stringify!($gen) ),*] as &[&str]).len();

                    let mut cache = SmallVec::with_capacity(COUNT);
                    $(
                        if !$gen::IS_ENTITY {
                            cache.push($gen::lookup(lookup));
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
pub trait ParamIterator<'w, T>: Sized + Iterator<Item = T> + ExactSizeIterator {
    fn empty(world: &'w World) -> Self;
}

pub unsafe trait ParamRef: Send {
    type Unref: 'static;
    type Output<'w>: 'w;
    type Iter<'t>: ParamIterator<'t, Self::Output<'t>>;

    const IS_ENTITY: bool;

    fn access(reg: &mut ComponentRegistry) -> AccessDesc;
    fn component_id(reg: &mut ComponentRegistry) -> ComponentId;
    fn lookup(map: &HashMap<TypeId, usize>) -> usize;
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
            exclusive: false
        }
    }

    fn component_id(_reg: &mut ComponentRegistry) -> ComponentId {
        unreachable!("attempt to lookup component ID of entity");
    }

    fn lookup(_map: &HashMap<TypeId, usize>) -> usize {
        unimplemented!("attempt to lookup column index of entity");
    }

    fn iter<'t>(world: &'t World, _table: usize, _col: usize) -> EntityIter<'t> {
        todo!()
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
            exclusive: false 
        }
    }

    fn component_id(reg: &mut ComponentRegistry) -> ComponentId {
        reg.get_or_assign::<T>()
    }

    fn lookup(map: &HashMap<TypeId, usize>) -> usize {
        let col = *map
            .get(&TypeId::of::<T>())
            .expect(&format!("table column lookup failed for component {}", std::any::type_name::<T>()));

        col
    }

    fn iter<'t>(world: &'t World, table: usize, col: usize) -> ColumnIter<'t, T> {
        let table = world.archetypes.table(table);
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
            exclusive: true
        }
    }

    fn component_id(reg: &mut ComponentRegistry) -> ComponentId {
        reg.get_or_assign::<T>()
    }

    fn lookup(map: &HashMap<TypeId, usize>) -> usize {
        let col = *map
            .get(&TypeId::of::<T>())
            .expect(&format!("table column lookup failed for component {}", std::any::type_name::<T>()));

        col
    }

    fn iter<'t>(world: &'t World, table: usize, col: usize) -> ColumnIterMut<'t, T> {
        let table = world.archetypes.table(table);
        let col = table.column(col);

        col.iter_mut()
    }
}

pub struct Query<'w, Q: QueryBundle, F: FilterBundle = ()> {
    world: &'w World,
    cache: &'w mut QueryCache<Q, F>,
    _marker: PhantomData<(Q, F)>
}

impl<'w, Q: QueryBundle, F: FilterBundle> Query<'w, Q, F> {
    pub fn new(world: &'w World, state: &'w mut QueryCache<Q, F>) -> Query<'w, Q, F> {
        // Update the plan cache
        state.update(&world.archetypes);

        Query {
            world,
            cache: state,
            _marker: PhantomData
        }
    }

    #[inline]
    pub fn iter(&self) -> Q::Iter<'_> {
        self.cache.iter(self.world)
    }
}

unsafe impl<'placeholder, Q: QueryBundle + 'static, F: FilterBundle + 'static> Param for Query<'placeholder, Q, F> {
    #[cfg(feature = "generics")]
    type AccessCount = Q::AccessCount;
    type State = QueryCache<Q, F>;
    type Output<'w> = Query<'w, Q, F>;

    #[cfg(feature = "generics")]
    fn access(world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount> {
        Q::access(&mut world.archetypes.registry)
    }

    #[cfg(not(feature = "generics"))]
    fn access(world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        Q::access(&mut world.archetypes.registry)
    }

    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut QueryCache<Q, F>) -> Query<'w, Q, F> {
        Query::new(world, state)
    }

    fn init(world: &mut World) -> QueryCache<Q, F> {
        QueryCache::new(&mut world.archetypes)
    }
}

#[cfg(feature = "generics")]
#[derive(Debug)]
pub struct CachedTable<N: ArrayLength> {
    pub table: usize,
    pub cols: GenericArray<usize, N>
}

#[cfg(not(feature = "generics"))]
#[derive(Debug)]
pub struct CachedTable {
    /// The table that contains the components.
    pub table: usize,
    /// The columns from this table that contain the components for this query.
    pub cols: SmallVec<[usize; param::INLINE_SIZE]>
}

pub struct QueryCache<Q: QueryBundle, F: FilterBundle> {
    #[cfg(feature = "generics")]
    cached_tables: SmallVec<[CachedTable<Q::AccessCount>; 8]>,
    #[cfg(not(feature = "generics"))]
    cached_tables: SmallVec<[CachedTable; param::INLINE_SIZE]>,

    filter_state: F,
    generation: u64,
    archetype: BitSet,
    _marker: PhantomData<(Q, F)>
}

impl<Q: QueryBundle, F: FilterBundle> QueryCache<Q, F> {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "QueryCache::new", fields(query = std::any::type_name::<Q>(), filter = std::any::type_name::<F>()), skip_all)
    )]
    pub fn new(archetypes: &mut Archetypes) -> QueryCache<Q, F> {
        let archetype = Q::archetype(&mut archetypes.registry);
        let filter_state = F::init(archetypes);
        
        let mut cached_tables = SmallVec::new();
        archetypes.cache_tables::<Q, F>(&archetype, &filter_state, &mut cached_tables);

        tracing::trace!("cached {} archetype tables", cached_tables.len());

        QueryCache {
            filter_state,
            generation: archetypes.generation(),
            archetype,
            cached_tables,
            _marker: PhantomData
        }
    }

    /// Updates the cache if required.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "QueryCache::update", fields(query = std::any::type_name::<Q>(), filter = std::any::type_name::<F>()), skip_all)
    )]
    pub fn update(&mut self, archetypes: &Archetypes) {
        if self.generation != archetypes.generation() {
            self.cached_tables.clear();
            archetypes.cache_tables::<Q, F>(&self.archetype, &self.filter_state, &mut self.cached_tables);

            tracing::trace!("refreshing archetype table cache ({} -> {}), {} tables cached", self.generation, archetypes.generation(), self.cached_tables.len());

            self.generation = archetypes.generation();
        }
    }

    pub fn iter<'w>(&'w self, world: &'w World) -> Q::Iter<'w> {
        Q::Iter::new(world, &self.cached_tables)
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