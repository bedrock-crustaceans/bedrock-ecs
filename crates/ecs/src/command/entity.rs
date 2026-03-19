use crate::command::{Command, Commands};
use crate::component::SpawnBundle;
use crate::entity::{Entity, EntityHandle, EntityIndex};
use crate::world::World;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityCommandsHandle {
    /// The commands will be applied to an existing entity
    Spawned(Entity),
    /// The commands will be applied to an entity that still needs to be spawned.
    /// This happens when a system spawns an entity and then also modifies it within the same
    /// tick.
    Deferred(EntityIndex),
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

impl<'s, 'c> EntityCommands<'s, 'c> {
    /// Returns the entity's handle if it exists.
    ///
    /// Entities that have been spawned during this tick will not have a handle yet.
    #[inline]
    pub fn handle(&self) -> Option<EntityHandle> {
        self.entity().map(|entity| entity.handle)
    }

    /// Returns the entity if it exists.
    ///
    /// Entities that have been spawned during this tick will not have a handle yet.
    #[inline]
    pub fn entity(&self) -> Option<&Entity> {
        match &self.entity {
            EntityCommandsHandle::Spawned(entity) => Some(entity),
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
    pub fn insert(&mut self, components: impl SpawnBundle) -> &mut Self {
        todo!()
    }

    /// Removes the given components from this entity.
    pub fn remove<S: SpawnBundle>(&mut self) {
        todo!()
    }

    /// Despawns the entity
    pub fn despawn(self) {
        self.commands.buffer.push(DespawnCommand {
            handle: self.entity,
        });
    }
}

pub struct InsertCommand<T: SpawnBundle> {
    entity: EntityCommandsHandle,
    components: T,
}

impl<T: SpawnBundle> Command for InsertCommand<T> {
    fn apply(self, world: &mut World) {
        todo!()
    }
}

impl<T: SpawnBundle> Drop for InsertCommand<T> {
    fn drop(&mut self) {
        println!("drop insert test: {:?}", self.entity);
    }
}

pub struct SpawnCommand<T: SpawnBundle> {
    pub(crate) handle: EntityCommandsHandle,
    pub(crate) components: T,
}

impl<T: SpawnBundle> Command for SpawnCommand<T> {
    #[inline]
    fn apply(self, world: &mut World) {
        world.spawn(self.components);
    }
}

pub struct DespawnCommand {
    /// This entity might not actually exist yet.
    handle: EntityCommandsHandle,
}

impl Command for DespawnCommand {
    #[inline]
    fn apply(self, world: &mut World) {
        todo!()
        // world.entities.despawn(self.handle)
    }
}
