use std::any::TypeId;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;

use crate::util::ConstNonNull;
#[cfg(debug_assertions)]
use crate::util::debug::BorrowEnforcer;

use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::archetype::Signature;
use crate::component::TypeRegistry;
use crate::table::{Column, ColumnRow, Table};

/// A component ID.
///
/// This is a unique ID that is assigned to every component type.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct ComponentId(pub(crate) usize);

impl fmt::Display for ComponentId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for ComponentId {
    type Target = usize;

    fn deref(&self) -> &usize {
        &self.0
    }
}

impl From<usize> for ComponentId {
    fn from(v: usize) -> Self {
        Self(v)
    }
}

pub enum StorageType {
    Table,
    SparseSet,
}

/// A marker trait indicating that the implementor can be used as a component.
pub trait Component: 'static + Send {
    // TODO: Add a tracking type so change tracking can be enabled or disabled for specific components.
    // TODO: Add a storage type constant so each component can decide whether it should be stored in a sparse set
    // or archetype table.
    const STORAGE: StorageType = StorageType::Table;
}

/// Implements the functionality to compare ticks in the [`Added`] and [`Changed`] filters.
///
/// [`Added`]: crate::query::Added
/// [`Changed`]: crate::query::Changed
pub trait TrackerFilterImpl {
    fn filter<T: Component>(&self, last_run_tick: u32) -> bool;
}

/// A collection of components used in a filter. This trait makes it possible to use tuples
/// of components inside of filters rather than just a single component.
///
/// It enables filters such as `With<(Health, Transform)>`.
pub trait ComponentBundle: 'static + Send {
    /// Requires `Send` to allow being sent to other threads in parallel iterators.
    type TrackerPtrs: TrackerFilterImpl + Send;

    const LEN: usize;

    /// Whether this bundle contains the given type ID.
    fn contains(ty_id: TypeId) -> bool;

    /// Converts this bundle to a signature to compare against archetype tables. If a component had not been
    /// registered, this method will register it.
    ///
    /// If you do not have mutable access to the component registry, try [`try_get_signature`].
    ///
    /// In the signature, the indices corresponding the components of this bundle are set to 1.
    ///
    /// [`try_get_signature`]: Self::try_get_signature
    fn get_or_assign_signature(reg: &mut TypeRegistry) -> Signature;

    /// Converts this bundle to a signature to compare against archetype tables. If a component had not been
    /// this function will return `None`.
    ///
    /// The advantage of this method is that you do not need mutable access to the component registry.
    ///
    /// In the signature, the indices corresponding the components of this bundle are set to 1.
    fn try_get_signature(reg: &TypeRegistry) -> Option<Signature>;

    /// Creates a new table for this bundle of components. This table can be inserted into the archetypes container.
    ///
    /// # Safety
    ///
    /// The given `signature` must be the exact signature of `Self`. This signature should be obtained using
    /// [`try_get_signature`] or [`get_or_assign_signature`].
    ///
    /// [`try_get_signature`]: ComponentBundle::try_get_signature
    /// [`get_or_assign_signature`]: ComponentBundle::get_or_assign_signature
    unsafe fn new_table(signature: Signature) -> Table;

    /// Creates a new table by joining these components to an existing table.
    ///
    /// `base` is the table to add these columns to and `sig` is the signature of `Self`.
    ///
    /// # Safety
    ///
    /// The given `self_signature` must be the exact signature of `Self` _combined_ with the signature
    /// from the `base` table.
    unsafe fn new_joined_table(base: &Table, self_signature: Signature) -> Table;

    /// Insert this bundle into an existing table.
    fn insert_into(self, table: &mut Table, current_tick: u32);

    /// Removes the components listed in `Self` from the given table.
    ///
    /// This function does not drop the components.
    unsafe fn copy_from(table: &mut Table, row: ColumnRow) -> Self;

    fn get_added_tracker_ptrs(table: &Table) -> Self::TrackerPtrs;

    fn get_changed_tracker_ptrs(table: &Table) -> Self::TrackerPtrs;

    fn dangling_tracker_ptrs() -> Self::TrackerPtrs;
}

macro_rules! replace_with {
    ($x:tt, $to:tt) => {
        $to
    };
}

