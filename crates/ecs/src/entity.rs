use std::{fmt, marker::PhantomData};

use bitvec::vec::BitVec;

use crate::{component::Component, world::World};

#[derive(Debug, Copy, Default, Clone, PartialEq, Eq, Hash)]
pub struct EntityId(pub(crate) usize);

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct GenerationId(pub(crate) usize);

#[derive(Clone)]
pub struct EntityMeta {
    id: EntityId,
    generation: GenerationId
}

#[derive(Clone)]
pub struct Entity<'w> {
    pub(crate) world: &'w World,
    pub(crate) id: EntityId
}

impl<'w> Entity<'w> {
    pub fn id(&self) -> EntityId {
        self.id
    }

    pub fn has<T: Component>(&self) -> bool {
        todo!()
        // self.world.components.has_component::<T>(self.id)
    }
}

pub struct EntityMut<'w> {
    pub(crate) world: &'w mut World,
    pub(crate) id: EntityId
}

impl<'w> EntityMut<'w> {
    pub fn id(&self) -> EntityId {
        self.id
    }

    pub fn has<T: Component>(&self) -> bool {
        todo!()
        // self.world.components.has_component::<T>(self.id)
    }
}

#[derive(Default)]
pub struct Entities {
    generation: GenerationId,
    indices: BitVec
}

impl Entities {
    pub fn new() -> Entities {
        Entities::default()
    }

    pub fn count(&self) -> usize {
        self.indices.count_ones()
    }

    pub fn alloc(&mut self) -> EntityId {
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