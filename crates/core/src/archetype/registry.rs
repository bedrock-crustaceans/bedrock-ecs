use std::ptr::NonNull;

use nonmax::NonMaxUsize;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::archetype::{ArchetypeGraph, Signature};
use crate::component::TypeRegistry;
use crate::entity::{Entities, Entity, EntityMeta};
use crate::prelude::ComponentBundle;

#[cfg(feature = "generics")]
use crate::query::TableCache;

use crate::query::{Filter, QueryGroup};
use crate::table::{ColumnRow, Table};

use crate::util::AsConstNonNull;
#[cfg(debug_assertions)]
use crate::util::debug::BorrowEnforcer;

/// Contains all archetype tables.
///
/// An archetype is a unique combination of components.
///
/// This registry is append-only, tables cannot be removed once they have been added.
//
// Tables cannot be removed because query caching currently relies on tables only being added to skip
// checking most of the tables.
#[derive(Default)]
pub struct Archetypes {
    #[cfg(debug_assertions)]
    enforcer: BorrowEnforcer,

    /// The current archetype generation.
    /// This is used by the query cache to figure out when the cache should be updated.
    /// It is only increased when an archetype table is added or removed, not when an entity is added
    /// to a table.
    generation: u64,
    /// The component registry. This registry maps `TypeIds` to smaller unique identifiers.
    /// These smaller identifiers allow the ECS to use bitsets to represent the components that a
    /// table or query contains.
    pub(crate) component_registry: TypeRegistry,
    /// All archetype tables. These are in a vector to allow for quick access when the location is
    /// already known. Queries cache these indices and access the vector directly
    /// instead of going through the lookup map. The `lookup` table can be used to
    /// find tables in this vector.
    #[expect(
        clippy::vec_box,
        reason = "
            tables are stored inside boxes to ensure their pointers do not change after reallocation,
            this makes it safe for entities to store pointers to their tables.
        "
    )]
    tables: Vec<Box<Table>>,
    /// An array of signatures where the indices in the array correspond to indices in the table array.
    /// This is faster to iterate over than using the lookup map.
    lookup_array: Vec<Signature>,
    /// Maps archetypes to indices in the `tables` vec. This can be used if the location of a specific
    /// archetype table is unknown and only the contained components are known.
    lookup: FxHashMap<Signature, usize>,
}

impl Archetypes {
    /// Creates a new, empty archetype list.
    pub fn new() -> Archetypes {
        Archetypes::default()
    }

    /// Returns the current generation of the archetypes.
    #[inline]
    pub fn generation(&self) -> u64 {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        self.generation
    }

