use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

use nonmax::NonMaxU32;

// This pretty similar to Bevy's version since it seems to work better than a plain `u64` with
// bitwise operations like we initially did. The fields are aligned in such a way that the struct
// is equivalent to a `u64`.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct Entity {
    index: EntityIndex,
    generation: EntityGeneration,
}

impl PartialEq for Entity {
    #[inline]
    fn eq(&self, other: &Entity) -> bool {
        self.to_bits() == other.to_bits()
    }
}

impl Eq for Entity {}

impl PartialOrd for Entity {
    #[inline]
    fn partial_cmp(&self, other: &Entity) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entity {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_bits().cmp(&other.to_bits())
    }
}

impl Hash for Entity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_bits().hash(state);
    }
}

impl Entity {
    const DANGLING: Entity =
        Entity::from_index_and_generation(EntityIndex(None), EntityGeneration(u32::MAX));

    #[inline]
    pub const fn from_index_and_generation(
        index: EntityIndex,
        generation: EntityGeneration,
    ) -> Entity {
        Entity { index, generation }
    }

    #[inline]
    pub const fn dangling() -> Entity {
        Self::DANGLING
    }

    #[inline]
    pub const fn is_dangling(&self) -> bool {
        self.to_bits() == Self::DANGLING.to_bits()
    }

    #[inline]
    pub const fn from_bits(bits: u64) -> Self {
        let gen_bits = bits as u32; // Only take the lower 32 bits
        let generation = EntityGeneration::from_bits(gen_bits);

        let idx_bits = (bits >> 32) as u32;
        let index = EntityIndex::from_bits(idx_bits);

        Self::from_index_and_generation(index, generation)
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
pub struct EntityIndex(pub(crate) Option<NonMaxU32>);

impl EntityIndex {
    pub const TOMBSTONE: EntityIndex = EntityIndex(None);

    /// `u32::MAX` is returned if the index refers to a 'tombstone', or dead entity.
    #[inline]
    pub const fn to_bits(&self) -> u32 {
        // This cannot be done using `Option::map` because it is not const
        // when attempting to drop a `NonMaxU32` inside of it.
        match self.0 {
            Some(handle) => handle.get(),
            None => u32::MAX,
        }
    }

    /// `u32::MAX` will mark this entity as a `tombstone`.
    #[inline]
    pub const fn from_bits(index: u32) -> EntityIndex {
        EntityIndex(NonMaxU32::new(index))
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
