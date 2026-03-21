use std::alloc::Layout;
use std::mem::MaybeUninit;
use std::ptr::NonNull;

use crate::command::Command;
use crate::entity::EntityIndex;
use crate::world::World;

struct CommandVTable {
    /// The amount of offset until the next command.
    stride: usize,
    apply_fn: ApplyFn,
    drop_fn: Option<DropFn>,
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
        let apply_fn = self.apply_fn;
        unsafe { apply_fn(cmd_ptr, world) };
    }

    pub unsafe fn drop_command(&self, vtable_ptr: NonNull<u8>) {
        if let Some(drop_fn) = self.drop_fn {
            let cmd_ptr = unsafe { Self::command_ptr(vtable_ptr) };
            unsafe { drop_fn(cmd_ptr) };
        }
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

/// An append-only list of commands for the current thread.
#[clippy::has_significant_drop]
#[derive(Default)]
pub struct LocalCommandBuffer {
    // Deferred entity ID counter.
    next_deferred_id: u32,
    pub(crate) commands: Vec<MaybeUninit<u8>>,
}

impl LocalCommandBuffer {
    /// Resets this command buffer. Any pushed commands will be dropped without executing.
    pub fn reset(&mut self) {
        self.next_deferred_id = 0;
        self.drop_all();
    }

    #[inline]
    pub fn reserve(&mut self, bytes: usize) {
        self.commands.reserve(bytes);
    }

    #[must_use]
    pub fn allocate_deferred_index(&mut self) -> EntityIndex {
        let id = self.next_deferred_id;
        self.next_deferred_id += 1;
        EntityIndex(id)
    }

    #[inline]
    pub fn spare_capacity_len(&self) -> usize {
        self.commands.capacity() - self.commands.len()
    }

    pub fn push<C: Command>(&mut self, command: C) {
        let tuple_layout = Layout::new::<(CommandVTable, C)>();
        let tuple_size = tuple_layout.size();

        let meta = CommandVTable {
            stride: tuple_size,
            apply_fn: apply_command::<C>,
            drop_fn: std::mem::needs_drop::<C>().then_some(drop_fn::<C>),
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

    #[expect(
        clippy::missing_panics_doc,
        reason = "assert for safety, but should realistically never be triggered"
    )]
    pub fn apply_all(&mut self, world: &mut World) {
        tracing::error!("applying all commands");

        let buffer_len = self.commands.len();

        assert!(
            buffer_len < isize::MAX as usize,
            "command buffer length exceeds `isize::MAX`"
        );

        // Set length to 0 immediately to ensure that no uninit memory is read in case of a panic.
        // Safety: This is sound because an empty vector has no items and 0 is always less than or equal
        // to any capacity.
        unsafe {
            self.commands.set_len(0);
        }

        let mut offset = 0;
        while offset < buffer_len {
            // Safety: This is safe because the pointer is derived from an initialised slice
            // and therefore cannot be null.
            let start_ptr: NonNull<u8> = unsafe {
                NonNull::new_unchecked(self.commands.as_mut_ptr())
                    .add(offset)
                    .cast::<u8>()
            };

            let vtable_ptr = start_ptr.cast::<CommandVTable>();
            let vtable = unsafe { std::ptr::read(vtable_ptr.as_ptr().cast_const()) };

            // Skip over the vtable and cast back to raw pointer.
            // Safety:
            // This is safe because every vtable is always followed by a command, thus this pointer
            // will not be outside of the allocation. Furthermore, the `add` operation skips over exactly
            // one [`CommandVTable`] which will not exceed `isize::MAX` in size.
            let cmd_ptr = unsafe { vtable_ptr.add(1) }.cast::<u8>();

            unsafe { vtable.apply(cmd_ptr, world) };

            // Skip over vtable and data to reach next command.
            offset += vtable.stride;
        }
    }

    pub(crate) fn drop_all(&mut self) {
        let buffer_len = self.commands.len();

        assert!(
            buffer_len < isize::MAX as usize,
            "Command buffer length exceeds `isize::MAX`"
        );

        // Set length to 0 immediately to ensure that no uninit memory is read in case of a panic.
        // Safety: This is sound because an empty vector has no items and 0 is always less than or equal
        // to any capacity.
        unsafe {
            self.commands.set_len(0);
        }

        let mut offset = 0;
        while offset < buffer_len {
            // Safety: This is safe because the pointer is derived from an initialised slice
            // and therefore cannot be null.
            let start_ptr: NonNull<u8> = unsafe {
                NonNull::new_unchecked(self.commands.as_mut_ptr())
                    .add(offset)
                    .cast::<u8>()
            };

            let vtable_ptr = start_ptr.cast::<CommandVTable>();
            let vtable = unsafe { std::ptr::read(vtable_ptr.as_ptr().cast_const()) };

            // Skip over the vtable and cast back to raw pointer.
            // Safety:
            // This is safe because every vtable is always followed by a command, thus this pointer
            // will not be outside of the allocation. Furthermore, the `add` operation skips over exactly
            // one [`CommandVTable`] which will not exceed `isize::MAX` in size.
            let cmd_ptr = unsafe { vtable_ptr.add(1) }.cast::<u8>();

            unsafe { vtable.drop_command(cmd_ptr) };

            // Skip over vtable and data to reach next command.
            offset += vtable.stride;
        }
    }
}

impl Drop for LocalCommandBuffer {
    fn drop(&mut self) {
        self.drop_all();
    }
}
