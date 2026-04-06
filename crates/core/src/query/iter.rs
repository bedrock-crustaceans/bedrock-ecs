//! Implements the query iterator functionality.
//!
use std::any::TypeId;
use std::iter::FusedIterator;
use std::marker::PhantomData;
use std::ptr::NonNull;

use nonmax::NonMaxUsize;

use generic_array::{ArrayLength, GenericArray};
use rustc_hash::FxHashMap;

use crate::archetype::Signature;
use crate::component::TypeRegistry;
use crate::query::{Filter, QueryBundle, QueryData, QueryState, QueryType, TableCache};
use crate::scheduler::AccessDesc;
use crate::table::{ColumnRow, Table};
use crate::world::World;

/// Queries do not care what a type is or contains, as long as it speaks the language of indices and filters.
/// This trait encapsulates that logic, allowing any type implementing this trait to be queried.
///
/// This is a separate trait from [`Index`] to allow for access without bounds checking.
/// Bounds checking is already performed by the query itself and should not be done by this trait.
///
/// It also moves the output type to an associated type rather than using a generic, since `Index<T>` would force
/// every array-like object to return `T`.
///
/// # Safety
///
/// - `Self` must have exactly as many items as the query thinks it has. I.e. if
///   there are five entities and we query `Query<(Entity, Has<Component>)>`, then
///   the underlying array for `Has<Component>` must be able to return at least 5 items.
///
/// - [`len`] must return the exact size of the type. The type must support indexing via [`get_unchecked`]
///   up to this length.
///
/// Queries perform their own bounds checking and then access all of the arrays without
/// performing additional checks. This trait must be implemented correctly to prevent out of
/// bounds access.
///
/// [`Index`]: std::ops::Index
/// [`len`]: crate::query::ArrayLike::len
/// [`get_unchecked`]: crate::query::ArrayLike::get_unchecked
pub unsafe trait ArrayLike {
    /// The item that this "array" contains.
    type Item;

    /// Returns the item at the given index.
    ///
    /// # Safety
    ///
    /// `index` should be within bounds for this array.
    unsafe fn get_unchecked(&mut self, index: usize) -> Self::Item;

    unsafe fn filter_unchecked(&self, index: usize) -> bool;

    fn empty() -> Self;

    /// The length of this array.
    fn len(&self) -> usize;
}

/// An iterator that can jump from table to table.
///
/// This is useful when querying a component that is contained in multiple archetypes.
///
#[cfg(feature = "generics")]
pub trait JumpingIterator<'t, Q: QueryBundle + 't, F: Filter>:
    Iterator<Item = Q::Output<'t>>
{
    /// Creates a new iterator over the given cache.
    fn from_cache(world: &'t World, meta: &'t QueryState<Q, F>) -> Self;
}

/// An iterator that can jump from table to table.
///
/// These iterators usually contain multiple subiterators that iterate over the columns in each table.
#[cfg(not(feature = "generics"))]
pub trait HoppingIterator<'t>: Sized {
    /// Creates a new iterator over the given cache.
    fn new(world: &'t World, cache: &'t [TableCache]) -> Self;

    // /// Estimates the total amount of components remaining, including remaining tables.
    // /// This estimate does not apply filters and will therefore always overestimate.
    // ///
    // /// Note that this iterator does not implement [`ExactSizeIterator`] due to the fact that
    // /// computing the length isn't a simple operation. The query needs to look through all of the
    // /// future tables and compute their lengths. Therefore, this method has a performance cost.
    // fn estimate_len(&self) -> usize;

    /// Returns the length of the iterator of the *current* table *without* filters.
    ///
    /// A hopping iterator jumps between tables and this function returns the remaining
    /// components in the current table, *not* the total amount of components.
    fn current_len(&self) -> usize;
}

pub struct QueryIter<'w, Q: QueryBundle, F: Filter> {
    len: usize,
    index: usize,
    cache: std::slice::Iter<'w, TableCache<Q>>,
    base_ptrs: Q::BasePtrs,
    filters: F::IterState,
}

impl<'w, Q: QueryBundle, F: Filter> JumpingIterator<'w, Q, F> for QueryIter<'w, Q, F> {
    fn from_cache(world: &'w World, meta: &'w QueryState<Q, F>) -> Self {
        let mut cache = meta.cache.iter();

        // Look up all column base pointers.
        let Some(first_cache) = cache.next() else {
            return Self::empty();
        };

        let table = unsafe { &*first_cache.table.as_ptr() };
        Self {
            len: table.len(),
            index: 0,
            cache,
            base_ptrs: Q::get_base_ptrs(table),
            filters: F::new_iter_state(table),
        }
    }
}

impl<'w, Q: QueryBundle, F: Filter> QueryIter<'w, Q, F> {
    /// Creates an empty iterator that always returns `None`. This exists because
    /// [`std::iter::empty()`] returns a concrete [`Empty`] type that is incompatible with the trait.
    ///
    /// [`Empty`]: std::iter::Empty
    pub fn empty() -> Self {
        Self {
            len: 0,
            index: 0,
            cache: [].iter(),
            base_ptrs: todo!(),
            filters: todo!(),
        }
    }

    /// Jumps to the next table, returning whether the jump was successful
    /// or whether the end of the query has been reached.
    fn jump(&mut self) -> bool {
        if let Some(next_cache) = self.cache.next() {
            let table = unsafe { &*next_cache.table.as_ptr() };
            self.index = 0;
            self.base_ptrs = Q::get_base_ptrs(table);

            true
        } else {
            self.len = 0;
            false
        }
    }

    /// Returns the next entity, while applying the query's dynamic filter.
    #[inline]
    fn next_filtered(&mut self) -> Option<<Self as Iterator>::Item> {
        todo!("next_filtered")
    }
}

#[allow(unused_parens)]
impl<'t, Q: QueryBundle, F: Filter> Iterator for QueryIter<'t, Q, F> {
    type Item = Q::Output<'t>;

    #[allow(non_snake_case, unused)]
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // Check whether iterator is empty
        if self.index >= self.len && !self.jump() {
            return None;
        }

        let item = Q::fetch_from_base(self.base_ptrs, self.index);

        self.index += 1;
        return Some(item);
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let upper_bound = todo!();
        // let upper_bound = self.unfiltered_len();

        if const { F::TRIVIAL } {
            // If this query performs no filtering, we know the exact size.
            (upper_bound, Some(upper_bound))
        } else {
            // Otherwise it has a size ranging from zero to the maximum of the query.
            (0, Some(upper_bound))
        }
    }
}

impl<'t, Q: QueryBundle> ExactSizeIterator for QueryIter<'t, Q, ()> {
    #[inline]
    fn len(&self) -> usize {
        todo!();
        // self.unfiltered_len()
    }
}

impl<'t, Q: QueryBundle, F: Filter> FusedIterator for QueryIter<'t, Q, F> {}
