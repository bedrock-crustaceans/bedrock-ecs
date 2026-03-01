use std::{iter::FusedIterator, marker::PhantomData, ops::Deref};

use bitvec::vec::BitVec;

use crate::{component::Component, filter::FilterGroup, query::QueryGroup, world::World};

#[derive(Debug, Copy, Default, Clone, PartialEq, Eq, Hash)]
pub struct EntityId(pub(crate) usize);

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

pub(crate) struct EntityIter<'a, Q: QueryGroup, F: FilterGroup> {
    world: &'a World,
    entities: &'a BitVec,
    bvec_index: usize,
    index: usize,
    _marker: PhantomData<&'a (Q, F)>
}

impl<'a, Q: QueryGroup, F: FilterGroup> Iterator for EntityIter<'a, Q, F> {
    type Item = Entity<'a>;

    fn next(&mut self) -> Option<Entity<'a>> {
        loop {
            // TODO: use bvec_index
            let next_id = self.entities.iter_ones().nth(self.index)?;
            
            self.index += 1;
            let entity = Entity {
                world: self.world,
                id: EntityId(next_id)
            };

            if Q::filter(&entity) && F::filter(&entity) {
                break Some(entity);
            }
        }
    }
}

impl<Q: QueryGroup, F: FilterGroup> FusedIterator for EntityIter<'_, Q, F> {}

#[derive(Default)]
pub(crate) struct Entities {
    generation: GenerationId,
    indices: BitVec
}

impl Entities {
    pub fn new() -> Entities {
        Entities::default()
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

    pub fn iter<'a, Q: QueryGroup, F: FilterGroup>(&'a self, world: &'a World) -> EntityIter<'a, Q, F> {
        EntityIter {
            world,
            entities: &self.indices,
            bvec_index: 0,
            index: 0,
            _marker: PhantomData
        }
    }
}