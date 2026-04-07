//! Implements the [`QueryGroup`] trait.

use std::any::TypeId;
#[cfg(feature = "generics")]
use std::fmt::Debug;
use std::ops::Add;
use std::ptr::NonNull;

use generic_array::{ArrayLength, GenericArray};
use nonmax::NonMaxUsize;
use rustc_hash::FxHashMap;

use crate::archetype::Signature;
use crate::component::{Component, ComponentId, TypeRegistry};
use crate::entity::{Entity, EntityRef};
use crate::query::{Filter, FragmentIterator, QueryIter, QueryState};
use crate::scheduler::{AccessDesc, AccessType};
use crate::table::{ColumnRow, Mut, Ref, Table};
use crate::util::{AsConstNonNull, ConstNonNull, SendWrapper};
use crate::world::World;

/// A collection of types that can be queried.
///
/// This is implemented for tuples of types that implement [`QueryData`].
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
pub unsafe trait QueryGroup: Sized {
    #[cfg(feature = "generics")]
    /// The amount of resources that this query accesses.
    type AccessCount: ArrayLength + Add + Debug;

    /// The tuple that is produced by this bundle. This is the type that iterators using
    /// this query will return.
    type Output<'a>
    where
        Self: 'a;

    /// Needs to be `Send` to allow sending the base pointers to other threads in parallel iterators.
    type BasePtrs: Copy + Send;

    /// The size of the tuple.
    const LEN: usize;

    /// Returns the signature of this query. This signature does not include possible filters.
    fn signature(reg: &mut TypeRegistry) -> Signature;

    /// Attempts to fetch a single entity from the query.
    ///
    /// `F` is the filter that should be applied to this operation. If the entity
    /// did not contain the components in this query bundle or the filter did not match,
    /// `None` will be returned.
    ///
    /// This uses a lookup table internally and can be significantly faster than iterating over the query to find the entity.
    fn get<'t, F: Filter>(
        world: &'t World,
        state: &'t QueryState<Self, F>,
        table: &'t Table,
        row: ColumnRow,
    ) -> Option<Self::Output<'t>>
    where
        Self: 't;

    #[cfg(feature = "generics")]
    /// A list of resources that this query wants to access. This is forwarded to the scheduler
    /// to avoid conflicts and schedule optimally.
    fn access(reg: &mut TypeRegistry) -> GenericArray<AccessDesc, Self::AccessCount>;

    #[cfg(feature = "generics")]
    /// Finds all required columns from a lookup table.
    ///
    /// The query is able to figure out which tables it should iterate over by itself.
    /// After finding a matching table, this function is then called to map the components in the query bundle directly to their
    /// corresponding columns in the table.
    ///
    /// Internally this function calls [`map_column`] on each item in the bundle.
    ///
    /// [`map_column`]: QueryData::map_column
    fn get_base_ptrs(table: &Table) -> Self::BasePtrs;

    unsafe fn fetch_from_base<'w>(ptrs: &mut Self::BasePtrs, current_tick: u32)
    -> Self::Output<'w>;

    fn dangling() -> Self::BasePtrs;

    #[cfg(not(feature = "generics"))]
    /// A list of resources that this query wants to access. This is forwarded to the scheduler
    /// to avoid conflicts and schedule optimally.
    fn access(reg: &mut TypeRegistry) -> SmallVec<[AccessDesc; SysArg::INLINE_SIZE]>;

    #[cfg(not(feature = "generics"))]
    /// Finds all required columns from a lookup table.
    ///
    /// When the query cache updates, it will very quickly collect all tables that contain the desired components.
    /// It however is not aware of the columns. This function then figures out which columns are useful
    /// and in which order they should be queried.
    fn cache_columns(lookup: &FxHashMap<TypeId, usize>) -> SmallVec<[usize; SysArg::INLINE_SIZE]>;
}

