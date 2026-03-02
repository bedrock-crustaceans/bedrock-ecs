use crate::{archetype::Archetypes, entity::{Entities, EntityMut}, spawn::ComponentBundle, system::Systems};

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

    pub fn spawn<'w, B: ComponentBundle>(&'w mut self, bundle: B) -> EntityMut<'w> {
        let id = self.entities.alloc();
        self.archetypes.insert(id, bundle);

        EntityMut {
            id, world: self
        }
    }
}