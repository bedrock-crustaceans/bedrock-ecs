use std::alloc::Layout;
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::sync::Arc;

use crate::command::{DespawnCommand, EntityCommands, EntityCommandsHandle};
use crate::component::SpawnBundle;
use crate::entity::{Entity, EntityHandle, EntityIndex};
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

struct CommandVTable {
    /// The amount of offset until the next command.
    stride: usize,
    apply_fn: ApplyFn,
    drop_fn: DropFn,
}

impl CommandVTable {
    /// # Safety
    ///
    /// The caller must ensure that this vtable is succeeded by the command describes by the vtable.
    pub(crate) unsafe fn command_ptr(vtable_ptr: NonNull<u8>) -> NonNull<u8> {
        // Cast to `NonNull<CommandVTable>` and then move to next
        // Safety: The offset clearly does not overflow and the caller condition ensures that the
        // pointer remains within the same allocation.
        unsafe { vtable_ptr.cast::<Self>().add(1) }.cast::<u8>()
    }

    /// Applies the command succeeding this vtable.
    ///
    /// # Safety
    ///
    /// The caller must ensure that this vtable is succeeded by the command describes by the vtable.
    pub unsafe fn apply(&self, vtable_ptr: NonNull<u8>, world: &mut World) {
        let cmd_ptr = unsafe { Self::command_ptr(vtable_ptr) };
        unsafe { self.apply_fn(cmd_ptr, world) };
    }

    pub unsafe fn drop_command(&self, vtable_ptr: NonNull<u8>) {
        let cmd_ptr = unsafe { Self::command_ptr(vtable_ptr) };
        unsafe { self.drop_fn(cmd_ptr) };
    }
}

type ApplyFn = unsafe fn(cmd_ptr: NonNull<u8>, world: &mut World);
type DropFn = unsafe fn(cmd_ptr: NonNull<u8>);

/// Applies the command to the given world
///
/// # Safety
///
/// The given pointer must point to a valid object of type `C`.
unsafe fn apply_command<C: Command>(cmd_ptr: NonNull<u8>, world: &mut World) {
    let command: C = unsafe { std::ptr::read(cmd_ptr.cast::<C>().as_ptr().cast_const()) };
    command.apply(world);
}

/// Calls the drop implementation of a command.
///
/// # Safety
///
/// The given pointer must point to a valid object of type `C`.
/// An applied command should not be dropped by this wrapper since the command application already drops
/// the command.
unsafe fn drop_fn<C: Command>(cmd_ptr: NonNull<u8>) {
    // Ensure that it does need to be dropped
    if std::mem::needs_drop::<C>() {
        unsafe { std::ptr::drop_in_place(cmd_ptr.cast::<C>().as_ptr()) }
    }
}

pub struct LocalCommandBuffer {
    // Deferred entity ID counter.
    next_deferred_id: u32,
    pub(crate) commands: Vec<MaybeUninit<u8>>,
}

impl LocalCommandBuffer {
    /// Resets this command buffer. Any pushed commands will be dropped without executing.
    pub fn reset(&mut self) {
        self.next_deferred_id = 0;

        let mut cursor = CommandCursor::from(self);
        cursor.drop_all();
    }

    #[inline]
    pub fn reserve(&mut self, bytes: usize) {
        self.commands.reserve(bytes);
    }

    pub fn allocate_deferred_id(&mut self) -> EntityIndex {
        let id = self.next_deferred_id;
        self.next_deferred_id += 1;
        EntityIndex(id)
    }

    #[inline]
    pub fn spare_capacity_len(&self) -> usize {
        self.commands.capacity() - self.commands.len()
    }

    pub fn push<C: Command>(&mut self, command: C) {
        tracing::trace!("pushed command");

        let cmd_layout = Layout::new::<C>();
        let tuple_layout = Layout::new::<(CommandVTable, C)>();
        let tuple_size = tuple_layout.size();

        tracing::error!("cmd layout: {cmd_layout:?}");
        tracing::error!("tuple layout {tuple_layout:?}");

        assert_eq!(tuple_size, tuple_layout.pad_to_align().size());

        let meta = CommandVTable {
            stride: tuple_size,
            apply_fn: apply_command::<C>,
            drop_fn: drop_fn::<C>,
        };

        // Ensure there is enough capacity left
        if self.spare_capacity_len() < tuple_size {
            // Allocate more capacity, also reserves some spare capacity we could maybe use for
            // another command.
            self.reserve(tuple_size);
        }

        let data = (meta, command);

        let spare_cap = self.commands.spare_capacity_mut().as_mut_ptr();
        unsafe {
            std::ptr::copy_nonoverlapping(
                (&raw const data).cast::<u8>(),
                spare_cap.cast::<u8>(),
                tuple_size,
            );
        }

        std::mem::forget(data);
        unsafe {
            self.commands.set_len(self.commands.len() + tuple_size);
        }
    }

    /// Applies all commands and clears the buffer.
    pub fn apply_all(&mut self, world: &mut World) {}
}

impl Default for LocalCommandBuffer {
    fn default() -> LocalCommandBuffer {
        Self {
            next_deferred_id: 0,
            commands: Vec::new(),
        }
    }
}

pub struct CommandCursor<'b> {
    cursor: usize,
    buffer: &'b mut LocalCommandBuffer,
}

impl CommandCursor<'_> {
    pub fn drop_all(&mut self) {
        let mut bytes = &self.buffer.commands;

        loop {
            let vtable = unsafe { std::ptr::read(bytes.as_ptr().cast::<CommandVTable>()) };
            let cmd_ptr = bytes[cursor + ]

            self.cursor += vtable.stride;
        }
    }
}

impl<'b> From<&'b mut LocalCommandBuffer> for CommandCursor<'b> {
    fn from(buffer: &'b mut LocalCommandBuffer) -> Self {
        Self { buffer }
    }
}

impl Drop for CommandCursor<'_> {
    fn drop(&mut self) {}
}

#[derive(Default, Clone)]
pub struct CommandPool {
    pool: Arc<ThreadLocal<UnsafeCell<LocalCommandBuffer>>>,
}

impl CommandPool {
    #[inline]
    pub fn get_buffer(&mut self) -> &mut LocalCommandBuffer {
        // `get_or_default` since this thread's buffer might be uninitialized.
        let cell = self.pool.get_or_default();
        unsafe { &mut *cell.get() }
    }
}

#[repr(transparent)]
pub struct Commands<'state>(pub(crate) &'state mut LocalCommandBuffer);

impl<'s> Commands<'s> {
    #[inline]
    pub fn spawn_empty(&mut self) -> EntityCommands<'_, 's> {
        let id = self.0.allocate_deferred_id();
        EntityCommands {
            entity: EntityCommandsHandle::Deferred(id),
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
