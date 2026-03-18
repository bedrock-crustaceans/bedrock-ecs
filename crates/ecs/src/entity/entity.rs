use std::ptr::NonNull;

use crate::component::ComponentBundle;
use crate::entity::{EntityGeneration, EntityHandle, EntityIndex};
use crate::table::{Table, TableRow};
use crate::world::World;

/// Having an instance of this entity means you have exclusive access to the entire world.
///
/// This allows calling mutable methods directly rather than having to push them to command buffers.
pub struct EntityMut<'w> {
    pub(crate) world: &'w mut World,
    pub(crate) handle: EntityHandle,
}

impl EntityMut<'_> {
    #[inline]
    pub fn handle(&self) -> EntityHandle {
        self.handle
    }

    pub fn index(&self) -> EntityIndex {
        self.handle.index()
    }

    pub fn generation(&self) -> EntityGeneration {
        self.handle.generation()
    }

    #[inline]
    pub fn despawn(self) {
        todo!()
    }
}

/// An entity that has immutable to the world.
#[derive(Clone)]
pub struct EntityRef<'w> {
    pub(crate) world: &'w World,
    pub(crate) handle: EntityHandle,
}

impl<'w> EntityRef<'w> {
    /// Returns the handle of this entity.
    pub fn handle(&self) -> EntityHandle {
        self.handle
    }

    /// Checks whether this entity has all the given components.
    ///
    /// This has relatively large overhead per entity compared to queries, so prefer using queries instead.
    pub fn has<T: ComponentBundle>(&self) -> bool {
        self.world.has_components::<T>(self.handle)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    pub handle: EntityHandle,
    pub table: Option<NonNull<Table>>,
    pub row: TableRow,
}

unsafe impl Send for Entity {}
