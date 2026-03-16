use std::{fmt, ptr::NonNull};
use std::cmp::Ordering;
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

/// An entity that has immutable to the world.
#[derive(Clone)]
pub struct EntityRef<'w> {
    pub(crate) world: &'w World,
    pub(crate) handle: EntityHandle,
}

impl<'w> EntityRef<'w> {
    /// Returns the handle of this entity.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    pub handle: EntityHandle,
    pub table: Option<NonNull<Table>>,
    pub row: TableRow
}

unsafe impl Send for Entity {}

#[derive(Default, Debug, Clone)]
pub struct Entities {
    next_id: u32,
    freelist: Vec<u32>,
    generations: Vec<u32>,
    dense: Vec<Entity>,
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
            EntityHandle::from_index_and_generation(
                EntityIndex::from_bits(id), EntityGeneration::from_bits(generation)
            )
        } else {
            let id = self.next_id;
            self.next_id += 1;

            self.generations.push(0);

            EntityHandle::from_index_and_generation(
                EntityIndex::from_bits(id), EntityGeneration::FIRST
            )
        };

        handle
    }

    /// Inserts the given entity metadata into the registry.
    pub fn spawn(&mut self, meta: Entity) {
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

// This pretty similar to Bevy's version since it seems to work better than a plain `u64` with
// bitwise operations like we initially did. The fields are aligned in such a way that the struct
// is equivalent to a `u64`.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct EntityHandle {
    index: EntityIndex,
    generation: EntityGeneration
}

impl PartialEq for EntityHandle {
    #[inline]
    fn eq(&self, other: &EntityHandle) -> bool {
        self.to_bits() == other.to_bits()
    }
}

impl Eq for EntityHandle {}

impl PartialOrd for EntityHandle {
    #[inline]
    fn partial_cmp(&self, other: &EntityHandle) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EntityHandle {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_bits().cmp(&other.to_bits())
    }
}

impl EntityHandle {
    const DANGLING: EntityHandle = EntityHandle::from_index_and_generation(
        EntityIndex(u32::MAX), EntityGeneration(u32::MAX)
    );

    #[inline]
    pub const fn from_index_and_generation(index: EntityIndex, generation: EntityGeneration) -> EntityHandle {
        EntityHandle {
            index, generation
        }
    }

    #[inline]
    pub const fn dangling() -> EntityHandle {
        Self::DANGLING
    }

    #[inline]
    pub const fn is_dangling(&self) -> bool {
        self.to_bits() == Self::DANGLING.to_bits()
    }

    #[inline]
    pub const fn to_bits(&self) -> u64 {
        self.generation.to_bits() as u64 | ((self.index.to_bits() as u64) << 32)
    }

    #[inline]
    pub const fn index(&self) -> EntityIndex {
        self.index
    }

    #[inline]
    pub fn generation(&self) -> EntityGeneration {
        self.generation
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct EntityIndex(pub(crate) u32);

impl EntityIndex {
    pub const TOMBSTONE: EntityIndex = EntityIndex(u32::MAX);

    #[inline]
    pub const fn to_bits(&self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn from_bits(index: u32) -> EntityIndex {
        EntityIndex(index)
    }
}

impl fmt::Display for EntityIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let index = self.0;
        write!(f, "{index}")
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct EntityGeneration(pub(crate) u32);

impl EntityGeneration {
    pub const FIRST: EntityGeneration = EntityGeneration(0);

    #[inline]
    pub const fn to_bits(&self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn from_bits(bits: u32) -> EntityGeneration {
        EntityGeneration(bits)
    }
}

impl fmt::Display for EntityGeneration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let generation = self.0;
        write!(f, "{generation}")
    }
}
