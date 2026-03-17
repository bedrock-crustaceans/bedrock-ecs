use std::marker::PhantomData;

use generic_array::{ArrayLength, GenericArray};
use smallvec::SmallVec;

use crate::archetype::{Archetypes, Signature};
use crate::query::{FilterBundle, HoppingIterator, QueryBundle};
use crate::scheduler::AccessDesc;
use crate::sealed::Sealed;
use crate::system::{Param, SystemMeta};
use crate::world::World;

pub struct Query<'w, Q: QueryBundle, F: FilterBundle = ()> {
    world: &'w World,
    cache: &'w mut QueryMeta<Q, F>,
}

impl<'w, Q: QueryBundle, F: FilterBundle> Query<'w, Q, F> {
    /// Creates a new query.
    ///
    /// A new query is created every time a system runs, while the
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
        Q::access(&mut world.archetypes.component_registry)
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
    signature: Signature,
    _marker: PhantomData<(Q, F)>,
}

impl<Q: QueryBundle, F: FilterBundle> QueryMeta<Q, F> {
    /// Creates a new query cache. This is only called when a system is first constructed.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "QueryCache::new", fields(query = std::any::type_name::<Q>(), filter = std::any::type_name::<F>()), skip_all)
    )]
    pub(crate) fn new(archetypes: &mut Archetypes) -> QueryMeta<Q, F> {
        let archetype = Q::signature(&mut archetypes.component_registry);
        let filter_state = F::init(archetypes);

        let mut cached = SmallVec::new();
        archetypes.cache_tables::<Q, F>(&archetype, &filter_state, &mut cached);

        tracing::trace!("cached {} archetype tables", cached.len());

        QueryMeta {
            filter_state,
            generation: archetypes.generation(),
            signature: archetype,
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
            archetypes.cache_tables::<Q, F>(&self.signature, &self.filter_state, &mut self.cache);

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
        &self.signature
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
