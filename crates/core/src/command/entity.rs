use std::marker::PhantomData;

use crate::command::{Command, Commands};
use crate::entity::{Entity, EntityIndex};
use crate::prelude::ComponentBundle;
use crate::world::World;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeferredEntity(pub(crate) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityCommandsHandle {
    /// The commands will be applied to an existing entity
    Spawned(Entity),
    /// The commands will be applied to an entity that still needs to be spawned.
    /// This happens when a system spawns an entity and then also modifies it within the same
    /// tick.
    Deferred(DeferredEntity),
}

impl EntityCommandsHandle {
    pub fn deferred(&self) -> bool {
        match self {
            Self::Deferred(_) => true,
            Self::Spawned(_) => false,
        }
    }
}

pub struct EntityCommands<'parent, 'state> {
    pub(crate) entity: EntityCommandsHandle,
    pub(crate) commands: &'parent mut Commands<'state>,
}

impl EntityCommands<'_, '_> {
    /// Returns the entity if it exists.
    ///
    /// Entities that have been spawned during this tick will not have a handle yet.
    #[inline]
    pub fn entity(&self) -> Option<Entity> {
        match &self.entity {
            EntityCommandsHandle::Spawned(entity) => Some(*entity),
            EntityCommandsHandle::Deferred(_) => None,
        }
    }

    /// Whether this entity is deferred.
    ///
    /// A deferred entity is one that does not exist yet, but will be created at some later point.
    #[inline]
    pub fn deferred(&self) -> bool {
        self.entity.deferred()
    }

    /// Adds components to this entity.
    ///
    /// This is a deferred operation and will be performed after the end of this tick.
    pub fn insert(&mut self, components: impl ComponentBundle) -> &mut Self {
        // self.commands.buffer.push(InsertCommand {
        //     entity: self.entity,
        //     components,
        // });
        self
    }

    /// Removes the given components from this entity.
    pub fn remove<B: ComponentBundle>(&mut self) -> &mut Self {
        let cmd: RemoveCommand<B> = RemoveCommand {
            entity: self.entity,
            _marker: PhantomData,
        };

        // self.commands.buffer.push(cmd);
        self
    }

    /// Despawns the entity
    pub fn despawn(self) {
        // self.commands.buffer.push(DespawnCommand {
        //     handle: self.entity,
        // });
    }
}

pub struct RemoveCommand<T: ComponentBundle> {
    entity: EntityCommandsHandle,
    _marker: PhantomData<T>,
}

impl<T: ComponentBundle> Command for RemoveCommand<T> {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "RemoveCommand::apply", skip_all)
    )]
    fn apply(self: Box<Self>, world: &mut World) {
        tracing::trace!(
            "removing {} from entity {:?}",
            std::any::type_name::<T>(),
            self.entity
        );

        match self.entity {
            EntityCommandsHandle::Spawned(handle) => {
                world
                    .get_entity_mut(handle)
                    .expect("entity did not exist")
                    .remove::<T>();
            }
            EntityCommandsHandle::Deferred(handle) => {
                let entity = *world
                    .deferred_entities
                    .get(&handle)
                    .expect("entity did not exist");

                world
                    .get_entity_mut(entity)
                    .expect("entity did not exist")
                    .remove::<T>();
            }
        }
    }
}

pub struct InsertCommand<T: ComponentBundle> {
    entity: EntityCommandsHandle,
    components: T,
}

impl<T: ComponentBundle> Command for InsertCommand<T> {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "InsertCommand::apply", skip_all)
    )]
    fn apply(self: Box<Self>, world: &mut World) {
        tracing::trace!(
            "inserting {} into {:?}",
            std::any::type_name::<T>(),
            self.entity
        );

        match self.entity {
            EntityCommandsHandle::Spawned(handle) => {
                world
                    .get_entity_mut(handle)
                    .expect("spawned entity not found")
                    .insert(self.components);
            }
            EntityCommandsHandle::Deferred(handle) => {
                // Get real entity ID.
                let entity = *world
                    .deferred_entities
                    .get(&handle)
                    .expect("entity did not exist");

                println!("inserting into deferred {handle:?} (real {entity:?})");

                world
                    .get_entity_mut(entity)
                    .expect("entity did not exist")
                    .insert(self.components);
            }
        }
    }
}

pub struct SpawnCommand<T: ComponentBundle> {
    pub(crate) handle: DeferredEntity,
    pub(crate) components: T,
}

impl<T: ComponentBundle> Command for SpawnCommand<T> {
    #[inline]
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "SpawnCommand::apply", skip_all)
    )]
    fn apply(self: Box<Self>, world: &mut World) {
        tracing::trace!("spawning {:?}", self.handle);
        let entity = world.spawn(self.components).handle();
        world.deferred_entities.insert(self.handle, entity);
    }
}

pub struct DespawnCommand {
    /// This entity might not actually exist yet.
    handle: EntityCommandsHandle,
}

impl Command for DespawnCommand {
    #[inline]
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "DespawnCommand::apply", skip_all)
    )]
    fn apply(self: Box<Self>, world: &mut World) {
        tracing::trace!("despawning {:?}", self.handle);

        match self.handle {
            EntityCommandsHandle::Spawned(handle) => world.despawn(handle),
            _ => todo!(),
        }
    }
}
