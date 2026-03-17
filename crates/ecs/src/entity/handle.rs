use std::cmp::Ordering;
use std::fmt;

// This pretty similar to Bevy's version since it seems to work better than a plain `u64` with
// bitwise operations like we initially did. The fields are aligned in such a way that the struct
// is equivalent to a `u64`.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct EntityHandle {
    index: EntityIndex,
    generation: EntityGeneration,
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
    const DANGLING: EntityHandle =
        EntityHandle::from_index_and_generation(EntityIndex(u32::MAX), EntityGeneration(u32::MAX));

    #[inline]
    pub const fn from_index_and_generation(
        index: EntityIndex,
        generation: EntityGeneration,
    ) -> EntityHandle {
        EntityHandle { index, generation }
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