    /// Inserts a new table and returns a mutable reference to it.
    ///
    /// Tables are boxed to ensure they have a stable address on the heap.
    pub fn insert_table(&mut self, table: Box<Table>) -> &mut Table {
        self.generation += 1;

        self.lookup
            .insert(table.signature.clone(), self.tables.len());
        self.lookup_array.push(table.signature.clone());
        self.tables.push(table);

        self.tables.last_mut().unwrap()
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
    /// This function returns the last table that was scanned.
    ///
    /// [`Filter`]: crate::query::Filter
    /// [`CachedTable`]: crate::query::CachedTable
    #[expect(
        clippy::missing_panics_doc,
        reason = "columns should realistically never have `usize::MAX` elements"
    )]
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "Archetypes::cache_tables", skip_all)
    )]
    pub fn cache_tables<Q: QueryGroup, F: Filter>(
        &self,
        archetype: &Signature,
        start_at: NonMaxUsize,
        filter: &F,

        #[cfg(feature = "generics")] cache: &mut SmallVec<[TableCache<Q>; 8]>,
        #[cfg(not(feature = "generics"))] cache: &mut SmallVec<[TableCache; 8]>,
    ) -> NonMaxUsize {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        let iter = self.lookup_array[start_at.get()..]
            .iter()
            .enumerate()
            .filter_map(|(i, sig)| {
                let table_index = i + start_at.get();

                if sig.contains(archetype) {
                    // This table matches the queried components. We now apply all passive filters.
                    // Dynamic filters will be applied during iteration.
                    if !filter.apply_coarse(sig) {
                        return None;
                    }

                    // Found match
                    let table = &self.tables[table_index];
                    let cols = Q::get_base_ptrs(table);

                    return Some(TableCache {
                        table: table.as_const_non_null(),
                        cols,
                    });
                }

                // Not a match
                None
            });

        cache.extend(iter);
        NonMaxUsize::new(self.lookup_array.len()).expect("archetype lookup array was maximum size")
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
    #[expect(
        clippy::missing_panics_doc,
        reason = "this function does not panic since an item is inserted before unwrapping"
    )]
    pub(crate) fn spawn<B: ComponentBundle>(
        &mut self,
        handle: Entity,
        bundle: B,
        current_tick: u32,
    ) -> EntityMeta {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        let sig = B::get_or_assign_signature(&mut self.component_registry);

        // Check whether archetype already exists, otherwise create it
        // Stacked borrows is too restrictive, create a `SharedReadWrite` tag instead of a `Unique`.
        let lookup = self.lookup.get(&sig);
        let index = lookup.copied().unwrap_or_else(|| self.tables.len());

        let row = if let Some(index) = lookup {
            self.tables[*index].insert(handle, bundle, current_tick)
        } else {
            self.insert_table(Box::new(unsafe { Table::new::<B>(sig) }))
                .insert(handle, bundle, current_tick)
        };

        EntityMeta {
            handle,
            row,
            // Safety: This is safe. The pointer inside of a `Box<Table>` is guaranteed to be non-null.
            // We create a pointer directly to the
            table: NonNull::new((&raw const *self.tables[index]).cast_mut()).unwrap(),
        }
    }

    pub(crate) fn insert<B: ComponentBundle>(
        &mut self,
        current_tick: u32,
        entities: &mut Entities,
        entity: EntityMeta,
        components: B,
    ) -> bool {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write(); // This function potentially creates a new table.

        // Check whether entity is alive.
        if !entities.is_alive(entity.handle) {
            return false;
        }

        let signature = B::get_or_assign_signature(&mut self.component_registry);
        let old_table = unsafe { &mut *entity.table.as_ptr() };

        let mut combined_signature = signature.clone();
        combined_signature.union(&old_table.signature);

        let new_table = if let Some(table) = self.get_by_signature_mut(&combined_signature) {
            tracing::debug!("found existing table for insertion");
            table
        } else {
            tracing::debug!("creating new table for insertion");
            let table = unsafe { B::new_joined_table(old_table, signature) };

            // Update generation to refresh query caches
            self.generation += 1;

            self.tables.push(Box::new(table));
            self.lookup_array.push(combined_signature.clone());
            self.lookup
                .insert(combined_signature, self.tables.len() - 1);

            self.tables.last_mut().unwrap()
        };

        debug_assert_eq!(new_table.columns.len(), old_table.columns.len() + B::LEN);

        new_table.entities.push(entity.handle);
        new_table
            .entity_lookup
            .insert(entity.handle, ColumnRow(new_table.entities.len() - 1));

        // Copy over all old data
        tracing::trace!("copying over old component data");
        for column in &mut old_table.columns {
            let new_column_idx = *new_table.lookup.get(&column.ty_id()).unwrap();
            let new_column = &mut new_table.columns[new_column_idx];

            unsafe { column.copy_component(new_column, entity.row.0, current_tick) };
        }

        // Remove data from old table
        tracing::trace!("removing old component references");
        old_table.remove(entities, entity, false);

        let table_ptr = NonNull::from_mut(new_table);

        // Update metadata reference to current table.
        tracing::trace!("updating entity metadata");
        entities.set_meta(
            entity.handle.index(),
            EntityMeta {
                handle: entity.handle,
                row: ColumnRow(new_table.columns[0].len() - 1),
                table: table_ptr,
            },
        );

        // Insert new data
        tracing::trace!("inserting new component data");
        components.insert_into(new_table, current_tick);

        // Verify that all columns now have the same length
        debug_assert!(
            new_table
                .columns
                .windows(2)
                .all(|a| a[0].len() == a[1].len()),
            "not all columns have the same length after move archetypes"
        );

        true
    }

    pub(crate) fn remove<B: ComponentBundle>(
        &mut self,
        current_tick: u32,
        entities: &mut Entities,
        meta: EntityMeta,
    ) -> Option<B> {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        let removal_signature = B::get_or_assign_signature(&mut self.component_registry);

        // Safety: Since the caller has unique access to this archetype registry, it has unique access to
        // the world. This means there cannot be any other active references to this table.
        let src_table = unsafe { meta.table.as_ptr().as_mut_unchecked() };

        // Ensure that the entity actually has all these components
        if !src_table.signature.contains(&removal_signature) {
            return None;
        }

        let mut dst_signature = src_table.signature.clone();
        dst_signature.remove(&removal_signature);

        let dst_table = if let Some(dst_table) = self.get_by_signature_mut(&dst_signature) {
            tracing::trace!("existing table found after component removal");
            dst_table
        } else {
            tracing::trace!("creating new table after component removal");

            // Table not found, create new table
            let new_table = src_table.new_subset::<B>(removal_signature);
            self.insert_table(Box::new(new_table))
        };

        // Update entity metadata in destination table.
        dst_table.entities.push(meta.handle);

        let dst_row = dst_table.entities.len() - 1;

        dst_table
            .entity_lookup
            .insert(meta.handle, ColumnRow(dst_row));

        // and update entity's own metadata
        entities.set_meta(
            meta.handle.index(),
            EntityMeta {
                handle: meta.handle,
                row: ColumnRow(dst_row),
                table: NonNull::from_mut(dst_table),
            },
        );

        tracing::trace!(
            "copying over {} components and returning {}",
            dst_table.columns.len(),
            B::LEN
        );

        // Copy non-removed data to new table...
        for column in &mut dst_table.columns {
            let ty_id = column.ty_id();
            let src_column = src_table
                .get_column_by_type(&ty_id)
                .expect("table was missing one of its required columns");

            println!(
                "column has {} rows, accessing row {}",
                src_column.len(),
                meta.row.0
            );

            unsafe { src_column.copy_component(column, meta.row.0, current_tick) };
        }

        // ...and remove the components from the remaining columns to return them.
        let removed = unsafe { B::copy_from(src_table, meta.row) };

        src_table.remove(entities, meta, false);

        Some(removed)
    }

    /// Removes the components of the specified entity.
    ///
    /// # Safety
    ///
    /// This function requires that the `table` field of `meta` points to a valid table inside this archetypes container.
    pub(crate) unsafe fn despawn(&mut self, entities: &mut Entities, meta: EntityMeta) {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read(); // Archetypes remain unchanged, tables only change internally.

        // Safety: This is safe because the caller should have given a valid pointer and since
        // this function receives a mutable self, we have unique access to this table.
        let table = unsafe { meta.table.as_ptr().as_mut_unchecked() };
        table.remove(entities, meta, true);
    }

    /// Returns the amount of archetypes currently contained in this container.
    ///
    /// It does *not* return the total amount of entities, only the unique combinations of components.
    #[inline]
    pub fn len(&self) -> usize {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        self.tables.len()
    }

    /// Whether this container contains no archetypes.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Checks whether the given entity is found in any of the tables containing the archetype `A`.
    pub fn has_components<A: ComponentBundle>(&self, entity: Entity) -> bool {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        let Some(bitset) = A::try_get_signature(&self.component_registry) else {
            // If the component has not been registered, then it cannot have been spawned.
            // Hence there are no entities with this component.
            return false;
        };

        self.tables
            .iter()
            .any(|table| table.signature.contains(&bitset) && table.entities.contains(&entity))
    }

    /// Returns a table at a specific index.
    ///
    /// Use [`get_by_archetype`] or [`get_by_signature`] if the index is unknown.
    ///
    /// # Panics
    ///
    /// This function panics if the index is out of bounds.
    ///
    /// [`get_by_archetype`]: Self::get_by_archetype
    /// [`get_by_signature`]: Self::get_by_signature
    #[inline]
    pub fn get_by_index(&self, index: usize) -> &Table {
        &self.tables[index]
    }

    /// Returns a table at a specific index.
    ///
    /// Use [`get_by_archetype_mut`] or [`get_by_signature_mut`] if the index is unknown.
    ///
    /// # Panics
    ///
    /// This function panics if the index is out of bounds.
    ///
    /// [`get_by_archetype_mut`]: Self::get_by_archetype_mut
    /// [`get_by_signature_mut`]: Self::get_by_signature_mut
    #[inline]
    pub fn get_by_index_mut(&mut self, index: usize) -> &mut Table {
        &mut self.tables[index]
    }

    /// Returns a table with a specific archetype
    ///
    /// This function returns `None` if the exact archetype was not found.
    /// It only fetches exactly matching archetypes.
    /// If a table has these components but also includes additional ones, it will return `None`.
    ///
    /// Alternatives: [`get_by_signature`], [`get_by_index`].
    ///
    /// [`get_by_signature`]: Self::get_by_signature
    /// [`get_by_index`]: Self::get_by_index
    #[inline]
    pub fn get_by_archetype<T: ComponentBundle>(&self) -> Option<&Table> {
        let bitset = T::try_get_signature(&self.component_registry)?;
        self.get_by_signature(&bitset)
    }

    /// Returns a table with a specific archetype
    ///
    /// This function returns `None` if the exact archetype was not found.
    /// It only fetches exactly matching archetypes.
    /// If a table has these components but also includes additional ones, it will return `None`.
    ///
    /// Alternatives: [`get_by_signature_mut`], [`get_by_index_mut`].
    ///
    /// [`get_by_signature_mut`]: Self::get_by_signature_mut
    /// [`get_by_index_mut`]: Self::get_by_index_mut
    #[inline]
    pub fn get_by_archetype_mut<T: ComponentBundle>(&mut self) -> Option<&mut Table> {
        let bitset = T::try_get_signature(&self.component_registry)?;
        self.get_by_signature_mut(&bitset)
    }

    /// Fetches a table by its archetype signature.
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
    pub fn get_by_signature(&self, signature: &Signature) -> Option<&Table> {
        let index = self.lookup.get(signature)?;
        Some(self.get_by_index(*index))
    }

    /// Fetches a table by its archetype signature.
    ///
    /// This function returns `None` if the exact archetype was not found.
    /// It only fetches exactly matching archetypes.
    /// If a table has these components but also includes additional ones, it will return `None`.
    ///
    /// Alternatives: [`get_by_archetype_mut`], [`get_by_index_mut`].
    ///
    /// [`get_by_archetype_mut`]: Self::get_by_archetype_mut
    /// [`get_by_index_mut`]: Self::get_by_index_mut
    #[inline]
    pub fn get_by_signature_mut(&mut self, signature: &Signature) -> Option<&mut Table> {
        let index = self.lookup.get(signature)?;
        Some(self.get_by_index_mut(*index))
    }
}
