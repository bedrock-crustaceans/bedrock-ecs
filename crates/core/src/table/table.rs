use std::any::TypeId;
use std::marker::PhantomData;

use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::archetype::Signature;
use crate::component::ComponentBundle;
use crate::entity::{Entities, Entity, EntityMeta};
use crate::query::Filter;
use crate::table::{Column, ColumnRow, EntityIter};
#[cfg(debug_assertions)]
use crate::util::debug::BorrowEnforcer;
#[cfg(debug_assertions)]
use crate::util::debug::{ReadGuard, WriteGuard};
use crate::world::World;

/// A table is the main storage container for entity components. It is made for a specific archetype only
/// and consists of a list of columns for each component.
///
/// Consider the archetype `(Health, Transform)` then its corresponding table contains
/// two columns: one for `Health` and another for `Transform`.
///
/// Table structure is immutable, i.e. once a table exists, no columns can be added to or removed from it.
#[derive(Debug)]
pub struct Table {
    #[cfg(debug_assertions)]
    pub(crate) enforcer: BorrowEnforcer,

    /// The signature of this table. This is by queries to quickly scan for their components
    /// through the entire component database.
    pub(crate) signature: Signature,
    // The `entities` and `columnns` fields are perfectly aligned, i.e.
    // an entity at index 5 in `entities` will have its components stored at row
    // 5 in the `columns` field.
    pub(crate) entities: Vec<Entity>,
    pub(crate) entity_lookup: FxHashMap<Entity, ColumnRow>,
    /// A lookup table that maps component type IDs to columns.
    pub(crate) lookup: FxHashMap<TypeId, usize>,
    /// All columns that this table contains. Most users will know exactly which column they want.
    /// Therefore, this is a vector, avoiding the cost of hashing. In case the column index is unknown,
    /// the `lookup` table can be used to find it.
    pub(crate) columns: Vec<Column>,
}

impl Table {
    /// Creates a new table with the columns of the components in `B` removed.
    ///
    /// # Panic
    ///
    /// This function panics if the components in bundle `B` are not all contained in this table.
    pub fn new_subset<B: ComponentBundle>(&self, signature: Signature) -> Self {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        assert!(
            self.signature.contains(&signature),
            "signature for `Table::new_subset` was not a subset"
        );

        let mut new_signature = self.signature.clone();
        new_signature.remove(&signature);

        let new_column_len = self.columns.len() - B::LEN;
        let mut columns = Vec::with_capacity(new_column_len);

        // Find the subset of columns that the new table needs.
        columns.extend(
            self.columns
                .iter()
                .filter_map(|c| B::contains(c.ty_id()).then(|| c.clone_empty())),
        );

        // Then create the lookup table for these columns.
        let mut lookup =
            FxHashMap::with_capacity_and_hasher(new_column_len, FxBuildHasher::default());

        lookup.extend(columns.iter().enumerate().map(|(i, c)| (c.ty_id(), i)));

        Self {
            #[cfg(debug_assertions)]
            enforcer: BorrowEnforcer::new(),

            signature: new_signature,
            entities: Vec::new(),
            entity_lookup: FxHashMap::default(),
            columns,
            lookup,
        }
    }

    /// Creates a new table for the given collection of components and inserts those components into the
    /// table.
    ///
    /// # Safety
    ///
    /// `signature` must be the actual signature of the component bundle in generic `B`.
    #[inline]
    pub unsafe fn new<B: ComponentBundle>(signature: Signature) -> Self {
        unsafe { B::new_table(signature) }
    }

    /// Returns the archetype of this table.
    pub fn archetype(&self) -> &Signature {
        // While the signature will never be written to, this function still takes a reference
        // to the entire table, hence we must uphold aliasing for the entire table.
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        &self.signature
    }

    /// Inserts a set of components into this table and returns the row it was inserted at
    pub fn insert(
        &mut self,
        entity: Entity,
        components: impl ComponentBundle,
        current_tick: u32,
    ) -> ColumnRow {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        let row = self.entities.len();
        self.entities.push(entity);
        self.entity_lookup
            .insert(entity, ColumnRow(self.entities.len() - 1));

        components.insert_into(self, current_tick);
        tracing::trace!("inserted bundle into row {row}");

        ColumnRow(row)
    }

    /// Removes the entity's data from this table and updates the entities metadata table to reflect this
    /// change.
    pub fn remove(&mut self, entities: &mut Entities, meta: EntityMeta, should_drop: bool) {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        tracing::trace!("dropping entity from table");
        tracing::debug!("table length is {}", self.columns[0].len());

        if let Some(row) = self.entity_lookup.remove(&meta.handle) {
            // Update metadata of the entity that will be moved into the current index.
            tracing::trace!(
                "update meta of entity {} (table row {})",
                meta.handle.index().to_bits(),
                row.0
            );

            // If there is only entity, there is no need to update row references.
            if self.entities.len() > 1
                && let Some(moved_entity) = self.entities.last()
            {
                // We swap remove entity A, thus entity B (at the end of the table) will be
                // moved into A's position. We update the entity meta of B to reflect this.
                //
                // If A was the last entity in the table, this code is not called.
                entities.set_row_meta(moved_entity.index(), row);
                self.entity_lookup.insert(*moved_entity, row);
            }

            // Now we swap remove.
            self.entities.swap_remove(row.0);
            self.columns
                .iter_mut()
                .for_each(|c| c.swap_remove(row.0, should_drop));
        }

        tracing::debug!("table length is now {}", self.columns[0].len());
    }

    /// Returns a list of all columns in this table.
    #[inline]
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    #[inline]
    pub fn columns_mut(&mut self) -> &mut [Column] {
        &mut self.columns
    }

    /// Returns the specified column from this table.
    ///
    /// # Panics
    ///
    /// This function panics if the index is out of range.
    #[inline]
    pub fn column(&self, index: usize) -> &Column {
        &self.columns[index]
    }

    /// Fetches the specific entity from this table.
    ///
    /// This fetches based on index within the table, *not* entity index.
    #[inline]
    pub fn get_entity(&self, index: usize) -> Option<Entity> {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        self.entities.get(index).copied()
    }

    /// Retrieves a column by its component's type ID.
    #[inline]
    pub fn get_column_by_type(&self, ty_id: &TypeId) -> Option<&Column> {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        let idx = *self.lookup.get(&ty_id)?;
        Some(&self.columns[idx])
    }

    /// Creates an iterator over all entities in this table.
    pub fn iter_entities<'w, F: Filter>(&'w self, world: &'w World) -> EntityIter<'w, F> {
        #[cfg(debug_assertions)]
        let guard = self.enforcer.read();

        EntityIter {
            tracker: todo!(),
            slice: &self.entities,
            current_tick: world.current_tick,

            _marker: PhantomData,

            #[cfg(debug_assertions)]
            _guard: Some(guard),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        self.entities.len()
    }

    /// The amount of columns in this table.
    #[inline]
    pub fn width(&self) -> usize {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        self.columns.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
