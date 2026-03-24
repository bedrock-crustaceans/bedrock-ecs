use generic_array::{ArrayLength, GenericArray};
use nonmax::NonMaxUsize;
use smallvec::SmallVec;

use crate::archetype::{Archetypes, Signature};
use crate::entity::Entity;
use crate::query::{Filter, HoppingIterator, QueryBundle};
use crate::scheduler::AccessDesc;
use crate::sealed::Sealed;
use crate::system::{Param, SystemMeta};
use crate::world::World;

/// A query is used to retrieve components from the components database.
pub struct Query<'w, Q: QueryBundle, F: Filter = ()> {
    /// The world that this query was created in.
    world: &'w World,
    /// The query's associated cache. This cache tells the query where to find its data.
    state: &'w mut QueryState<Q, F>,
}

impl<'w, Q: QueryBundle, F: Filter> Query<'w, Q, F> {
    /// Creates a new query.
    ///
    /// A new query is created every time a system runs, while the
    /// [`QueryCache`] is persistent across runs by storing it in the system state.
    pub(crate) fn new(world: &'w World, state: &'w mut QueryState<Q, F>) -> Query<'w, Q, F> {
        // Update the plan cache
        state.update(&world.archetypes);

        Query { world, state }
    }

    pub fn get(&self, entity: Entity) -> Option<Q::Output<'_>> {
        let meta = self.world.entities.get_meta(entity)?;
        println!("meta: {meta:?}");

        let table_ptr = meta.table?;

        let table = unsafe { table_ptr.as_ptr().cast_const().as_ref_unchecked() };
        Q::get::<F>(self.world, &self.state, table, meta.row)
    }

    /// Returns the metadata associated with this query.
    #[inline]
    pub fn meta(&self) -> &QueryState<Q, F> {
        self.state
    }

    /// Creates an iterator that iterates over all components matching this query.
    ///
    /// Most of the query is cached, hence the query generally does not have to perform any look ups and will immediately
    /// retrieve items from the tables.
    #[inline]
    pub fn iter(&self) -> Q::Iter<'_, F> {
        self.state.iter(self.world)
    }
}

unsafe impl<Q: QueryBundle + 'static, F: Filter + 'static> Param for Query<'_, Q, F> {
    #[cfg(feature = "generics")]
    type AccessCount = Q::AccessCount;
    type State = QueryState<Q, F>;
    type Output<'w> = Query<'w, Q, F>;

    #[cfg(feature = "generics")]
    fn access(world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount> {
        Q::access(&mut world.archetypes.component_registry)
    }

    #[cfg(not(feature = "generics"))]
    fn access(world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        Q::access(&mut world.archetypes.registry)
    }

    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut QueryState<Q, F>) -> Query<'w, Q, F> {
        // Update the tick
        state.last_tick = state.current_tick;
        state.current_tick = world.current_tick;

        Query::new(world, state)
    }

    fn init(world: &mut World, _meta: &SystemMeta) -> QueryState<Q, F> {
        QueryState::new(&mut world.archetypes, world.current_tick)
    }
}

/// A collection of columns in a table.
#[cfg(feature = "generics")]
#[derive(Debug, Clone)]
pub struct TableCache<N: ArrayLength> {
    /// The index of the table in the archetypes container.
    pub table: usize,
    /// The columns within this table that should be queried.
    pub cols: GenericArray<Option<NonMaxUsize>, N>,
}

/// A collection of columns in a table.
#[cfg(not(feature = "generics"))]
#[derive(Debug, Clone)]
pub struct TableCache {
    /// The table that contains the components.
    pub table: usize,
    /// The columns from this table that contain the components for this query.
    pub cols: SmallVec<[Option<NonMaxUsize>; param::INLINE_SIZE]>,
}

/// The metadata of the query.
///
/// This caches the locations of desired components in the database. It also keeps track of the state of the
/// filters, if the query has any.
#[derive(Debug)]
pub struct QueryState<Q: QueryBundle, F: Filter> {
    #[cfg(feature = "generics")]
    pub(crate) cache: SmallVec<[TableCache<Q::AccessCount>; 8]>,
    #[cfg(not(feature = "generics"))]
    pub(crate) cache: SmallVec<[TableCache; param::INLINE_SIZE]>,

    /// The index of the next table that should be scanned if this state is updated. We only need to check
    /// tables that have an index greater than or equal to this one.
    pub(crate) next_scan: NonMaxUsize,
    /// The current tick.
    pub(crate) current_tick: u32,
    /// The last tick that this query was used.
    pub(crate) last_tick: u32,
    /// The state of the filters being used by this query.
    pub(crate) filter_state: F,
    /// The generation of the cache. If this does not equal the archetype generation, the cache should
    /// be rebuilt.
    pub(crate) generation: u64,
    /// The archetype bitset of this query. This is used to quickly discard tables that do not match the query.
    pub(crate) signature: Signature,
}

impl<Q: QueryBundle, F: Filter> QueryState<Q, F> {
    /// Creates a new query cache. This is only called when a system is first constructed.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "QueryCache::new", fields(query = std::any::type_name::<Q>(), filter = std::any::type_name::<F>()), skip_all)
    )]
    pub(crate) fn new(archetypes: &mut Archetypes, current_tick: u32) -> QueryState<Q, F> {
        let archetype = Q::signature(&mut archetypes.component_registry);
        let filter_state = F::init(archetypes);

        let mut cached = SmallVec::new();
        let next_scan = archetypes.cache_tables::<Q, F>(
            &archetype,
            NonMaxUsize::ZERO,
            &filter_state,
            &mut cached,
        );

        tracing::trace!("cached {} archetype tables", cached.len());

        QueryState {
            current_tick,
            last_tick: 0,
            next_scan,
            filter_state,
            generation: archetypes.generation(),
            signature: archetype,
            cache: cached,
        }
    }

    /// Updates the cache and ticks.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "QueryCache::update", fields(query = std::any::type_name::<Q>(), filter = std::any::type_name::<F>()), skip_all)
    )]
    pub(crate) fn update(&mut self, archetypes: &Archetypes) {
        self.last_tick = self.current_tick;

        if self.generation != archetypes.generation() {
            // Rather than iterating over every single table again, just store the index where iteration stopped last generation
            // and only check the newly added tables. The old tables have not changed anyways.
            //
            // We should also use smallest index scanning. Keep track of the amount of archetype tables that have a certain component
            // and only check all tables
            // todo!("better table scanning");

            self.next_scan = archetypes.cache_tables::<Q, F>(
                &self.signature,
                self.next_scan,
                &self.filter_state,
                &mut self.cache,
            );

            tracing::trace!(
                "refreshing archetype table cache ({} -> {}), {} tables cached",
                self.generation,
                archetypes.generation(),
                self.cache.len()
            );

            tracing::error!("{:?}", self.cache);

            self.generation = archetypes.generation();
        }
    }

    /// Creates an iterator that iterates over this cache.
    #[inline]
    pub(crate) fn iter<'w>(&'w self, world: &'w World) -> Q::Iter<'w, F> {
        Q::Iter::from_cache(world, self)
    }

    /// Returns the amount of components that this query requests.
    #[inline]
    pub const fn data_len(&self) -> usize {
        Q::LEN
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
impl<'q, Q: QueryBundle, F: Filter> IntoIterator for &'q Query<'_, Q, F> {
    type Item = Q::Output<'q>;
    type IntoIter = Q::Iter<'q, F>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
