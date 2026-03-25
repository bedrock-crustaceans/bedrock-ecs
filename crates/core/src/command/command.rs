use std::cell::UnsafeCell;
use std::marker::PhantomData;

use crate::command::{EntityCommands, EntityCommandsHandle, LocalCommandBuffer, SpawnCommand};
use crate::entity::{Entity, EntityMeta};
use crate::prelude::ComponentBundle;
use crate::scheduler::AccessDesc;
use crate::sealed::Sealed;
use crate::system::{Param, SystemMeta};
#[cfg(debug_assertions)]
use crate::util::debug::{BorrowEnforcer, ReadGuard, WriteGuard};
use crate::world::World;
use generic_array::GenericArray;
use generic_array::typenum::U0;
use thread_local::ThreadLocal;

pub trait Command {
    fn apply(self, world: &mut World);
}

#[derive(Default)]
struct CommandCell {
    #[cfg(debug_assertions)]
    pub(crate) enforcer: BorrowEnforcer,
    pub(crate) buffer: UnsafeCell<LocalCommandBuffer>,
}

#[derive(Default)]
pub struct CommandPool {
    #[cfg(debug_assertions)]
    enforcer: BorrowEnforcer,
    buffers: ThreadLocal<CommandCell>,
}

impl CommandPool {
    #[inline]
    pub fn new() -> CommandPool {
        Self::default()
    }

    /// # Safety
    ///
    /// This function is only safe to call if there are no other references to the current thread's local command buffer.
    pub unsafe fn get_buffer(&self) -> Commands<'_> {
        #[cfg(debug_assertions)]
        let pool_guard = self.enforcer.read();

        let cell = self.buffers.get_or_default();

        #[cfg(debug_assertions)]
        let local_guard = cell.enforcer.write();

        Commands {
            buffer: unsafe { &mut *cell.buffer.get() },
            _marker: PhantomData,
            #[cfg(debug_assertions)]
            _pool_guard: pool_guard,
            #[cfg(debug_assertions)]
            _local_guard: local_guard,
        }
    }

    /// # Safety
    ///
    /// This function should only be called if the world has exclusive access to the command buffers.
    /// This means there should not be any [`Command`] references in existence.
    pub unsafe fn apply_all(&mut self, world: &mut World) {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        // Apply all commands
        self.buffers.iter_mut().for_each(|cmd| {
            #[cfg(debug_assertions)]
            let _guard = cmd.enforcer.write();
            let cmd = cmd.buffer.get_mut();

            // TODO: Create proper scheduler?

            cmd.apply_all(world);
        });
    }
}

pub struct Commands<'state> {
    pub(crate) buffer: &'state mut LocalCommandBuffer,
    /// Ensure that this type is !Send and !Sync.
    pub(crate) _marker: PhantomData<*const ()>,

    #[cfg(debug_assertions)]
    pub(crate) _pool_guard: ReadGuard,
    #[cfg(debug_assertions)]
    pub(crate) _local_guard: WriteGuard,
}

impl<'s> Commands<'s> {
    pub fn spawn_empty(&mut self) -> EntityCommands<'_, 's> {
        let index = self.buffer.allocate_deferred_index();
        self.buffer.push(SpawnCommand {
            handle: EntityCommandsHandle::Deferred(index),
            components: (),
        });

        EntityCommands {
            entity: EntityCommandsHandle::Deferred(index),
            commands: self,
        }
    }

    pub fn spawn(&mut self, bundle: impl ComponentBundle) -> EntityCommands<'_, 's> {
        let index = self.buffer.allocate_deferred_index();
        self.buffer.push(SpawnCommand {
            handle: EntityCommandsHandle::Deferred(index),
            components: bundle,
        });

        EntityCommands {
            entity: EntityCommandsHandle::Deferred(index),
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

impl Drop for Commands<'_> {
    fn drop(&mut self) {}
}

unsafe impl Param for Commands<'_> {
    #[cfg(feature = "generics")]
    type AccessCount = U0;

    type Output<'s> = Commands<'s>;
    type State = ();

    #[cfg(feature = "generics")]
    #[inline]
    fn access(_world: &mut World) -> GenericArray<AccessDesc, U0> {
        // Safety: This is safe because an empty generic array does not require initialization.
        unsafe { GenericArray::assume_init(GenericArray::<AccessDesc, U0>::uninit()) }
    }

    #[cfg(not(feature = "generics"))]
    #[inline]
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        SmallVec::new()
    }

    fn init(_world: &mut World, _meta: &SystemMeta) {}

    #[inline]
    fn fetch<'w, S: Sealed>(world: &'w World, _state: &'w mut ()) -> Commands<'w> {
        let Some(commands) = &world.commands else {
            panic!("World::commands was none when system tried to access");
        };

        unsafe { commands.get_buffer() }
    }
}