/// Implements [`ComponentBundle`] for tuples of varying arities.
macro_rules! impl_component_bundle {
    ($count:literal, $($gen:ident),*) => {
        paste::paste! {
            pub struct [< ComponentTracker $count >]<$($gen:Component),*>($(ConstNonNull<replace_with!($gen, u32)>),*, PhantomData<($($gen),*)>);

            impl<$($gen:Component),*> TrackerFilterImpl for [< ComponentTracker $count >]<$($gen),*> {
                #[inline]
                fn filter<T: Component>(&self, last_run_tick: u32) -> bool {
                    let [< ComponentTracker $count >]($($gen),*, ..) = self;

                    // Only try filtering if the given `T` is actually in this filter.
                    $(
                        if TypeId::of::<T>() == TypeId::of::<$gen>() {
                            println!("{} vs. {}", unsafe { *$gen.as_ptr() }, last_run_tick);
                            return unsafe { *$gen.as_ptr() } > last_run_tick
                        }
                    )*

                    true
                }
            }

            #[allow(unused)]
            impl<$($gen:Component),*> ComponentBundle for ($($gen),*) {
                // Using the `map_u32` macro to use `$gen` inside the repetition.
                type TrackerPtrs = [< ComponentTracker $count >]<$($gen),*>;

                const LEN: usize = (&[$( stringify!($gen) ),*] as &[&str]).len();

                #[inline]
                fn contains(ty_id: TypeId) -> bool {
                    $(TypeId::of::<$gen>() == ty_id)||*
                }

                fn get_or_assign_signature(reg: &mut TypeRegistry) -> Signature {
                    let mut set = Signature::new();
                    $(
                        let id = reg.get_or_assign::<$gen>();
                        set.set(*id);
                    )*
                    set
                }

                fn try_get_signature(reg: &TypeRegistry) -> Option<Signature> {
                    let mut set = Signature::new();
                    $(
                        let id = reg.get::<$gen>()?;
                        set.set(*id);
                    )*
                    Some(set)
                }

                #[allow(unused)]
                unsafe fn new_table(signature: Signature) -> Table {
                    let mut lookup = FxHashMap::with_capacity_and_hasher(Self::LEN, FxBuildHasher::default());
                    let mut counter = 0;
                    $(
                        lookup.insert(TypeId::of::<$gen>(), counter);
                        counter += 1;
                    )*

                    Table {
                        signature,
                        entities: Vec::new(),
                        entity_lookup: FxHashMap::default(),
                        lookup,
                        columns: vec![
                            $(
                                Column::new::<$gen>()
                            ),*
                        ],

                        #[cfg(debug_assertions)]
                        enforcer: BorrowEnforcer::new()
                    }
                }

                unsafe fn new_joined_table(base: &Table, mut signature: Signature) -> Table {
                    // Check whether the original table and this bundle are disjoint.
                    if !base.signature.is_disjoint(&signature) {
                        todo!("table and bundle are not disjoint");
                    }

                    // Combine the signature of the table with `Self`'s signature.
                    signature.union(&base.signature);

                    let old_col_count = base.columns.len();
                    let new_col_count = old_col_count + Self::LEN;

                    let mut columns = Vec::with_capacity(new_col_count);

                    // Copy over base column metadata
                    columns.extend(base.columns.iter().map(|c| c.clone_empty()));

                    // Add new columns
                    $(
                        columns.push(Column::new::<$gen>());
                    )*

                    let mut lookup = base.lookup.clone();
                    let mut counter = lookup.len();
                    $(
                        lookup.insert(TypeId::of::<$gen>(), counter);
                        counter += 1;
                    )*

                    Table {
                        signature,
                        entities: Vec::new(),
                        entity_lookup: FxHashMap::default(),
                        lookup,
                        columns,

                        #[cfg(debug_assertions)]
                        enforcer: BorrowEnforcer::new()
                    }
                }

                #[cfg_attr(
                    feature = "tracing",
                    tracing::instrument(name = "ComponentBundle::insert_into", fields(bundle = std::any::type_name::<Self>()), skip_all)
                )]
                #[allow(unused)]
                fn insert_into(self, storage: &mut Table, current_tick: u32) {
                    let ($([<$gen:lower>]),*) = self;
                    $(
                        let column_idx = *storage.lookup.get(&TypeId::of::<$gen>()).expect("attempt to insert data into wrong archetype table");
                        storage.columns[column_idx].push([<$gen:lower>], current_tick);
                    )*
                }

                unsafe fn copy_from(table: &mut Table, row: ColumnRow) -> Self {
                    ($(
                        {
                            let col = table.get_column_by_type(&TypeId::of::<$gen>()).expect("table did not have requested component");
                            let ptr = col.get_erased_ptr(row.0).expect("requested row was not found in column");

                            unsafe { std::ptr::read(ptr.cast::<$gen>().as_ptr()) }
                        }
                    ),*)
                }

                fn get_added_tracker_ptrs(table: &Table) -> Self::TrackerPtrs {
                    [< ComponentTracker $count >]($(
                        {
                            let col = table.get_column_by_type(&TypeId::of::<$gen>()).expect("table did not have required column");
                            col.added_base_ptr()
                        }
                    ),*, PhantomData)
                }

                fn get_changed_tracker_ptrs(table: &Table) -> Self::TrackerPtrs {
                    [< ComponentTracker $count >]($(
                        {
                            let col = table.get_column_by_type(&TypeId::of::<$gen>()).expect("table did not have required column");
                            col.changed_base_ptr()
                        }
                    ),*, PhantomData)
                }

                fn dangling_tracker_ptrs() -> Self::TrackerPtrs {
                    [< ComponentTracker $count >]($(
                        replace_with!($gen, { ConstNonNull::dangling() })
                    ),*, PhantomData)
                }
            }
        }
    }
}

impl_component_bundle!(1, A);
impl_component_bundle!(2, A, B);
impl_component_bundle!(3, A, B, C);
impl_component_bundle!(4, A, B, C, D);
impl_component_bundle!(5, A, B, C, D, E);
impl_component_bundle!(6, A, B, C, D, E, F);
impl_component_bundle!(7, A, B, C, D, E, F, G);
impl_component_bundle!(8, A, B, C, D, E, F, G, H);
impl_component_bundle!(9, A, B, C, D, E, F, G, H, I);
impl_component_bundle!(10, A, B, C, D, E, F, G, H, I, J);
