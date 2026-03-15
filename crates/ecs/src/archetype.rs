use std::{collections::{HashMap}};
use std::ops::Deref;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

#[cfg(debug_assertions)]
use crate::util::debug::RwFlag;
use crate::{Component, signature::Signature, component::{ComponentBundle, ComponentRegistry}, entity::EntityId, query::{QueryBundle, TableCache}, spawn::SpawnBundle, table::Table, util::{self}};
use crate::filter::FilterBundle;

/// Contains all archetype tables.
/// 
/// An archetype is a unique combination of components. 
#[derive(Default, Debug)]
pub struct Archetypes {
    /// The current archetype generation.
    /// This is used by the query cache to figure out when the cache should be updated.
    /// It is only increased when an archetype table is added or removed, not when an entity is added
    /// to a table.
    generation: u64,
    /// The component registry. This registry maps `TypeIds` to smaller unique identifiers.
    /// These smaller identifiers allow the ECS to use bitsets to represent the components that a
    /// table or query contains.
    pub(crate) registry: ComponentRegistry,
    /// All archetype tables. These are in a vector to allow for quick access when the location is
    /// already known. Queries cache these indices and access the vector directly
    /// instead of going through the lookup map. The `lookup` table can be used to 
    /// find tables in this vector.
    tables: Vec<Table>,
    /// An array of signatures where the indices in the array correspond to indices in the table array.
    /// This is faster to iterate over than using the lookup map.
    lookup_array: Vec<Signature>,
    /// Maps archetypes to indices in the `tables` vec. This can be used if the location of a specific
    /// archetype table is unknown and only the contained components are known.
    lookup: FxHashMap<Signature, usize>
}

impl Archetypes {
    /// Creates a new, empty archetype list.
    pub fn new() -> Archetypes {
        Archetypes::default()
    }

    /// Returns the current generation of the archetypes.
    pub fn generation(&self) -> u64 {
        self.generation
    }
    
    /// Caches the table and column locations for a specific archetype. This is used by the
    /// query cache to completely skip archetype lookups on iteration. Only when a new archetype is
    /// added or removed, is this function used to update the cache.
    /// 
    /// `archetype` should be the bitset containing the requested components, i.e. the bits of the requested
    /// components should be set to 1 and everything else set to 0.
    /// `filter` is the state of the query filter. This is used to discard archetypes that do not match the
    /// static filter. See [`Filter`] for more information about static and dynamic filters.
    /// 
    /// It takes a mutable reference to a smallvec to allow reusing existing capacity rather than
    /// allocating a new vector.
    /// 
    /// # Returns
    /// 
    /// This function returns a list of [`CachedTable`] objects. These contain the table index and
    /// the columns in the table that contain the requested components. These columns are in the same order
    /// as the components in the query.
    /// 
    /// [`Filter`]: crate::filter::Filter
    /// [`CachedTable`]: crate::query::CachedTable
    #[cfg_attr(
        feature = "tracing", 
        tracing::instrument(name = "Archetypes::cache_tables", skip_all)
    )]
    pub fn cache_tables<Q: QueryBundle, F: FilterBundle>(
        &self, archetype: &Signature, filter: &F,

        #[cfg(feature = "generics")]
        cache: &mut SmallVec<[TableCache<Q::AccessCount>; 8]>,
        #[cfg(not(feature = "generics"))]
        cache: &mut SmallVec<[TableCache; 8]>
    ) {
        cache.clear();

        let iter = self.lookup_array
            .iter()
            .enumerate()
            .filter_map(|(table_index, sig)| {
                if sig.contains(archetype) {
                    // This table matches the queried components. We now apply all passive filters.
                    // Dynamic filters will be applied during iteration.
                    if !filter.apply_static_filters(sig) {
                        return None
                    }

                    // Found match
                    let table = &self.tables[table_index];
                    let cols = Q::cache_columns(&table.lookup);

                    return Some(TableCache {
                        table: table_index,
                        cols
                    })
                }

                // Not a match
                None
            });

        cache.extend(iter);
    }

    /// Inserts a set of components into a table.
    /// 
    /// If a table for the archetype exists, it is inserted into the existing columns.
    /// Otherwise a new table is created and the generation is updated to refresh the query caches.
    /// 
    /// This function takes a [`SpawnBundle`], which is a tuple of components.
    /// 
    /// [`SpawnBundle`]: crate::spawn::SpawnBundle
    #[cfg_attr(
        feature = "tracing", 
        tracing::instrument(name = "Archetypes::insert", skip(self, bundle))
    )]
    pub fn insert<B: SpawnBundle + 'static>(&mut self, id: EntityId, bundle: B) {
        self.generation += 1;
        
        let sig = B::signature(&mut self.registry);

        // Check whether archetype already exists, otherwise create it
        let lookup = self.lookup.get(&sig);
        let table = if let Some(index) = lookup {
            &mut self.tables[*index]
        } else {
            self.lookup_array.push(sig.clone());
            self.lookup.insert(sig.clone(), self.tables.len());
            let table = Table::new::<B>(sig);
            self.tables.push(table);

            self.tables.last_mut().unwrap()
        };

        table.insert(id, bundle);
    }

    /// Returns the amount of archetypes currently contained in this container.
    /// 
    /// It does *not* return the total amount of entities, only the unique combinations of components.
    pub fn len(&self) -> usize {
        self.tables.len()
    }

    /// Checks whether the given entity is found in any of the tables containing the archetype `A`.
    pub fn has_components<A: ComponentBundle>(&self, entity: EntityId) -> bool {
        let Some(bitset) = A::get_signature(&self.registry) else {
            // If the component has not been registered, then it cannot have been spawned.
            // Hence there are no entities with this component.
            return false
        };

        self.tables.iter().any(|table| {
            table.signature.contains(&bitset) && table.entities.contains(&entity)
        })
    }

    /// Returns a table at a specific index.
    /// 
    /// Use [`get_by_archetype`] or [`get_by_bitset`] if the index is unknown.
    /// 
    /// # Panics 
    /// 
    /// This function panics if the index is out of bounds.
    /// 
    /// [`get_by_archetype`]: Self::get_by_archetype
    /// [`get_by_bitset`]: Self::get_by_bitset
    #[inline]
    pub fn get_by_index(&self, index: usize) -> &Table {
        &self.tables[index]
    }

    /// Returns a table with a specific archetype
    /// 
    /// This function returns `None` if the exact archetype was not found.
    /// It only fetches exactly matching archetypes. 
    /// If a table has these components but also includes additional ones, it will return `None`.
    /// 
    /// Alternatives: [`get_by_bitset`], [`get_by_index`].
    /// 
    /// [`get_by_bitset`]: Self::get_by_bitset
    /// [`get_by_index`]: Self::get_by_index
    #[inline]
    pub fn get_by_archetype<T: ComponentBundle>(&self) -> Option<&Table> {
        let bitset = T::get_signature(&self.registry)?;
        self.get_by_bitset(&bitset)
    }

    /// Fetches a table by archetype bitset.
    /// 
    /// This function returns `None` if the exact archetype was not found.
    /// It only fetches exactly matching archetypes. 
    /// If a table has these components but also includes additional ones, it will return `None`.
    /// 
    /// Alternatives: [`get_by_archetype`], [`get_by_index`].
    /// 
    /// [`get_by_archetype`]: Self::get_by_archetype
    /// [`get_by_index`]: Self::get_by_index
    #[inline]
    pub fn get_by_bitset(&self, bitset: &Signature) -> Option<&Table> {
        let index = self.lookup.get(bitset)?;
        Some(self.get_by_index(*index))
    }
}