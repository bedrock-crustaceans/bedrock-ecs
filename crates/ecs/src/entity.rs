use std::{fmt, marker::PhantomData, ops::Deref};

use bitvec::vec::BitVec;

#[cfg(debug_assertions)]
use crate::util::debug::RwGuard;
use crate::{ComponentBundle, component::Component, world::World};

#[derive(Debug, Copy, Default, Clone, PartialEq, Eq, Hash)]
pub struct EntityId(pub(crate) usize);

impl EntityId {
    pub fn dangling() -> EntityId {
        EntityId(usize::MAX)
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct GenerationId(pub(crate) usize);

/// Having an instance of this entity means you have exclusive access to the entire world.
pub struct EntityMut<'w> {
    pub(crate) world: &'w mut World,
    pub(crate) id: EntityId,
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
    pub(crate) id: EntityId,
}

impl<'w> Entity<'w> {
    /// Returns the ID of this entity.
    pub fn id(&self) -> EntityId {
        self.id
    }

    /// Checks whether this entity has all the given components.
    ///
    /// This has relatively large overhead per entity compared to queries.
    pub fn has<T: ComponentBundle>(&self) -> bool {
        self.world.has_components::<T>(self.id)
    }
}

#[cfg(debug_assertions)]
impl<'w> Drop for Entity<'w> {
    fn drop(&mut self) {
        self.world.flag.unlock_guardless();
    }
}

#[derive(Default)]
pub struct Entities {
    generation: GenerationId,
    indices: BitVec,
}

impl Entities {
    pub fn new() -> Entities {
        Entities::default()
    }

    pub fn reserve(&mut self, n: usize) {
        self.indices.reserve(n);
    }

    pub fn count(&self) -> usize {
        self.indices.count_ones()
    }

    pub fn alloc(&mut self) -> EntityId {
        // TODO: Should probably keep a list of empty spots instead of trying to find them.
        // This is incredibly slow.
        let gap = self
            .indices
            .iter()
            .by_vals()
            .enumerate()
            .find_map(|(i, v)| if v { None } else { Some(i) });

        let id = if let Some(gap) = gap {
            self.indices.set(gap, true);
            gap
        } else {
            self.indices.push(true);
            self.indices.len()
        };

        EntityId(id)
    }

    pub fn free(&mut self, entity: EntityId) {
        self.indices.set(entity.0, false);
    }
}