/// A reference that can be used in a query. This is either [`Entity`], or a mutable/immutable reference
/// to a type implementing [`Component`].
///
/// # Safety
///
/// Implementors of this trait should uphold the following conditions:
/// - `Unref` must be the exact type you would get if you were to remove the reference, i.e. if `Self = &T` then
///   `Self::Unref` must be `T`.
///
/// - `Output<'w>` must equal `Self` but with its lifetime bound to `'w`. Incorrect lifetimes will lead to use after
///   free situations.
///
/// - `Iter<'t>` must be an iterator that only returns mutable references if `Self`'s access descriptor also
///   indicates it requires mutable access.
///
/// - `IS_ENTITY` must only be set to true when implementing this trait for [`Entity`].
///
/// - `access` must return the correct descriptor, indicating which resources this system argument uses.
///   Incorrect descriptors will cause undefined behaviour through mutable reference aliasing.
///
/// - `component_id` must return the correct ID for `Self::Unref`. Incorrect component IDs will cause the query
///   cache to read the wrong columns, which means data is interpreted with the incorrect type.
///
/// [`Component`]: crate::component::Component
/// [`Entity`]: crate::entity::Entity
pub unsafe trait QueryData {
    /// Removes all references from `Self`.
    type Deref: 'static;

    /// The type returned by the query. This does not have to equal `Self`.
    ///
    /// For components this is used to bound the lifetime to the query while other exotic types
    /// like [`Has`] use it to output a completely different type.
    ///
    /// [`Has`]: crate::query::Has
    type Output<'w>: 'w;

    /// The base pointer of this data.
    ///
    /// This is the pointer that points to the start of the data column.
    ///
    /// For some specialized types (such as [`Has`]) this can also be non-pointer data.
    ///
    /// Needs to be `Send` to allow sending to other threads in parallel iterators.
    ///
    /// [`Has`]: crate::query::Has
    type BasePtr: Copy + Send;

    const TY: QueryType;

    /// Returns the resource that this system argument accessess.
    fn access(reg: &mut TypeRegistry) -> AccessDesc;

    // /// Finds the index of the column that contains this type, in the given table.
    // ///
    // /// # Panics
    // ///
    // /// This function panics when `Self` is an entity since entities are not stored in columns.
    // /// It also panics if the column is not found.
    // fn map_column(table: &Table) -> NonMaxUsize;

    fn get_base_ptr(table: &Table) -> Self::BasePtr;

    fn dangling() -> Self::BasePtr;

    unsafe fn fetch_from_base<'w>(base: &mut Self::BasePtr, current_tick: u32) -> Self::Output<'w>;

    /// Attempts to fetch the component of type `Self` that is contained in the given table, column and row.
    ///
    /// This is used by [`Query::get`] to fetch a single entity.
    ///
    /// [`Query::get`]: crate::query::Query::get
    fn get<'w, Q: QueryGroup, F: Filter>(
        world: &'w World,
        state: &'w QueryState<Q, F>,
        table: &'w Table,
        row: ColumnRow,
        col: Option<NonMaxUsize>,
    ) -> Option<Self::Output<'w>>;
}

/// The type of the query data. This is used inside of queries to figure out what the query should
/// fetch.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum QueryType {
    Component,
    Entity,
    Has,
}

impl QueryType {
    /// Whether `Self == Self::Component`.
    ///
    /// This is a separate function because `PartialEq` is `const`-unstable.
    pub const fn is_component(&self) -> bool {
        matches!(self, Self::Component)
    }
}

/// Fetches the entity handle associated with the components. [`Entity`] is a stable reference and can be stored
/// inside of other components to be used later.
///
/// If the query does not contain any components, all entities in the entire world will be fetched. If it does,
/// only entities with the specified components will be returned.
unsafe impl QueryData for Entity {
    type Deref = Entity;
    type Output<'w> = Entity;
    type BasePtr = ConstNonNull<Entity>;

    const TY: QueryType = QueryType::Entity;

    #[inline]
    fn access(_reg: &mut TypeRegistry) -> AccessDesc {
        AccessDesc {
            ty: AccessType::None,
            mutable: false,
        }
    }

    #[inline]
    fn get_base_ptr(table: &Table) -> Self::BasePtr {
        table.entities.as_const_non_null()
    }

    fn dangling() -> Self::BasePtr {
        ConstNonNull::dangling()
    }

