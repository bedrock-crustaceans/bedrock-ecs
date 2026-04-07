#[cfg(feature = "generics")]
use std::ptr::NonNull;

use generic_array::{ArrayLength, GenericArray};
use nonmax::NonMaxUsize;
use smallvec::SmallVec;

use crate::archetype::{Archetypes, Signature};
use crate::entity::Entity;
use crate::query::{Filter, FragmentIterator, ParallelQueryIter, QueryGroup, QueryIter};
use crate::scheduler::AccessDesc;
use crate::sealed::Sealed;
use crate::system::{SysArg, SysMeta};
#[cfg(feature = "generics")]
use crate::table::Table;
#[cfg(feature = "generics")]
use crate::util::ConstNonNull;
use crate::world::World;

/// A query is used to retrieve components from the components database.
///
/// It consists of a data and a filter part.
///
/// See the [`QueryData`] and [`Filter`] traits for all types that can be used for data and filtering respectively.
///
/// [`QueryData`]: crate::query::QueryData
/// [`Filter`]: crate::query::Filter
pub struct Query<'w, Q: QueryGroup, F: Filter = ()> {
    /// The world that this query was created in.
    world: &'w World,
    /// The query's associated cache. This cache tells the query where to find its data.
    state: &'w mut QueryState<Q, F>,
}

impl<'w, Q: QueryGroup> Query<'w, Q, ()> {
    #[inline]
    pub fn len(&self) -> usize {
        self.len_upper_bound()
    }
}

impl<'w, Q: QueryGroup, F: Filter> Query<'w, Q, F> {
    /// Creates a new query.
    ///
    /// A new query is created every time a system runs, while the
    /// [`QueryCache`] is persistent across runs by storing it in the system state.
    pub(crate) fn new(world: &'w World, state: &'w mut QueryState<Q, F>) -> Query<'w, Q, F> {
        // Update the plan cache
        state.update(&world.archetypes);

        Query { world, state }
    }

    pub fn size_hint(&self) -> (usize, Option<usize>) {
        let upper_bound = self.len_upper_bound();
        if F::TRIVIAL {
            (upper_bound, Some(upper_bound))
        } else {
            (0, Some(upper_bound))
        }
    }

    fn len_upper_bound(&self) -> usize {
        self.state
            .cache
            .iter()
            .map(|c| unsafe { &*c.table.as_ptr() }.len())
            .sum::<usize>()
    }

    /// Attempts to fetch the specified `entity` using this query.
    ///
    /// # Returns
    ///
    /// This function returns `None` if the entity did not have the requested data or
    /// was excluded by the query's filters.
    pub fn get(&self, entity: Entity) -> Option<Q::Output<'_>> {
        let meta = self.world.entities.get_meta(entity)?;
        let table = unsafe { meta.table.as_ptr().cast_const().as_ref_unchecked() };
        Q::get::<F>(self.world, self.state, table, meta.row)
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
    pub fn iter(&self) -> QueryIter<'_, Q, F> {
        QueryIter::from_state(self.state)
    }

    #[inline]
    pub fn par_iter(&self) -> ParallelQueryIter<'_, Q, F> {
        ParallelQueryIter::from_state(self.state)
    }
}

unsafe impl<Q: QueryGroup + 'static, F: Filter + 'static> SysArg for Query<'_, Q, F> {
    #[cfg(feature = "generics")]
    type AccessCount = Q::AccessCount;
    type State = QueryState<Q, F>;
    type Output<'w> = Query<'w, Q, F>;

    #[cfg(feature = "generics")]
    fn access(world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount> {
        Q::access(&mut world.archetypes.component_registry)
    }

    #[cfg(not(feature = "generics"))]
    fn access(world: &mut World) -> SmallVec<[AccessDesc; SysArg::INLINE_SIZE]> {
        Q::access(&mut world.archetypes.registry)
    }

    fn before_update<'w>(world: &'w World, state: &'w mut QueryState<Q, F>) -> Query<'w, Q, F> {
        state.current_tick = world.current_tick;

        Query::new(world, state)
    }

    fn after_update(world: &World, state: &mut QueryState<Q, F>) {
        state.last_run_tick = state.current_tick;
    }

    fn init(world: &mut World, _meta: &SysMeta) -> QueryState<Q, F> {
        QueryState::new(&mut world.archetypes, world.current_tick)
    }
}

/// A collection of columns in a table.
#[cfg(feature = "generics")]
#[derive(Clone)]
pub struct TableCache<Q: QueryGroup> {
    /// The index of the table in the archetypes container.
    pub table: ConstNonNull<Table>,
    /// The columns within this table that should be queried.
    pub cols: Q::BasePtrs,
}

/// A collection of columns in a table.
#[cfg(not(feature = "generics"))]
#[derive(Debug, Clone)]
pub struct TableCache {
    /// The table that contains the components.
    pub table: usize,
    /// The columns from this table that contain the components for this query.
    pub cols: SmallVec<[Option<NonMaxUsize>; SysArg::INLINE_SIZE]>,
}

/// The metadata of the query.
///
/// This caches the locations of desired components in the database. It also keeps track of the state of the
/// filters, if the query has any.
pub struct QueryState<Q: QueryGroup, F: Filter> {
    #[cfg(feature = "generics")]
    pub(crate) cache: SmallVec<[TableCache<Q>; 8]>,

    /// The index of the next table that should be scanned if this state is updated. We only need to check
    /// tables that have an index greater than or equal to this one.
    pub(crate) next_scan: NonMaxUsize,
    /// The current tick.
    pub(crate) current_tick: u32,
    /// The last tick that this query was used.
    pub(crate) last_run_tick: u32,
    /// The state of the filters being used by this query.
    pub(crate) filter_state: F,
    /// The generation of the cache. If this does not equal the archetype generation, the cache should
    /// be rebuilt.
    pub(crate) generation: u64,
    /// The archetype bitset of this query. This is used to quickly discard tables that do not match the query.
    pub(crate) signature: Signature,
}

impl<Q: QueryGroup, F: Filter> QueryState<Q, F> {
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
            last_run_tick: 0,
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

            self.generation = archetypes.generation();
        }
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
    pub fn cache(&self) -> &[TableCache<Q>] {
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
impl<'q, Q: QueryGroup, F: Filter> IntoIterator for &'q Query<'_, Q, F> {
    type Item = Q::Output<'q>;
    type IntoIter = QueryIter<'q, Q, F>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
