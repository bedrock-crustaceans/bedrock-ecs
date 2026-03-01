use crate::{archetype::Archetypes, component::Components, entity::{Entities, Entity, EntityMut}, spawn::ComponentGroup, system::Systems};

#[derive(Default)]
pub struct World {
    pub(crate) archetypes: Archetypes,
    pub(crate) entities: Entities,
    pub(crate) systems: Systems
}

impl World {
    pub fn new() -> World {
        World::default()
    }

    pub fn spawn<'w, B: ComponentGroup>(&'w mut self, bundle: B) -> EntityMut<'w> {
        let id = self.entities.alloc();
        bundle.insert_into(id, &mut self.archetypes);

        EntityMut {
            id, world: self
        }
    }
}