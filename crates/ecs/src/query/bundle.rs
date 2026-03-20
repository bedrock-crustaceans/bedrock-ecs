///! Implements the [`QueryBundle`] and [`ParamRef`] related traits.
use std::any::TypeId;
use std::ops::{Add, Deref, DerefMut};

use generic_array::{ArrayLength, GenericArray};
use rustc_hash::FxHashMap;

use crate::archetype::Signature;
use crate::component::{Component, ComponentId, ComponentRegistry};
use crate::entity::{Entity, EntityRef};
use crate::query::{EmptyableIterator, HoppingIterator};
use crate::scheduler::{AccessDesc, AccessType};
use crate::table::{ChangeTracker, ColumnIter, ColumnIterMut, EntityIter, EntityRefIter, Mut, Ref};
use crate::world::World;

/// A collection of types that can be queried.
///
/// This is implemented for tuples of types that implement [`ParamRef`].
/// In other words, this represents collection of component references or entities that appear
/// inside a `Query<...>`.
///
/// # Safety:
///
/// The `access` method must correctly return the types this query uses.
/// Incorrect implementation will lead to reference aliasing and inevitable UB.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid query type",
    label = "invalid query",
    // note = "only `Entity`, `&T` and `&mut T` where `T: Component` or tuples thereof can be used in queries",
    note = "components in a query must be wrapped in a reference, e.g. `&{Self}` or `&mut {Self}`",
    note = "if `{Self}` is a component, do not forget to implement the `Component` trait"
)]
pub unsafe trait QueryBundle: Sized {
    #[cfg(feature = "generics")]
    /// The amount of resources that this query accesses.
    type AccessCount: ArrayLength + Add;

    /// The item that the query outputs. This is what is actually given to the system when ran.
    type Output<'a>
    where
        Self: 'a;

    #[cfg(feature = "generics")]
    /// The type of iterator over the columns. Every collection size has a different iterator type
    /// specialised for its size. These iterators are [`IteratorBundle1`], [`IteratorBundle2`], ...
    type Iter<'a>: HoppingIterator<'a, Self> + Iterator<Item = Self::Output<'a>>
    where
        Self: 'a;

    #[cfg(not(feature = "generics"))]
    /// The type of iterator over the columns. Every collection size has a different iterator type
    /// specialised for its size. These iterators are [`IteratorBundle1`], [`IteratorBundle2`], ...
    type Iter<'a>: HoppingIterator<'a> + Iterator<Item = Self::Output<'a>>
    where
        Self: 'a;

    /// The amount of items in this bundle.
    const LEN: usize;

    /// Returns the signature of this query. This signature does not include possible filters.
    fn signature(reg: &mut ComponentRegistry) -> Signature;

    #[cfg(feature = "generics")]
    /// A list of resources that this query wants to access. This is forwarded to the scheduler
    /// to avoid conflicts and schedule optimally.
    fn access(reg: &mut ComponentRegistry) -> GenericArray<AccessDesc, Self::AccessCount>;

    #[cfg(feature = "generics")]
    /// Finds all required columns from a lookup table.
    ///
    /// When the query cache updates, it will very quickly collect all tables that contain the desired components.
    /// It however is not aware of the columns. This function then figures out which columns are useful
    /// and in which order they should be queried.
    fn cache_columns(lookup: &FxHashMap<TypeId, usize>) -> GenericArray<usize, Self::AccessCount>;

    #[cfg(not(feature = "generics"))]
    /// A list of resources that this query wants to access. This is forwarded to the scheduler
    /// to avoid conflicts and schedule optimally.
    fn access(reg: &mut ComponentRegistry) -> SmallVec<[AccessDesc; param::INLINE_SIZE]>;

    #[cfg(not(feature = "generics"))]
    /// Finds all required columns from a lookup table.
    ///
    /// When the query cache updates, it will very quickly collect all tables that contain the desired components.
    /// It however is not aware of the columns. This function then figures out which columns are useful
    /// and in which order they should be queried.
    fn cache_columns(lookup: &FxHashMap<TypeId, usize>) -> SmallVec<[usize; param::INLINE_SIZE]>;
}

/// A reference that can be used in a query. This is either [`Entity`], or a mutable/immutable reference
/// to a type implementing [`Component`].
///
/// # Safety
///
/// Implementors of this trait should uphold the following conditions:
/// - `Unref` must be the exact type you would get if you were to remove the reference, i.e. if `Self = &T` then
/// `Self::Unref` must be `T`.
///
/// - `Output<'w>` must equal `Self` but with its lifetime bound to `'w`. Incorrect lifetimes will lead to use after
/// free situations.
///
/// - `Iter<'t>` must be an iterator that only returns mutable references if `Self`'s access descriptor also
/// indicates it requires mutable access.
///
/// - `IS_ENTITY` must only be set to true when implementing this trait for [`Entity`].
///
/// - `access` must return the correct descriptor, indicating which resources this parameter uses.
/// Incorrect descriptors will cause undefined behaviour through mutable reference aliasing.
///
/// - `component_id` must return the correct ID for `Self::Unref`. Incorrect component IDs will cause the query
/// cache to read the wrong columns, which means data is interpreted with the incorrect type.
///
/// [`Component`]: crate::component::Component
/// [`Entity`]: crate::entity::EntityRef
pub unsafe trait ParamRef: Send {
    /// The type you would get if you were to remove the reference attached to `Self`.
    type Unref: 'static;

