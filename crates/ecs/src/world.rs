use crate::{archetype::Archetypes, entity::{Entities, EntityMut}, spawn::SpawnGroup, system::Systems};
use crate::schedule::ScheduleBuilder;

#[derive(Default)]
pub struct World {
    pub(crate) archetypes: Archetypes,
    pub(crate) entities: Entities,
    pub systems: Systems
}

impl World {
    pub fn new() -> World {
        World::default()
    }

    pub fn spawn<B: SpawnGroup>(&mut self, bundle: B) -> EntityMut<'_> {
        let id = self.entities.alloc();
        self.archetypes.insert(id, bundle);

        EntityMut {
            id,
            world: self
        }
    }
    
    pub fn run(&self, schedule: &ScheduleBuilder) {
        todo!()
    }
}

unsafe impl Send for World {}
unsafe impl Sync for World {}