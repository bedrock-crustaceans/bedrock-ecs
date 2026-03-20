use std::any::TypeId;
use std::cell::UnsafeCell;
use std::ptr::NonNull;

use rustc_hash::FxHashMap;

use crate::archetype::Signature;
use crate::component::SpawnBundle;
use crate::entity::{Entity, EntityHandle};
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
    pub(crate) entities: Vec<EntityHandle>,

    pub(crate) entity_lookup: FxHashMap<EntityHandle, TableRow>,
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
    #[inline]
    pub fn new<G: SpawnBundle>(bitset: Signature) -> Table {
        G::new_table(bitset)
    }

    /// Returns the archetype of this table.
    pub fn archetype(&self) -> &Signature {
        &self.signature
    }

    /// Inserts a set of components into this table and returns the row it was inserted at
    pub fn insert<G: SpawnBundle>(&mut self, entity: EntityHandle, components: G) -> TableRow {
        let row = self.entities.len();
        self.entities.push(entity);
        self.entity_lookup
            .insert(entity, TableRow(self.entities.len() - 1));

        components.insert_into(&mut self.columns);

        TableRow(row)
    }

    pub fn remove(&mut self, entity: EntityHandle) {
        if let Some(row) = self.entity_lookup.remove(&entity) {
            self.entities.swap_remove(row.0);
            self.columns.iter_mut().for_each(|c| c.swap_remove(row.0));
        }
    }

    /// Returns a list of all columns in this table.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Returns the specified column from this table.
    pub fn column(&self, index: usize) -> &Column {
        &self.columns[index]
    }

    /// Creates an iterator over all the entities in this table.
    pub fn iter_entity_refs<'w>(&'w self, world: &'w World) -> EntityRefIter<'w> {
        EntityRefIter {
            world,
            iter: self.entities.iter(),
        }
    }

    pub fn iter_entities<'w>(&'w self, world: &'w World) -> EntityIter<'w> {
        EntityIter {
            row_index: 0,
            table: NonNull::new(self as *const Table as *mut Table),
            iter: self.entities.iter(),
        }
    }

    /// Returns the amount of entities stored in this table.
    pub fn len(&self) -> usize {
        self.entities.len()
    }
}
