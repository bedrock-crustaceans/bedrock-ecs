use crate::component::{Components, SpawnBundle};
use crate::entity::{Entities, Entity};
use crate::scheduler::{MultiThreadedExecutor, Schedule, Scheduler, SingleThreadedExecutor};
use crate::{EcsResult, Events, Resource, Resources, Systems};
use std::sync::Arc;

#[derive(Default)]
pub struct World {
    pub(crate) entities: Entities,
    pub(crate) components: Components,
    pub(crate) systems: Systems,
    pub(crate) scheduler: Scheduler,
    pub(crate) events: Events,
    pub(crate) resources: Resources,
}

impl World {
    pub fn new() -> Arc<World> {
        Arc::new(World::default())
    }

    pub fn spawn<B: SpawnBundle>(self: &Arc<Self>, bundle: B) -> EcsResult<Entity> {
        let entity = self.entities.alloc();
        bundle.insert_into(&self.components, entity)?;

        Ok(Entity {
            world: Arc::clone(self),
            id: entity,
        })
    }

    pub fn schedule_single_threaded(self: &Arc<Self>) -> Schedule<SingleThreadedExecutor> {
        Schedule::new(self)
    }

    pub fn schedule_multi_threaded(self: &Arc<Self>) -> Schedule<MultiThreadedExecutor> {
        Schedule::new(self)
    }

    #[inline]
    pub fn spawn_empty(self: &Arc<Self>) -> EcsResult<Entity> {
        self.spawn(())
    }

    pub fn add_resource<R: Resource>(self: &Arc<Self>, resource: R) {
        self.resources.insert(resource)
    }

    // pub fn add_system<P, R, S>(self: &Arc<Self>, system: S)
    // where
    //     P: SystemParams + Send + Sync + 'static,
    //     R: SystemReturnable + Send + Sync + 'static,
    //     S: ParameterizedSystem<P, R> + Send + Sync + 'static,
    //     FnContainer<P, R, S>: System,
    // {
    //     let contained = Arc::new(system.into_container(self));
    //     self.systems.push(self, contained);
    // }

    // pub fn add_async_system<P, S>(self: &Arc<Self>, system: S)
    // where
    //     P: SystemParams + Send + Sync + 'static,
    //     S: AsyncSystem<P>
    // {
    //     let contained = Arc::new(system.pinned(self));
    //     self.systems.push(self, contained);
    // }

    // pub fn async_system<P, R, S, F>(self: &Arc<Self>, system: S)
    // where
    //     P: SystemParams + Send + Sync + 'static,
    //     R: SystemReturnable + Send + Sync + 'static,
    //     S: ParameterizedSystem<P, R> + Send + Sync + 'static,
    //     AsyncContainer<P, R, S>: System,
    // {
    //     let wrapped = AsyncContainer::new(self);
    //     self.systems.push(self, wrapped);
    // }

    pub async fn tick(self: &Arc<Self>) {
        self.scheduler.pre_tick(self);
        self.systems.call(self).await;
        self.scheduler.post_tick(self);
    }
}
