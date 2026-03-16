use generic_array::GenericArray;
#[cfg(feature = "generics")]
use generic_array::typenum::U1;
#[cfg(not(feature = "generics"))]
use smallvec::{SmallVec, smallvec};

#[cfg(not(feature = "generics"))]
use crate::param;

use crate::archetype::Archetypes;
use crate::component::ComponentBundle;
use crate::entity::{Entities, Entity, EntityHandle, EntityMut};
use crate::graph::{AccessDesc, AccessType, Schedule};
use crate::param::Param;
use crate::resource::{Resource, ResourceBundle, Resources};
use crate::schedule::ScheduleBuilder;
use crate::spawn::SpawnBundle;
use crate::system::SystemMeta;

#[cfg(debug_assertions)]
use crate::util::debug::RwFlag;

#[derive(Default)]
pub struct World {
    pub(crate) archetypes: Archetypes,
    pub(crate) entities: Entities,
    pub(crate) resources: Resources,

    #[cfg(debug_assertions)]
    pub(crate) flag: RwFlag,
}

impl World {
    #[inline]
    pub fn new() -> World {
        World::default()
    }

    // Entities
    // ======================================================================================

    pub fn spawn<B: SpawnBundle>(&mut self, bundle: B) -> EntityMut<'_> {
        let id = self.entities.spawn();
        self.archetypes.insert(id, bundle);

        #[cfg(debug_assertions)]
        self.flag.write_guardless();

        EntityMut {
            handle: id,
            world: self,
        }
    }

    #[inline]
    pub(crate) fn despawn(&mut self, handle: EntityHandle) {
        self.entities.despawn(handle)
    }

    #[inline]
    pub(crate) fn has_components<T: ComponentBundle>(&self, entity: EntityHandle) -> bool {
        self.archetypes.has_components::<T>(entity)
    }

    pub fn get_entity(&self, handle: EntityHandle) -> Option<Entity<'_>> {
        if self.entities.is_alive(handle) {
            return Some(Entity {
                handle,
                world: self,
            });
        }

        None
    }

    pub fn get_entity_mut(&mut self, handle: EntityHandle) -> Option<EntityMut<'_>> {
        if self.entities.is_alive(handle) {
            return Some(EntityMut {
                handle,
                world: self,
            });
        }

        None
    }

    #[inline]
    pub fn alive_count(&self) -> usize {
        self.entities.alive_count()
    }

    // Resources
    // ======================================================================================

    pub fn add_resources<R: ResourceBundle>(&mut self, resources: R) {
        resources.insert_into(&mut self.resources);
    }

    #[inline]
    pub fn get_resource<R: Resource>(&self) -> Option<&R> {
        self.resources.get::<R>()
    }

    #[inline]
    pub fn get_resource_mut<R: Resource>(&mut self) -> Option<&mut R> {
        self.resources.get_mut::<R>()
    }

    #[inline]
    pub fn contains_resource<R: ResourceBundle>(&self) -> bool {
        self.resources.contains::<R>()
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

unsafe impl Param for &World {
    #[cfg(feature = "generics")]
    type AccessCount = U1;

    type State = ();

    type Output<'w> = &'w World;

    #[cfg(feature = "generics")]
    fn access(_world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount> {
        GenericArray::from((AccessDesc {
            ty: AccessType::World,
            exclusive: false,
        },))
    }

    #[cfg(not(feature = "generics"))]
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        smallvec![AccessDesc {
            ty: AccessType::World,
            exclusive: false
        }]
    }

    fn fetch<'w, S: crate::sealed::Sealed>(
        world: &'w World,
        _state: &'w mut Self::State,
    ) -> Self::Output<'w> {
        world
    }

    fn init(_world: &mut World, _meta: &SystemMeta) {
        unimplemented!("A world cannot initialise another world");
    }
}

unsafe impl Send for World {}
unsafe impl Sync for World {}
