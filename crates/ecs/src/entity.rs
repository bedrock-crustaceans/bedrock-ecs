use std::fmt;

use crate::{component::ComponentBundle, world::World};

/// Having an instance of this entity means you have exclusive access to the entire world.
pub struct EntityMut<'w> {
    pub(crate) world: &'w mut World,
    pub(crate) handle: EntityHandle,
}

impl EntityMut<'_> {
    #[inline]
    pub fn handle(&self) -> EntityHandle {
        self.handle
    }

    #[inline]
    pub fn despawn(self) {
        self.world.despawn(self.handle);
    }
}

#[cfg(debug_assertions)]
impl<'w> Drop for EntityMut<'w> {
    fn drop(&mut self) {
        self.world.flag.unlock_guardless();
    }
}

#[derive(Clone)]
pub struct Entity<'w> {
    pub(crate) world: &'w World,
    pub(crate) handle: EntityHandle,
}

impl<'w> Entity<'w> {
    /// Returns the ID of this entity.
    pub fn handle(&self) -> EntityHandle {
        self.handle
    }

    /// Checks whether this entity has all the given components.
    ///
    /// This has relatively large overhead per entity compared to queries, so prefer using queries instead.
    pub fn has<T: ComponentBundle>(&self) -> bool {
        self.world.has_components::<T>(self.handle)
    }
}

#[cfg(debug_assertions)]
impl<'w> Drop for Entity<'w> {
    fn drop(&mut self) {
        self.world.flag.unlock_guardless();
    }
}

const TOMBSTONE: u32 = u32::MAX;

#[derive(Default, Debug, Clone)]
pub struct SparseSet {
    dense: Vec<u32>,
    sparse: Vec<u32>,
}

impl SparseSet {
    #[inline]
    pub fn new() -> SparseSet {
        SparseSet::default()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn insert(&mut self, index: u32) {
        if index as usize >= self.sparse.len() {
            self.sparse.resize(index as usize + 1, TOMBSTONE);
        }

        if self.sparse[index as usize] == TOMBSTONE {
            self.dense.push(index);
            self.sparse[index as usize] = self.dense.len() as u32 - 1;
        }
    }

    pub fn remove(&mut self, index: u32) {
        let dense_index = self.sparse[index as usize];

        self.dense.swap_remove(dense_index as usize);
        self.sparse[self.dense.len()] = dense_index;
        self.sparse[index as usize] = TOMBSTONE;
    }

    #[inline]
    pub fn contains(&self, index: u32) -> bool {
        self.sparse[index as usize] != TOMBSTONE
    }
}

#[derive(Default, Debug, Clone)]
pub struct Entities {
    next_id: u32,
    freelist: Vec<u32>,
    generations: Vec<u32>,
    sparse: SparseSet,
}

impl Entities {
    #[inline]
    pub fn new() -> Entities {
        Entities::default()
    }

    pub fn spawn(&mut self) -> EntityHandle {
        // Check if there are any free IDs...
        if let Some(id) = self.freelist.pop() {
            // Insert it back into the sparse set and increase the generation
            self.sparse.insert(id);
            self.generations[id as usize] += 1;

            let generation = self.generations[id as usize];

            tracing::trace!("spawned entity via freelist (id: {id}, generation: {generation})");
            EntityHandle::new(id, generation)
        } else {
            // ... otherwise allocate a new one
            let id = self.next_id;
            self.next_id += 1;

            self.sparse.insert(id);
            self.generations.push(0);

            tracing::trace!("spawned entity with new ID {id}");
            EntityHandle::new(id, 0)
        }
    }

    pub fn despawn(&mut self, entity: EntityHandle) {
        let id = entity.index();
        let generation = entity.generation();

        if self.generations[id as usize] != generation {
            // This is an old entity, ignore it
            tracing::trace!("attempt to despawn orphaned entity");
            return;
        }

        self.sparse.remove(id);
        self.freelist.push(id);
        tracing::trace!("despawned entity {id}");
    }

    #[inline]
    pub fn alive_count(&self) -> usize {
        self.sparse.len()
    }

    pub fn is_alive(&self, entity: EntityHandle) -> bool {
        let id = entity.index();
        let generation = entity.generation();

        // Check whether it's in the sparse list and the generation is up to date.
        self.sparse.contains(id) && self.generations[id as usize] == generation
    }
}

#[derive(Debug, Copy, Default, Clone, PartialEq, Eq, Hash)]
pub struct EntityHandle(pub(crate) u64);

impl EntityHandle {
    pub(crate) fn new(id: u32, generation: u32) -> EntityHandle {
        let generation = (generation as u64) << 32;
        EntityHandle(id as u64 | generation)
    }

    #[inline]
    pub fn dangling() -> EntityHandle {
        EntityHandle(u64::MAX)
    }

    #[inline]
    pub fn is_dangling(&self) -> bool {
        self.0 == u64::MAX
    }

    #[inline]
    pub fn unique_id(&self) -> u64 {
        self.0
    }

    #[inline]
    pub fn index(&self) -> u32 {
        const ID_MASK: u64 = 0x00000000FFFFFFFF;
        (self.0 & ID_MASK) as u32
    }

    #[inline]
    pub fn generation(&self) -> u32 {
        const GEN_MASK: u64 = 0xFFFFFFFF00000000;
        ((self.0 & GEN_MASK) >> 32) as u32
    }
}

impl fmt::Display for EntityHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let id = self.index();
        let generation = self.generation();

        write!(f, "{id} ({generation})")
    }
}

#[cfg(test)]
mod tests {
    use tracing::Level;

    use crate::entity::Entities;

    #[test]
    fn spawn_despawn() {
        tracing_subscriber::fmt()
            .with_max_level(Level::TRACE)
            .compact()
            .init();

        let mut ents = Entities::new();

        let ent1 = ents.spawn();

        println!("{ents:?}");

        let ent2 = ents.spawn();

        println!("{ents:?}");

        let ent3 = ents.spawn();

        println!("{ents:?}");

        ents.despawn(ent2);

        println!("{ents:?}");

        let ent4 = ents.spawn();

        println!("{ents:?}");

        assert!(!ents.is_alive(ent2));

        println!("alive: {}", ents.alive_count());
    }
}
