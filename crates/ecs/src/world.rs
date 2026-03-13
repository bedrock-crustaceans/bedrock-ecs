use crate::{archetype::Archetypes, component::ComponentRegistry, entity::{Entities, EntityMut}, spawn::SpawnBundle};
use crate::graph::Schedule;
use crate::schedule::ScheduleBuilder;

#[derive(Default)]
pub struct World {
    pub(crate) archetypes: Archetypes,
    pub(crate) entities: Entities,
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
            for id in set {
                schedule.systems.get(id).unwrap().call(&self);
            }

            tracing::info!("Running next set");
            // rayon::scope(|s| {
            //     for id in set {
            //         s.spawn(|_| {
            //             schedule.systems.get(id).unwrap().call(&self);
            //         });
            //     }
            // });
        }
    }

    pub fn build_schedule(&mut self) -> ScheduleBuilder<'_> {
        ScheduleBuilder::new(self)
    }
}

unsafe impl Send for World {}
unsafe impl Sync for World {}