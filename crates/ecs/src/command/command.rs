use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::Arc;

use crate::command::{DespawnCommand, EntityCommands, EntityCommandsHandle};
use crate::component::SpawnBundle;
use crate::entity::{Entity, EntityHandle};
use crate::scheduler::AccessDesc;
use crate::sealed::Sealed;
use crate::system::{Param, SystemMeta};
use crate::world::World;
use generic_array::GenericArray;
use generic_array::typenum::U0;
use thread_local::ThreadLocal;

pub trait Command {
    fn apply(self, world: &mut World);
}

#[derive(Default)]
pub struct CommandBuffers {
    pool: CommandPool,
}

impl CommandBuffers {
    #[inline]
    pub fn get_pool(&self) -> CommandPool {
        self.pool.clone()
    }
}

#[derive(Default)]
pub struct ThreadCommandBuffer {}

impl ThreadCommandBuffer {
    pub fn push(&mut self, command: impl Command) {
        tracing::trace!("pushed command");

        todo!()
    }
}

#[derive(Default, Clone)]
pub struct CommandPool {
    pool: Arc<ThreadLocal<UnsafeCell<ThreadCommandBuffer>>>,
}

impl CommandPool {
    #[inline]
    pub fn get_buffer(&mut self) -> &mut ThreadCommandBuffer {
        // `get_or_default` since this thread's buffer might be uninitialized.
        let cell = self.pool.get_or_default();
        unsafe { &mut *cell.get() }
    }
}

#[repr(transparent)]
pub struct Commands<'state>(pub(crate) &'state mut ThreadCommandBuffer);

impl<'s> Commands<'s> {
    #[inline]
    pub fn spawn_empty(&mut self) -> EntityCommands<'_, 's> {
        EntityCommands {
            entity: EntityCommandsHandle::Deferred,
            commands: self,
        }
    }

    #[inline]
    pub fn entity(&mut self, entity: Entity) -> EntityCommands<'_, 's> {
        EntityCommands {
            entity: EntityCommandsHandle::Spawned(entity),
            commands: self,
        }
    }
}

unsafe impl Param for Commands<'_> {
    #[cfg(feature = "generics")]
    type AccessCount = U0;

    type Output<'s> = Commands<'s>;
    type State = CommandPool;

    #[cfg(feature = "generics")]
    #[inline]
    fn access(world: &mut World) -> GenericArray<AccessDesc, U0> {
        // Safety: This is safe because an empty generic array does not require initialization.
        unsafe { GenericArray::assume_init(GenericArray::<AccessDesc, U0>::uninit()) }
    }

    #[cfg(not(feature = "generics"))]
    #[inline]
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        SmallVec::new()
    }

    #[inline]
    fn init(world: &mut World, _meta: &SystemMeta) -> CommandPool {
        // Initialise the
        world.commands.get_pool()
    }

    #[inline]
    fn fetch<'w, S: Sealed>(_world: &'w World, state: &'w mut CommandPool) -> Commands<'w> {
        let buffer = state.get_buffer();
        Commands(buffer)
    }
}
