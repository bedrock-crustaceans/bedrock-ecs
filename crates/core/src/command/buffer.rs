use std::ptr::NonNull;

use nonmax::{NonMaxU32, NonMaxUsize};

use crate::command::Command;
use crate::entity::EntityIndex;
use crate::world::World;

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
pub struct LocalCommandQueue {
    // Deferred entity ID counter.
    next_deferred_id: u32,
    // TODO: This should be done with a raw memory buffer to reduce pointer indirection.
    buffer: Vec<Box<dyn Command>>,
}

impl LocalCommandQueue {
    const ALIGN: usize = 8;

    pub fn new() -> LocalCommandQueue {
        Self {
            next_deferred_id: 0,
            buffer: Vec::with_capacity(16),
        }
    }

    /// Resets this command buffer. Any pushed commands will be dropped without executing.
    pub fn reset(&mut self) {
        self.next_deferred_id = 0;
        self.buffer.clear();
    }

    #[must_use]
    pub fn allocate_deferred_index(&mut self) -> EntityIndex {
        let id = self.next_deferred_id;
        self.next_deferred_id += 1;
        EntityIndex(NonMaxU32::new(id))
    }

    pub fn push<C: Command>(&mut self, cmd: C) {
        let cmd = Box::new(cmd);
        self.buffer.push(cmd);
    }

    #[expect(
        clippy::missing_panics_doc,
        reason = "assert for safety, but should realistically never be triggered"
    )]
    pub fn apply_all(&mut self, world: &mut World) {
        println!("Local buffer has {} commands", self.buffer.len());

        self.buffer.drain(..).for_each(|cmd| {
            cmd.apply(world);
        })
    }
}

impl Default for LocalCommandQueue {
    fn default() -> Self {
        Self::new()
    }
}