    /// The type that is returned by the query. This is equal to `Self` but with a restricted lifetime
    /// to ensure that the queried types do not outlive the query and world itself.
    type Output<'w>: 'w;

    /// Iterator used to iterate over columns of type `Self`.
    type Iter<'t>: EmptyableIterator<'t, Self::Output<'t>>;

    /// Whether this parameter is an entity.
    const TY: QueryType;

    /// Returns the resource that this parameter accessess.
    fn access(reg: &mut ComponentRegistry) -> AccessDesc;

    /// Returns the component ID of this type.
    ///
    /// # Panics
    ///
    /// This function panics when `Self` is an entity since entities do not have a component ID.
    fn component_id(reg: &mut ComponentRegistry) -> ComponentId;

    /// Returns column index that `Self` is contained in.
    ///
    /// # Panics
    ///
    /// This function panics when `Self` is an entity since entities are not stored in columns.
    /// It also panics if the column is not found.
    fn cache_column(map: &FxHashMap<TypeId, usize>) -> usize;

    /// Returns an iterator over the column in the given table.
    ///
    /// If `Self` is an entity then this returns an iterator over the entities in the table.
    fn iter(world: &World, table: usize, col: usize) -> Self::Iter<'_>;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum QueryType {
    Component,
    Entity,
    EntityRef,
}

unsafe impl ParamRef for Entity {
    type Unref = Entity;
    type Output<'w> = Entity;
    type Iter<'t> = EntityIter<'t>;

    const TY: QueryType = QueryType::Entity;

    #[inline]
    fn access(_reg: &mut ComponentRegistry) -> AccessDesc {
        AccessDesc {
            ty: AccessType::None,
            exclusive: false,
        }
    }

    fn component_id(_reg: &mut ComponentRegistry) -> ComponentId {
        unimplemented!("cannot call `component_id` on `Entity`")
    }

    fn cache_column(_map: &FxHashMap<TypeId, usize>) -> usize {
        unimplemented!("cannot call `cache_column` on `Entity`")
    }

    fn iter(world: &World, table: usize, _col: usize) -> EntityIter<'_> {
        let table = world.archetypes.get_by_index(table);
        table.iter_entities(world)
    }
}

unsafe impl ParamRef for EntityRef<'_> {
    type Unref = EntityRef<'static>;
    type Output<'w> = EntityRef<'w>;
    type Iter<'t> = EntityRefIter<'t>;

    const TY: QueryType = QueryType::EntityRef;

    fn access(_reg: &mut ComponentRegistry) -> AccessDesc {
        AccessDesc {
            ty: AccessType::World,
            exclusive: false,
        }
    }

    fn component_id(_reg: &mut ComponentRegistry) -> ComponentId {
        unreachable!("attempt to lookup component ID of entity");
    }

    fn cache_column(_map: &FxHashMap<TypeId, usize>) -> usize {
        unreachable!("attempt to lookup column index of entity");
    }

    fn iter(world: &World, table: usize, _col: usize) -> EntityRefIter<'_> {
        let table = world.archetypes.get_by_index(table);
        table.iter_entity_refs(world)
    }
}

unsafe impl<T: Component + Send + Sync> ParamRef for &T {
    type Unref = T;
    type Output<'w> = Ref<'w, T>;
    type Iter<'t> = ColumnIter<'t, T>;

    const TY: QueryType = QueryType::Component;

    fn access(reg: &mut ComponentRegistry) -> AccessDesc {
        AccessDesc {
            ty: AccessType::Component(reg.get_or_assign::<T>()),
            exclusive: false,
        }
    }

    fn component_id(reg: &mut ComponentRegistry) -> ComponentId {
        reg.get_or_assign::<T>()
    }

    fn cache_column(map: &FxHashMap<TypeId, usize>) -> usize {
        let col = *map.get(&TypeId::of::<T>()).expect(&format!(
            "table column lookup failed for component {}",
            std::any::type_name::<T>()
        ));

        col
    }

    fn iter(world: &World, table: usize, col: usize) -> ColumnIter<'_, T> {
        let table = world.archetypes.get_by_index(table);
        let col = table.column(col);

        col.iter()
    }
}

unsafe impl<T: Component + Send + Sync> ParamRef for &mut T {
    type Unref = T;
    type Output<'w> = Mut<'w, T>;
    type Iter<'t> = ColumnIterMut<'t, T>;

    const TY: QueryType = QueryType::Component;

    fn access(reg: &mut ComponentRegistry) -> AccessDesc {
        AccessDesc {
            ty: AccessType::Component(reg.get_or_assign::<T>()),
            exclusive: true,
        }
    }

    fn component_id(reg: &mut ComponentRegistry) -> ComponentId {
        reg.get_or_assign::<T>()
    }

    fn cache_column(map: &FxHashMap<TypeId, usize>) -> usize {
        let col = *map.get(&TypeId::of::<T>()).expect(&format!(
            "table column lookup failed for component {}",
            std::any::type_name::<T>()
        ));

        col
    }

    fn iter<'t>(world: &'t World, table: usize, col: usize) -> ColumnIterMut<'t, T> {
        let table = world.archetypes.get_by_index(table);
        let col = table.column(col);
        col.iter_mut()
    }
}