    #[inline]
    unsafe fn fetch_from_base<'w>(
        base: &mut Self::BasePtr,
        _current_tick: u32,
    ) -> Self::Output<'w> {
        let item = unsafe { *base.as_ptr() };
        *base = unsafe { base.add(1) };
        item
    }

    fn get<'w, Q: QueryGroup, F: Filter>(
        _world: &'w World,
        _state: &'w QueryState<Q, F>,
        table: &'w Table,
        row: ColumnRow,
        col: Option<NonMaxUsize>,
    ) -> Option<Self::Output<'w>> {
        debug_assert!(
            col.is_none(),
            "column index passed to entity handle iterator",
        );

        table.get_entity(row.0)
    }
}

/// Requests immutable access to a component of type `T`.
///
/// Rather than returning `&T` directly, queries will give the `Ref<T>` type which automatically
/// dereferences to `T`.
///
/// # Access
///
/// Components also follow Rust's aliasing rules. Systems that request immutable access to components
/// can be scheduled in parallel with other systems requesting an immutable reference to same components. Any systems
/// that request a mutable reference will be given exclusive access to the component.
unsafe impl<T: Component> QueryData for &T {
    type Deref = T;
    type Output<'w> = &'w T;
    type BasePtr = ConstNonNull<T>;

    const TY: QueryType = QueryType::Component;

    #[inline]
    fn access(reg: &mut TypeRegistry) -> AccessDesc {
        AccessDesc {
            ty: AccessType::Component(reg.get_or_assign::<T>()),
            mutable: false,
        }
    }

    #[inline]
    fn get_base_ptr(table: &Table) -> Self::BasePtr {
        let col = table
            .get_column_by_type(&TypeId::of::<T>())
            .expect("expected column not found in table");

        ConstNonNull::from(col.base_ptr())
    }

    #[inline]
    unsafe fn fetch_from_base<'w>(
        base: &mut Self::BasePtr,
        _current_tick: u32,
    ) -> Self::Output<'w> {
        let item = unsafe { &*base.as_ptr() };
        *base = unsafe { base.add(1) };
        item
    }

    fn dangling() -> Self::BasePtr {
        ConstNonNull::dangling()
    }

    fn get<'w, Q: QueryGroup, F: Filter>(
        _world: &'w World,
        _state: &'w QueryState<Q, F>,
        table: &'w Table,
        row: ColumnRow,
        col: Option<NonMaxUsize>,
    ) -> Option<Self::Output<'w>> {
        let col = table.column(col.unwrap().get());
        let item = unsafe {
            col.get_ptr::<T>(row.0)?
                .as_ptr()
                .cast_const()
                .as_ref_unchecked()
        };

        Some(item)
    }
}

/// Requests mutable access to a component of type `T`.
///
/// Queries will return the [`Mut`] type, which automatically dereferences to `T`,
/// instead of returning the reference directly. This is used
/// by the change tracking system to trigger change events. Instead of `&mut T`, `Mut<T>` can also be used
/// in the queries. They are both equivalent.
///
/// # Access
///
/// Components also follow Rust's aliasing model. Using a mutable component reference will force the scheduler
/// to give this system exclusive access to `T` for the duration of the system.
unsafe impl<T: Component> QueryData for &mut T {
    type Deref = T;
    type Output<'w> = Mut<'w, T>;
    // base pointer for column + change detection
    type BasePtr = SendWrapper<(NonNull<T>, NonNull<u32>)>;

    const TY: QueryType = QueryType::Component;

    #[inline]
    fn access(reg: &mut TypeRegistry) -> AccessDesc {
        AccessDesc {
            ty: AccessType::Component(reg.get_or_assign::<T>()),
            mutable: true,
        }
    }

    #[inline]
    fn get_base_ptr(table: &Table) -> Self::BasePtr {
        let col = table
            .get_column_by_type(&TypeId::of::<T>())
            .expect("expected column not found in table");

        SendWrapper((col.base_ptr(), col.changed_base_ptr_mut()))
    }

    #[inline]
    fn dangling() -> Self::BasePtr {
        SendWrapper((NonNull::dangling(), NonNull::dangling()))
    }

