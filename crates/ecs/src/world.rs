use std::sync::atomic::{AtomicU64, Ordering};

use generic_array::GenericArray;
#[cfg(feature = "generics")]
use generic_array::typenum::U1;
#[cfg(not(feature = "generics"))]
use smallvec::{SmallVec, smallvec};

#[cfg(not(feature = "generics"))]
use crate::param;

use crate::archetype::Archetypes;
use crate::command::CommandPool;
use crate::component::ComponentBundle;
use crate::entity::{Entities, Entity, EntityHandle, EntityMut, EntityRef};
use crate::resource::{Resource, ResourceBundle, Resources};
use crate::scheduler::{AccessDesc, AccessType, Schedule, ScheduleBuilder};
use crate::sealed::Sealed;
use crate::system::{Param, SystemMeta};

pub struct World {
    pub(crate) archetypes: Archetypes,
    pub(crate) entities: Entities,
    pub(crate) resources: Resources,
    pub(crate) commands: Option<CommandPool>,

    pub(crate) current_tick: u32,
}

impl World {
    #[inline]
    #[must_use]
    pub fn new() -> World {
        World {
            archetypes: Archetypes::new(),
            entities: Entities::new(),
            resources: Resources::new(),
            commands: Some(CommandPool::new()),

            current_tick: 0,
        }
    }

    pub fn apply_commands(&mut self) {
        // Take out of the world temporarily to allow commands to take a `&mut World`.
        #[expect(clippy::missing_panics_doc, reason = "can never be triggered by user")]
        let mut commands = self.commands.take().expect("World::commands was empty");
        unsafe { commands.apply_all(self) };

        self.commands = Some(commands);
    }

    // Entities
    // ======================================================================================
    pub fn spawn(&mut self, bundle: impl ComponentBundle) -> EntityMut<'_> {
        let id = self.entities.allocate();
        let meta = self.archetypes.insert(id, bundle, self.current_tick);
        self.entities.spawn(meta);

        EntityMut {
            handle: id,
            world: self,
        }
    }

    #[inline]
    pub(crate) fn despawn(&mut self, entity: Entity) {
        // Remove from table
        // self.archetypes.remove(&entity);
        todo!();

        // Remove from alive list.
        self.entities.despawn(entity.handle);
    }

    #[inline]
    pub(crate) fn has_components<T: ComponentBundle>(&self, entity: EntityHandle) -> bool {
        self.archetypes.has_components::<T>(entity)
    }

    pub fn get_entity(&self, handle: EntityHandle) -> Option<EntityRef<'_>> {
        if self.entities.is_alive(handle) {
            return Some(EntityRef {
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

    /// Returns the amount of entities currently alive in this world.
    #[inline]
    pub fn alive_count(&self) -> usize {
        self.entities.alive_count()
    }

    // Resources
    // ======================================================================================

    #[inline]
    pub fn add_resources(&mut self, resources: impl ResourceBundle) {
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

    #[expect(clippy::missing_panics_doc, reason = "internal invariant")]
    pub fn run(&mut self, schedule: &Schedule) {
        for set in &schedule.sets {
            // for id in set {
            //     schedule.systems.get(id).unwrap().call(&self);
            // }

            // rayon::scope(|s| {
            //     for id in set {
            //         s.spawn(|_| {
            //             unsafe { schedule.systems.get(id).unwrap().call(self) };
            //         });
            //     }
            // });

            #[cfg(miri)] // Miri is not very happy about rayon.
            std::thread::scope(|s| {
                // for system in schedule.systems.values() {
                //     s.spawn(|| {
                //         unsafe { system.call(self) };
                //     });
                // }

                for id in set {
                    s.spawn(|| {
                        unsafe { schedule.systems.get(id).unwrap().call(self) };
                    });
                }
            })

            // tracing::info!("Running next set");
            // rayon::scope(|s| {
            //     for id in set {
            //         s.spawn(|_| {
            //             schedule.systems.get(id).unwrap().call(&self);
            //         });
            //     }
            // });
        }
        self.current_tick += 1;
    }

    #[inline]
    pub fn build_schedule(&mut self) -> ScheduleBuilder<'_> {
        ScheduleBuilder::new(self)
    }
}

impl Default for World {
    #[inline]
    fn default() -> World {
        Self::new()
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

unsafe impl Param for &mut World {
    #[cfg(feature = "generics")]
    type AccessCount = U1;

    type State = ();

    type Output<'w> = &'w mut World;

    #[cfg(feature = "generics")]
    fn access(_world: &mut World) -> GenericArray<AccessDesc, U1> {
        GenericArray::from((AccessDesc {
            ty: AccessType::World,
            exclusive: true,
        },))
    }

    #[cfg(not(feature = "generics"))]
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        smallvec![AccessDesc {
            ty: AccessType::World,
            exclusive: true
        }]
    }

    fn init(_world: &mut World, _meta: &SystemMeta) {}

    fn fetch<'w, S: Sealed>(_world: &'w World, _state: &'w mut ()) -> &'w mut World {
        todo!()
    }
}

unsafe impl Send for World {}
unsafe impl Sync for World {}
