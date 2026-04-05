use generic_array::GenericArray;
#[cfg(feature = "generics")]
use generic_array::typenum::U1;
use rustc_hash::FxHashMap;
#[cfg(not(feature = "generics"))]
use smallvec::{SmallVec, smallvec};

#[cfg(not(feature = "generics"))]
use crate::param;

use crate::archetype::Archetypes;
use crate::command::{CommandPool, DeferredEntity};
use crate::component::ComponentBundle;
use crate::entity::{Entities, Entity, EntityMut, EntityRef};
use crate::resource::{Resource, ResourceBundle, Resources};
use crate::scheduler::{AccessDesc, AccessType, ScheduleBuilder};
use crate::system::{IntoSystem, Param, System, SystemContainer, SystemMeta};

pub struct World {
    pub(crate) archetypes: Archetypes,
    pub(crate) entities: Entities,
    pub(crate) resources: Resources,
    pub(crate) commands: Option<CommandPool>,
    pub(crate) deferred_entities: FxHashMap<DeferredEntity, Entity>,

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
            deferred_entities: FxHashMap::default(),

            current_tick: 0,
        }
    }

    pub fn apply_commands(&mut self) {
        // Take out of the world temporarily to allow commands to take a `&mut World`.
        #[expect(clippy::missing_panics_doc, reason = "can never be triggered by user")]
        let mut commands = self.commands.take().expect("World::commands was empty");
        unsafe { commands.apply_all(self) };

        self.commands = Some(commands);
        self.deferred_entities.clear();
    }

    // Entities
    // ======================================================================================
    pub fn spawn(&mut self, bundle: impl ComponentBundle) -> EntityMut<'_> {
        let handle = self.entities.allocate();
        let meta = self.archetypes.spawn(handle, bundle, self.current_tick);
        self.entities.spawn(meta);

        EntityMut {
            handle,
            world: self,
        }
    }

    pub fn spawn_batch(
        &mut self,
        batch: impl Iterator<Item = impl ComponentBundle>,
    ) -> Vec<EntityMut<'_>> {
        let (min, max) = batch.size_hint();
        let size = max.unwrap_or(min);

        todo!();
    }

    #[inline]
    pub fn despawn(&mut self, entity: Entity) {
        let Some(meta) = self.entities.get_meta(entity) else {
            tracing::error!("attempt to despawn entity that was already dead");
            return;
        };

        // Remove from table
        unsafe { self.archetypes.despawn(&mut self.entities, meta) };

        // Remove from alive list.
        self.entities.despawn_meta(entity);
    }

    #[inline]
    pub(crate) fn has_components<T: ComponentBundle>(&self, entity: Entity) -> bool {
        self.archetypes.has_components::<T>(entity)
    }

    pub fn get_entity(&self, handle: Entity) -> Option<EntityRef<'_>> {
        if self.entities.is_alive(handle) {
            return Some(EntityRef {
                handle,
                world: self,
            });
        }

        None
    }

    pub fn get_entity_mut(&mut self, handle: Entity) -> Option<EntityMut<'_>> {
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

    #[inline]
    pub fn build_schedule(&mut self) -> ScheduleBuilder<'_> {
        ScheduleBuilder::new(self)
    }

    pub fn run_system<P, S: IntoSystem<P>>(&mut self, system: S) {
        let system = system.into_system(self);
        unsafe {
            system.call(self);
        }
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
            mutable: false,
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

    fn init(_world: &mut World, _meta: &SystemMeta) {}
}

unsafe impl Send for World {}
unsafe impl Sync for World {}