    #[inline]
    unsafe fn fetch_from_base<'w>(base: &mut Self::BasePtr, current_tick: u32) -> Self::Output<'w> {
        let inner = unsafe { &mut *base.0.0.as_ptr() };
        let tracker = unsafe { &mut *base.1.as_ptr() };

        *base = SendWrapper((unsafe { base.0.0.add(1) }, unsafe { base.0.1.add(1) }));

        Mut {
            current_tick,
            tracker,
            inner,
        }
    }

    fn get<'w, Q: QueryGroup, F: Filter>(
        _world: &'w World,
        state: &'w QueryState<Q, F>,
        table: &'w Table,
        row: ColumnRow,
        col: Option<NonMaxUsize>,
    ) -> Option<Self::Output<'w>> {
        let col = table.column(col.unwrap().get());

        // Safety: This query has unique access to this column.
        let item = unsafe { col.get_ptr::<T>(row.0)?.as_ptr().as_mut_unchecked() };

        // Safety: This query has unique access to this column.
        let tracker = unsafe { col.changed_base_ptr().add(row.0) };

        Some(Mut {
            inner: item,
            current_tick: state.current_tick,
            tracker: unsafe { &mut *tracker.as_ptr().cast_mut() },
        })
    }
}

macro_rules! impl_bundle {
    ($count:literal, $($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens)]
            #[diagnostic::do_not_recommend]
            unsafe impl<$($gen: QueryData),*> QueryGroup for ($($gen),*) {
                type AccessCount = generic_array::typenum::[< U $count >];
                type Output<'t> = ($($gen::Output<'t>),*) where
                    Self: 't,
                    ($($gen),*): 't;

                type BasePtrs = ($($gen::BasePtr),*);

                const LEN: usize = (&[$(stringify!($gen)),*] as &[&str]).len();

                fn signature(reg: &mut TypeRegistry) -> Signature {
                    let mut sig = Signature::new();

                    $(
                        if const { $gen::TY.is_component() } {
                            let id = reg.get_or_assign::<$gen::Deref>();
                            sig.set(*id);
                        }
                    )*

                    sig
                }

                fn get<'t, T: Filter>(world: &'t World, state: &'t QueryState<Self, T>, table: &'t Table, row: ColumnRow) -> Option<Self::Output<'t>> where Self: 't {
                    todo!("QueryGroup::get");
                }

                unsafe fn fetch_from_base<'w>(ptrs: &mut ($($gen::BasePtr),*), current_tick: u32) -> Self::Output<'w> where Self: 'w {
                    #[allow(non_snake_case)]
                    let ($($gen),*) = ptrs;
                    ($(
                        unsafe { $gen::fetch_from_base($gen, current_tick) }
                    ),*)
                }

                #[cfg_attr(
                    feature = "tracing",
                    tracing::instrument(name = "QueryGroup::access", fields(size = $count), skip_all)
                )]
                #[inline]
                fn access(reg: &mut TypeRegistry) -> GenericArray<AccessDesc, Self::AccessCount> {
                    GenericArray::from(
                        ($(
                            $gen::access(reg),
                        )*)
                    )
                }

                #[inline]
                fn dangling() -> Self::BasePtrs {
                    (
                        $(
                            $gen::dangling()
                        ),*
                    )
                }

                #[cfg_attr(
                    feature = "tracing",
                    tracing::instrument(name = "QueryGroup::cache_columns", fields(size = $count), skip_all)
                )]
                #[inline]
                fn get_base_ptrs(table: &Table) -> Self::BasePtrs {
                    (
                        ($(
                            $gen::get_base_ptr(table)
                        ),*)
                    )
                }
            }
        }
    }
}

impl_bundle!(1, A);
impl_bundle!(2, A, B);
impl_bundle!(3, A, B, C);
impl_bundle!(4, A, B, C, D);
impl_bundle!(5, A, B, C, D, E);
impl_bundle!(6, A, B, C, D, E, F);
impl_bundle!(7, A, B, C, D, E, F, G);
impl_bundle!(8, A, B, C, D, E, F, G, H);
impl_bundle!(9, A, B, C, D, E, F, G, H, I);
impl_bundle!(10, A, B, C, D, E, F, G, H, I, J);
