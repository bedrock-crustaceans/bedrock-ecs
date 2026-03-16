use std::{fmt, ptr::NonNull};

use crate::{component::ComponentBundle, table::{Table, TableRow}, world::World};

/// Having an instance of this entity means you have exclusive access to the entire world.
/// 
/// This allows calling mutable methods directly rather than having to push them to command buffers.
pub struct EntityMut<'w> {
    pub(crate) world: &'w mut World,
    pub(crate) handle: EntityHandle,
}

impl EntityMut<'_> {
    #[inline]
    pub fn handle(&self) -> EntityHandle {
        self.handle
    }

    pub fn index(&self) -> EntityIndex {
        self.handle.index()
    }

    pub fn generation(&self) -> EntityGeneration {
        self.handle.generation()
    }

    #[inline]
    pub fn despawn(self) {
        self.world.despawn(self.handle);
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

const TOMBSTONE: u32 = u32::MAX;

#[derive(Debug, Clone)]
pub struct EntityMeta {
    pub handle: EntityHandle,
    pub table: Option<NonNull<Table>>,
    pub row: TableRow
}   

#[derive(Default, Debug, Clone)]
pub struct Entities {
    next_id: u32,
    freelist: Vec<u32>,
    generations: Vec<u32>,
    dense: Vec<EntityMeta>,
    sparse: Vec<u32>
}

impl Entities {
    #[inline]
    pub fn new() -> Entities {
        Entities::default()
    }

    /// Allocates an entity handle but does not insert it into the registry yet.
    pub fn allocate(&mut self) -> EntityHandle {
        // Allocate an ID for this new entity.
        let handle = if let Some(id) = self.freelist.pop() {
            self.generations[id as usize] += 1;

            let generation = self.generations[id as usize];

            tracing::trace!("spawned entity via freelist (id: {id}, generation: {generation})");
            EntityHandle::new(id, generation)
        } else {
            let id = self.next_id;
            self.next_id += 1;

            self.generations.push(0);

            EntityHandle::new(id, 0)
        };

        handle
    }

    /// Inserts the given entity metadata into the registry.
    pub fn spawn(&mut self, meta: EntityMeta) {
        tracing::error!("spawning {:?}", meta.handle);

        let index = meta.handle.index().0 as usize;

        self.dense.push(meta);

        if index >= self.sparse.len() {
            self.sparse.resize(index + 1, TOMBSTONE);
        }
        self.sparse[index] = self.dense.len() as u32 - 1;
    }

    pub fn despawn(&mut self, entity: EntityHandle) {
        let id = entity.index().0;
        let generation = entity.generation().0;

        if self.generations[id as usize] != generation {
            // This is an old entity, ignore it
            tracing::trace!("attempt to despawn old entity");
            return;
        }

        self.dense.swap_remove(id as usize);
        let swapped_index = self.dense[id as usize].handle.index().0;
        self.sparse[swapped_index as usize] = id;

        self.sparse[id as usize] = TOMBSTONE;
        self.freelist.push(id);

        tracing::trace!("despawned entity {id}");
    }

    #[inline]
    pub fn alive_count(&self) -> usize {
        self.sparse.len()
    }

    pub fn is_alive(&self, entity: EntityHandle) -> bool {
        let id = entity.index().0;
        let generation = entity.generation().0;

        // Check whether index is in the sparse list and verify generation
        self.sparse[id as usize] != TOMBSTONE && self.generations[id as usize] == generation
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
    pub fn index(&self) -> EntityIndex {
        const ID_MASK: u64 = 0x00000000FFFFFFFF;
        let index = (self.0 & ID_MASK) as u32;
        EntityIndex(index)
    }

    #[inline]
    pub fn generation(&self) -> EntityGeneration {
        const GEN_MASK: u64 = 0xFFFFFFFF00000000;
        let generation = ((self.0 & GEN_MASK) >> 32) as u32;
        EntityGeneration(generation)
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct EntityIndex(pub(crate) u32);

impl fmt::Display for EntityIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let index = self.0;
        write!(f, "{index}")
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct EntityGeneration(pub(crate) u32);

impl fmt::Display for EntityGeneration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let generation = self.0;
        write!(f, "{generation}")
    }
}
