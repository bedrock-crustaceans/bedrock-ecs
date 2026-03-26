use std::any::TypeId;
use std::ptr::NonNull;

use rustc_hash::FxHashMap;

use crate::archetype::Signature;
use crate::component::ComponentBundle;
use crate::entity::{Entities, Entity, EntityMeta};
use crate::table::{Column, EntityIter, EntityRefIter, TableRow};
use crate::world::World;

/// A table is the main storage container for entity components. It is made for a specific archetype only
/// and consists of a list of columns for each component.
///
/// Consider the archetype `(Health, Transform)` then its corresponding table contains
/// two columns: one for `Health` and another for `Transform`.
///
/// # Safety
///
/// Tables are always to read from during a tick since entities will only be summoned in between ticks.
#[derive(Debug)]
pub struct Table {
    /// The signature of this table. This is by queries to quickly scan for their components
    /// through the entire component database.
    pub(crate) signature: Signature,
    // The `entities` and `columnns` fields are perfectly aligned, i.e.
    // an entity at index 5 in `entities` will have its components stored at row
    // 5 in the `columns` field.
    pub(crate) entities: Vec<Entity>,

    pub(crate) entity_lookup: FxHashMap<Entity, TableRow>,
    /// A lookup table that maps component type IDs to columns.
    pub(crate) lookup: FxHashMap<TypeId, usize>,
    /// All columns that this table contains. Most users will know exactly which column they want.
    /// Therefore, this is a vector, avoiding the cost of hashing. In case the column index is unknown,
    /// the `lookup` table can be used to find it.
    pub(crate) columns: Vec<Column>,
}

impl Table {
    /// Creates a new table for the given collection of components and inserts those components into the
    /// table.
    ///
    /// # Safety
    ///
    /// `signature` must be the actual signature of the component bundle in generic `B`.
    #[inline]
    pub unsafe fn new<B: ComponentBundle>(signature: Signature) -> Table {
        unsafe { B::new_table(signature) }
    }

    /// Returns the archetype of this table.
    pub fn archetype(&self) -> &Signature {
        &self.signature
    }

    /// Inserts a set of components into this table and returns the row it was inserted at
    pub fn insert(
        &mut self,
        entity: Entity,
        components: impl ComponentBundle,
        current_tick: u32,
    ) -> TableRow {
        let row = self.entities.len();
        self.entities.push(entity);
        self.entity_lookup
            .insert(entity, TableRow(self.entities.len() - 1));

        components.insert_into(self, current_tick);
        tracing::trace!("inserted bundle into row {row}");

        TableRow(row)
    }

    /// Removes the entity's data from this table and updates the entities metadata table to reflect this
    /// change.
    pub(crate) fn remove(&mut self, entities: &mut Entities, meta: EntityMeta, should_drop: bool) {
        tracing::trace!("dropping entity from table");
        tracing::debug!("table length is {}", self.columns[0].len());

        if let Some(row) = self.entity_lookup.remove(&meta.handle) {
            // Update metadata of the entity that will be moved into the current index.
            let last_index = self.entities.len() - 1;
            entities.set_row_meta(meta.handle.index(), TableRow(last_index));

            // Remove entity data
            self.entities.swap_remove(row.0);
            self.columns
                .iter_mut()
                .for_each(|c| c.swap_remove(row.0, should_drop));
        }

        tracing::debug!("table length is now {}", self.columns[0].len());
    }

    /// Returns a list of all columns in this table.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Returns the specified column from this table.
    pub fn column(&self, index: usize) -> &Column {
        &self.columns[index]
    }

    #[inline]
    pub fn get_entity(&self, index: usize) -> Option<Entity> {
        self.entities.get(index).copied()
    }

    /// Creates an iterator over all the entities in this table.
    pub fn iter_entity_refs<'w>(&'w self, world: &'w World) -> EntityRefIter<'w> {
        EntityRefIter {
            world,
            iter: self.entities.iter(),
        }
    }

    pub fn iter_entities<'w>(&'w self, _world: &'w World) -> EntityIter<'w> {
        EntityIter {
            iter: self.entities.iter(),
        }
    }

    /// Returns the amount of entities stored in this table.
    #[inline]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
