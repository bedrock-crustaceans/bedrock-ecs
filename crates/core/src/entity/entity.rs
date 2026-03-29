use std::ptr::NonNull;

use crate::component::ComponentBundle;
use crate::entity::{Entity, EntityGeneration, EntityIndex};
use crate::table::{ColumnRow, Table};
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

    #[expect(
        clippy::missing_panics_doc,
        reason = "it is not possible for the entity to be despawned while this type exists"
    )]
    pub fn insert(&mut self, bundle: impl ComponentBundle) {
        // meta is not stored inside of the entity since it could change while `self` is alive.
        let meta = self
            .world
            .entities
            .get_meta(self.handle)
            .expect("`EntityMut` entity died, this is impossible");

        self.world.archetypes.insert(
            self.world.current_tick,
            &mut self.world.entities,
            meta,
            bundle,
        );
    }

    #[inline]
    pub fn remove<B: ComponentBundle>(&mut self) -> Option<B> {
        // meta is not stored inside of the entity since it could change while `self` is alive.
        let meta = self
            .world
            .entities
            .get_meta(self.handle)
            .expect("`EntityMut` entity died, this is impossible");

        self.world
            .archetypes
            .remove::<B>(self.world.current_tick, &mut self.world.entities, meta)
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

/// Describes where an entity's data is located.
///
/// This is an unstable reference and should only be used internally. Entity references
/// exposed to the user should make use of [`Entity`] instead, which provides a stable reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityMeta {
    /// The index and generation of this entity.
    pub handle: Entity,
    /// Pointer to the table this entity is currently located in.
    pub table: NonNull<Table>,
    /// Row inside the `table` that stores this entity's components.
    pub row: ColumnRow,
}

unsafe impl Send for EntityMeta {}
unsafe impl Sync for EntityMeta {}
