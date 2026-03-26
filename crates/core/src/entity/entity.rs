use std::ptr::NonNull;

use crate::component::ComponentBundle;
use crate::entity::{Entity, EntityGeneration, EntityIndex};
use crate::table::{Table, TableRow};
use crate::world::World;

/// Having an instance of this entity means you have exclusive access to the entire world.
///
/// This allows calling mutable methods directly rather than having to push them to command buffers.
pub struct EntityMut<'w> {
    pub(crate) world: &'w mut World,
    pub(crate) handle: Entity,
}

impl EntityMut<'_> {
    #[inline]
    pub fn handle(&self) -> Entity {
        self.handle
    }

    pub fn index(&self) -> EntityIndex {
        self.handle.index()
    }

    pub fn generation(&self) -> EntityGeneration {
        self.handle.generation()
    }

    pub fn insert(&mut self, bundle: impl ComponentBundle) {
        let meta = self
            .world
            .entities
            .get_meta(self.handle)
            .expect("`EntityMut` entity died");

        self.world.archetypes.insert(
            self.world.current_tick,
            &mut self.world.entities,
            meta,
            bundle,
        );
    }

    pub fn remove<T: ComponentBundle>(&mut self) -> Option<T> {
        let meta = self
            .world
            .entities
            .get_meta(self.handle)
            .expect("`EntityMut` entity died");

        todo!("remove components from entity");
    }

    #[inline]
    pub fn despawn(self) {
        self.world.despawn(self.handle);
    }
}

/// An entity that has immutable to the world.
#[derive(Clone)]
pub struct EntityRef<'w> {
    pub(crate) world: &'w World,
    pub(crate) handle: Entity,
}

impl EntityRef<'_> {
    /// Returns the handle of this entity.
    pub fn handle(&self) -> Entity {
        self.handle
    }

    /// Checks whether this entity has all the given components.
    ///
    /// This has relatively large overhead per entity compared to queries, so prefer using queries instead.
    pub fn has<T: ComponentBundle>(&self) -> bool {
        self.world.has_components::<T>(self.handle)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EntityMeta {
    pub handle: Entity,
    pub table: Option<NonNull<Table>>,
    pub row: TableRow,
}

unsafe impl Send for EntityMeta {}
unsafe impl Sync for EntityMeta {}
