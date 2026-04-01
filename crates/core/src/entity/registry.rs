use nonmax::NonMaxU32;

use crate::{
    entity::{Entity, EntityGeneration, EntityIndex, EntityMeta},
    table::ColumnRow,
};

#[derive(Default, Debug, Clone)]
pub struct Entities {
    next_id: u32,
    freelist: Vec<u32>,
    generations: Vec<u32>,
    dense: Vec<EntityMeta>,
    sparse: Vec<Option<NonMaxU32>>,
}

impl Entities {
    #[inline]
    pub fn new() -> Entities {
        Entities::default()
    }

    /// Allocates an entity handle but does not insert it into the registry yet.
    pub fn allocate(&mut self) -> Entity {
        // Allocate an ID for this new entity.
        if let Some(id) = self.freelist.pop() {
            self.generations[id as usize] += 1;

            let generation = self.generations[id as usize];

            tracing::trace!("spawned entity via freelist (id: {id}, generation: {generation})");
            Entity::from_index_and_generation(
                EntityIndex::from_bits(id),
                EntityGeneration::from_bits(generation),
            )
        } else {
            let id = self.next_id;
            self.next_id += 1;

            self.generations.push(0);

            Entity::from_index_and_generation(EntityIndex::from_bits(id), EntityGeneration::FIRST)
        }
    }

    /// Retrieves the metadata of the given entity.
    pub(crate) fn get_meta(&self, entity: Entity) -> Option<EntityMeta> {
        let index = entity.index().to_bits();
        let generation = entity.generation().0;

        // Check whether generation is up to date
        if *self.generations.get(index as usize)? != generation {
            return None;
        }

        let Some(dense_idx) = self.sparse.get(index as usize)? else {
            tracing::error!("`Entities::get_meta` failed on dead entity: {entity:?}");
            return None;
        };

        self.dense.get(dense_idx.get() as usize).copied()
    }

    /// Inserts the given entity metadata into the registry.
    pub(crate) fn spawn(&mut self, meta: EntityMeta) {
        tracing::trace!(
            "spawning entity with ID {}, gen {}",
            meta.handle.index().to_bits(),
            meta.handle.generation().0
        );

        let index = meta.handle.index().to_bits() as usize;

        self.dense.push(meta);

        if index >= self.sparse.len() {
            self.sparse.resize(index + 1, None);
        }

        self.sparse[index] = NonMaxU32::new(self.dense.len() as u32 - 1);
    }

    #[inline]
    pub fn despawn(&mut self, entity: Entity) {
        let _ = self.despawn_meta(entity);
    }

    /// Despawns the entity, returning its metadata.
    pub(crate) fn despawn_meta(&mut self, entity: Entity) -> Option<EntityMeta> {
        let id = entity.index().to_bits();
        let generation = entity.generation().to_bits();

        tracing::trace!("despawning id {id}, gen {generation}");

        if self.generations[id as usize] != generation {
            // This is an old entity, ignore it
            tracing::error!(
                "entity {id} is already dead, generation outdated ({} vs. {})",
                self.generations[id as usize],
                generation
            );
            return None;
        }

        let Some(dense_idx) = self.sparse[id as usize].take() else {
            tracing::error!("entity is dead, index {id} points to tombstone");
            return None;
        };

        self.freelist.push(id);

        let meta = self.dense.swap_remove(dense_idx.get() as usize);
        if dense_idx.get() as usize != self.dense.len() {
            // If it's the last element, `swap_remove` just decreases the len.
            // Because nothing moves we don't have to do anything.
            let swapped_idx = self.dense[dense_idx.get() as usize]
                .handle
                .index()
                .to_bits();
            self.sparse[swapped_idx as usize] = Some(dense_idx);
        }

        tracing::trace!(
            "entities alive now: {}. {} in freelist",
            self.dense.len(),
            self.freelist.len()
        );

        Some(meta)
    }

    /// Updates the `row` metadata of the specified entity.
    ///
    /// This method assumes the entity is up to date and does not check generations.
    pub(crate) fn set_row_meta(
        &mut self,
        entity: EntityIndex,
        row: ColumnRow,
    ) -> Option<ColumnRow> {
        let Some(dense_idx) = *self.sparse.get(entity.to_bits() as usize)? else {
            tracing::error!(
                "cannot update table row of dead entity {}",
                entity.to_bits()
            );
            return None;
        };

        let old = std::mem::replace(&mut self.dense[dense_idx.get() as usize].row, row);

        tracing::trace!(
            "update entity {} row from {} to {}",
            entity.to_bits(),
            old.0,
            row.0
        );

        Some(old)
    }

    pub(crate) fn set_meta(&mut self, entity: EntityIndex, meta: EntityMeta) -> Option<EntityMeta> {
        let Some(dense_idx) = *self.sparse.get(entity.to_bits() as usize)? else {
            tracing::error!("cannot set meta of dead entity");
            return None;
        };

        let old = std::mem::replace(&mut self.dense[dense_idx.get() as usize], meta);
        tracing::trace!(
            "update full entity {} from {:?} to {meta:?}",
            entity.to_bits(),
            old
        );

        Some(old)
    }

    #[inline]
    pub fn alive_count(&self) -> usize {
        self.sparse.len()
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        let id = entity.index().to_bits();
        let generation = entity.generation().to_bits();

        // Check whether index is in the sparse list and verify generation
        self.sparse[id as usize].is_some() && self.generations[id as usize] == generation
    }
}
