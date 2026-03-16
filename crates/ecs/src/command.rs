use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::Arc;
use generic_array::GenericArray;
use generic_array::typenum::U0;
use thread_local::ThreadLocal;
use crate::entity::{Entity, EntityHandle};
use crate::graph::AccessDesc;
use crate::param;
use crate::param::Param;
use crate::sealed::Sealed;
use crate::spawn::SpawnBundle;
use crate::system::SystemMeta;
use crate::world::World;

pub type CommandCell = Arc<ThreadLocal<UnsafeCell<CommandQueue>>>;

#[derive(Default, Debug)]
pub struct CommandScheduler {
    queues: CommandCell
}

impl CommandScheduler {
    #[inline]
    pub fn new() -> CommandScheduler {
        CommandScheduler::default()
    }

    #[inline]
    pub fn get_queue(&self) -> CommandCell {
        Arc::clone(&self.queues)
    }
}

pub struct CommandMeta {

}

pub trait Command<Output = ()> {
    fn apply(self, world: &mut World) -> Output;
}

#[derive(Debug, Default)]
pub struct CommandQueue {
    bytes: Vec<MaybeUninit<u8>>
}

impl CommandQueue {
    
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EntityCommandsHandle {
    /// The commands will be applied to an existing entity
    Spawned(Entity),
    /// The commands will be applied to an entity that still needs to be spawned.
    /// This happens when a system spawns an entity and then also modifies it within the same
    /// tick.
    Deferred
}

pub struct EntityCommands<'s, 'c> {
    entity: EntityCommandsHandle,
    commands: &'s mut Commands<'c>
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
            EntityCommandsHandle::Deferred => None
        }
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
}

pub struct Commands<'s>(&'s mut CommandQueue);

impl<'c> Commands<'c> {
    pub fn spawn(&mut self, components: impl SpawnBundle) -> EntityCommands<'_, 'c> {
        let mut commands = EntityCommands {
            entity: EntityCommandsHandle::Deferred,
            commands: self
        };

        commands
    }

    pub fn entity<'s>(&'s mut self, entity: Entity) -> EntityCommands<'s, 'c> {
        EntityCommands {
            entity: EntityCommandsHandle::Spawned(entity),
            commands: self
        }
    }

    pub fn test(&self) {
        tracing::error!("TEST!!!!");
    }
}

unsafe impl Param for Commands<'_> {
    #[cfg(feature = "generics")]
    type AccessCount = U0;

    type Output<'s> = Commands<'s>;
    type State = CommandCell;

    #[cfg(feature = "generics")]
    #[inline]
    fn access(world: &mut World) -> GenericArray<AccessDesc, U0> {
        // Safety: This is safe because an empty generic array does not require initialization.
        unsafe {
            GenericArray::assume_init(GenericArray::<AccessDesc, U0>::uninit())
        }
    }

    #[cfg(not(feature = "generics"))]
    #[inline]
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        SmallVec::new()
    }

    #[inline]
    fn init(world: &mut World, _meta: &SystemMeta) -> CommandCell {
        world.commands.get_queue()
    }

    #[inline]
    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut CommandCell) -> Commands<'w> {
        let cell = state.get_or_default();
        Commands(unsafe { &mut *cell.get() })
    }
}