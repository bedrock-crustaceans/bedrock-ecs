use crate::{
    entity::{Entity, EntityGeneration, EntityIndex, EntityMeta},
    table::TableRow,
};

#[derive(Default, Debug, Clone)]
pub struct Entities {
    next_id: u32,
    freelist: Vec<u32>,
    generations: Vec<u32>,
    dense: Vec<EntityMeta>,
    sparse: Vec<u32>,
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
        let index = entity.index().0;
        let generation = entity.generation().0;

        // Check whether generation is up to date
        if *self.generations.get(index as usize)? != generation {
            return None;
        }

        // Safety: `generations` and `sparse` have the same length.
        let dense_idx = *self.sparse.get(index as usize)?;
        self.dense.get(dense_idx as usize).copied()
    }

    /// Inserts the given entity metadata into the registry.
    pub(crate) fn spawn(&mut self, meta: EntityMeta) {
        let index = meta.handle.index().0 as usize;

        self.dense.push(meta);

        if index >= self.sparse.len() {
            self.sparse.resize(index + 1, EntityIndex::TOMBSTONE.0);
        }
        self.sparse[index] = self.dense.len() as u32 - 1;
    }

    #[inline]
    pub fn despawn(&mut self, entity: Entity) {
        let _ = self.despawn_meta(entity);
    }

    /// Despawns the entity, returning its metadata.
    pub(crate) fn despawn_meta(&mut self, entity: Entity) -> Option<EntityMeta> {
        let id = entity.index().0;
        let generation = entity.generation().0;

        if self.generations[id as usize] != generation {
            // This is an old entity, ignore it
            tracing::trace!("attempt to despawn dead entity");
            return None;
        }

        let dense_idx = std::mem::replace(&mut self.sparse[id as usize], EntityIndex::TOMBSTONE.0);
        println!("dense_idx = {dense_idx}");
        self.freelist.push(id);

        let meta = self.dense.swap_remove(dense_idx as usize);
        if dense_idx as usize != self.dense.len() {
            // If it's the last element, `swap_remove` just decreases the len.
            // Because nothing moves we don't have to do anything.

            let swapped_idx = self.dense[dense_idx as usize].handle.index().0;
            self.sparse[swapped_idx as usize] = dense_idx;
        }

        println!("sparse: {:?}", self.sparse);

        Some(meta)
    }

    /// Updates the `row` metadata of the specified entity.
    ///
    /// This method assumes the entity is up to date and does not check generations.
    pub(crate) fn set_row_meta(&mut self, entity: EntityIndex, row: TableRow) -> Option<TableRow> {
        let dense_idx = *self.sparse.get(entity.0 as usize)?;
        if dense_idx == EntityIndex::TOMBSTONE.0 {
            tracing::warn!("Attempted to despawn entity that was already dead");
            // Entity is already dead
            return None;
        }

        Some(std::mem::replace(
            &mut self.dense[dense_idx as usize].row,
            row,
        ))
    }

    pub fn set_meta(&mut self, entity: EntityIndex, meta: EntityMeta) -> Option<EntityMeta> {
        let dense_idx = *self.sparse.get(entity.0 as usize)?;
        if dense_idx == EntityIndex::TOMBSTONE.0 {
            // Entity was dead
            return None;
        }

        Some(std::mem::replace(&mut self.dense[dense_idx as usize], meta))
    }

    #[inline]
    pub fn alive_count(&self) -> usize {
        self.sparse.len()
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        let id = entity.index().0;
        let generation = entity.generation().0;

        // Check whether index is in the sparse list and verify generation
        self.sparse[id as usize] != EntityIndex::TOMBSTONE.0
            && self.generations[id as usize] == generation
    }
}
