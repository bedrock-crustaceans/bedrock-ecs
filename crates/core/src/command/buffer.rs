use std::alloc::Layout;
use std::io::Write;
use std::mem::MaybeUninit;
use std::ptr::NonNull;

use crate::command::Command;
use crate::entity::EntityIndex;
use crate::world::World;

/// Rounds the pointer up to the nearest alignment of type `T`.
fn align_ptr<T>(ptr: NonNull<u8>) -> NonNull<u8> {
    let align = std::mem::align_of::<T>();
    let offset = ptr.align_offset(align);

    let aligned = unsafe { ptr.add(offset) };

    debug_assert!(
        aligned.cast::<T>().is_aligned(),
        "pointer returned from `align_ptr` was not actually aligned"
    );

    aligned
}

#[derive(Debug)]
struct CommandVTable {
    /// The amount of offset until the next command.
    stride: usize,
    /// Offset from the start of this vtable to the start of the command
    padding: usize,
    /// Pointer to function that knows how to apply this command.
    apply_fn: ApplyFn,
    /// Pointer to function that knows how to drop this command. Is `None` if the type does not need to be dropped.
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

#[repr(C)]
struct CommandContainer<C: Command> {
    vtable: CommandVTable,
    cmd: C,
}

/// Applies the command to the given world
///
/// # Safety
///
/// The given pointer must point to a valid object of type `C`.
unsafe fn apply_command<C: Command>(cmd_ptr: NonNull<u8>, world: &mut World) {
    let cmd_ptr = cmd_ptr.cast::<C>();
    debug_assert!(
        cmd_ptr.is_aligned(),
        "command pointer not aligned during apply"
    );

    let command: C = unsafe { std::ptr::read(cmd_ptr.as_ptr().cast_const()) };
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
    let cmd_ptr = cmd_ptr.cast::<C>();
    debug_assert!(
        cmd_ptr.is_aligned(),
        "command pointer not aligned during drop"
    );

    // Ensure that it does need to be dropped
    if std::mem::needs_drop::<C>() {
        unsafe { std::ptr::drop_in_place(cmd_ptr.as_ptr()) }
    }
}

/// An append-only list of commands for the current thread.
#[clippy::has_significant_drop]
pub struct LocalCommandBuffer {
    // Deferred entity ID counter.
    next_deferred_id: u32,

    curr_offset: usize,
    pub(crate) commands: Vec<MaybeUninit<u8>>,
}

impl LocalCommandBuffer {
    pub fn new() -> LocalCommandBuffer {
        Self {
            next_deferred_id: 0,
            curr_offset: 0,
            // Allocate some capacity so we already have a pointer to work with.
            commands: Vec::with_capacity(20),
        }
    }

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

    pub fn push<C: Command>(&mut self, cmd: C) {
        let vtable = CommandVTable {
            stride: 0,  // Will be set later
            padding: 0, // Will be set later
            apply_fn: apply_command::<C>,
            drop_fn: std::mem::needs_drop::<C>().then_some(drop_fn::<C>),
        };

        let start_len = self.commands.len();

        // Add enough to the total size to cover the alignment. This ensures that after reserving capacity, if the pointer is suddenly at
        // a location that needs an even larger offset, it is already covered.
        let size_upper_bound = std::mem::align_of::<CommandVTable>()
            + std::mem::size_of::<CommandVTable>()
            + std::mem::align_of::<C>()
            + std::mem::size_of::<C>();

        // Ensure there is enough capacity left for the worst possible alignment situation.
        let spare_cap = self.commands.spare_capacity_mut().len();
        if spare_cap < size_upper_bound {
            // Clamp to the current capacity to make sure that the capacity doubles every time.
            // This is similar to the reallocation strategy normally used by Vec.
            let size = size_upper_bound.clamp(self.commands.capacity(), usize::MAX);
            self.commands.reserve(size);

            println!("reserved {size}");
        }

        // vec has enough capacity, we can now create a pointer to the data
        let start_ptr = self.commands.spare_capacity_mut().as_mut_ptr().cast::<u8>();
        let align_offset = start_ptr.align_offset(std::mem::align_of::<CommandVTable>());
        assert!(align_offset < std::mem::align_of::<CommandVTable>());
        println!(
            "vtable offset: {align_offset}, size: {}",
            std::mem::size_of::<CommandVTable>()
        );

        let vtable_size = align_offset + std::mem::size_of::<CommandVTable>();
        let vtable_ptr = unsafe { start_ptr.add(align_offset).cast::<CommandVTable>() };

        debug_assert!(
            vtable_ptr.is_aligned(),
            "command vtable write pointer is not aligned"
        );

        unsafe {
            std::ptr::write(vtable_ptr, vtable);
            self.commands.set_len(self.commands.len() + vtable_size);
        }

        // Go to end of vtable.
        let cmd_ptr = unsafe {
            vtable_ptr
                .cast::<u8>()
                .add(std::mem::size_of::<CommandVTable>())
        };

        // Then add padding for alignment
        let align_offset = cmd_ptr.align_offset(std::mem::align_of::<C>());
        assert!(align_offset < std::mem::align_of::<C>());

        let cmd_size = align_offset + std::mem::size_of::<C>();
        let cmd_ptr = unsafe { cmd_ptr.add(align_offset).cast::<C>() };

        println!(
            "cmd offset: {align_offset}, cmd size: {}",
            std::mem::size_of::<C>()
        );

        debug_assert!(cmd_ptr.is_aligned(), "command write pointer is not aligned");
        debug_assert!(self.commands.len() + cmd_size <= self.commands.capacity());

        unsafe {
            std::ptr::write(cmd_ptr, cmd);
            self.commands.set_len(self.commands.len() + cmd_size);
        }

        let stride = self.commands.len() - start_len;
        let vtable = unsafe { &mut *vtable_ptr };

        vtable.stride = stride;
        vtable.padding = align_offset + std::mem::size_of::<CommandVTable>();
        println!("vtable: {vtable:?}");
    }

    #[expect(
        clippy::missing_panics_doc,
        reason = "assert for safety, but should realistically never be triggered"
    )]
    pub fn apply_all(&mut self, world: &mut World) {
        tracing::trace!("applying all commands");

        let len = self.commands.len();
        unsafe { self.commands.set_len(0) };

        let mut offset = 0;
        while offset < len {
            let mut curr_ptr = unsafe {
                NonNull::new_unchecked(self.commands.as_mut_ptr().add(offset)).cast::<u8>()
            };
            let align_offset = curr_ptr.align_offset(std::mem::align_of::<CommandVTable>());
            println!("vtable align: {align_offset}");
            assert!(align_offset < std::mem::align_of::<CommandVTable>());

            curr_ptr = unsafe { curr_ptr.add(align_offset) };
            let vtable = unsafe { &*curr_ptr.cast::<CommandVTable>().as_ptr() };

            println!("vtable: {vtable:?}");

            let apply_fn = vtable.apply_fn;
            let cmd_ptr = unsafe { curr_ptr.add(vtable.padding) };

            unsafe {
                apply_fn(cmd_ptr, world);
            }

            offset += vtable.stride;
            println!("offset is now {offset}");
        }
    }

    pub(crate) fn drop_all(&mut self) {
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

            unsafe { vtable.drop_command(cmd_ptr) };

            // Skip over vtable and data to reach next command.
            offset += vtable.stride;
        }
    }
}

impl Default for LocalCommandBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LocalCommandBuffer {
    fn drop(&mut self) {
        self.drop_all();
    }
}
