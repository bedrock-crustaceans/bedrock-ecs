use crate::{archetype::Archetypes, entity::{Entities, EntityMut}, spawn::SpawnBundle, system::Systems};
use crate::graph::Schedule;
use crate::schedule::ScheduleBuilder;

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

    pub fn spawn<B: SpawnBundle>(&mut self, bundle: B) -> EntityMut<'_> {
        let id = self.entities.alloc();
        self.archetypes.insert(id, bundle);

        EntityMut {
            id,
            world: self
        }
    }

    pub fn entities(&self) -> &Entities {
        &self.entities
    }
    
    pub fn run(&mut self, schedule: &Schedule) {
        for set in &schedule.sets {
            println!("Running next set");
            rayon::scope(|s| {
                for id in set {
                    s.spawn(|_| {
                        schedule.systems.get(id).unwrap().call(&self);
                    });
                }
            });
        }
    }
}

unsafe impl Send for World {}
unsafe impl Sync for World {}