use std::alloc::Layout;
use std::any::TypeId;
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::query::Filter;
use crate::table::{ChangeTracker, ChangeTrackerIter, ColumnIter, ColumnIterMut};
use crate::util::LayoutExt;

#[cfg(debug_assertions)]
use crate::util::debug::BorrowEnforcer;

/// A function pointer to a function that can drop an array of elements.
type DropFn = unsafe fn(ptr: *mut u8, len: usize);

/// Drops `len` items of type `T` contained in `ptr`.
///
/// This function is used to invoke the `Drop` implementation on the items in a `Column`.
///
/// # Safety
///
/// This function must only be called when the following conditions are met:
/// - `ptr` is a valid pointer to an array of objects of type `T`.
/// - `len` is less than or equal to the amount of objects contained in the array starting at `ptr`.
unsafe fn drop_wrapper<T>(ptr: *mut u8, len: usize) {
    let ptr = ptr.cast::<T>();
    for i in 0..len {
        unsafe {
            std::ptr::drop_in_place(ptr.add(i));
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableRow(pub(crate) usize);

/// Stores a collection of a single component type.
#[derive(Debug)]
pub struct Column {
    #[cfg(debug_assertions)]
    enforcer: BorrowEnforcer,
    /// Tracks which components have changed in this Column.
    tracker: UnsafeCell<ChangeTracker>,
    /// Which components in this
    /// The type ID of the item contained in this Column.
    ty: TypeId,
    /// The layout of the component type.
    layout: Layout,
    /// The amount of items contained in this Column.
    pub(crate) len: usize,
    /// The capacity of this Column.
    cap: usize,
    /// An optional pointer to the buffer that holds the Column data.
    ///
    /// This field is `None` if and only if `cap` is 0.
    data: Option<NonNull<u8>>,
    /// The function that is called when an item is dropped.
    ///
    /// This field is `None` if the item does not require dropping, i.e.
    /// `std::mem::needs_drop<T>` returned false.
    drop_fn: Option<DropFn>,
}

impl Column {
    /// Creates a new Column for the type `T`.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "Column::new", skip_all)
    )]
    pub fn new<T: 'static>() -> Column {
        // The static requirement on `T` ensures that the type does not contain any references.

        let drop_fn = if std::mem::needs_drop::<T>() {
            Some(drop_wrapper::<T> as DropFn)
        } else {
            None
        };

        let layout = Layout::new::<T>();
        if std::mem::size_of::<T>() == 0 {
            tracing::trace!(
                "created new column for ZST `{}`, needs drop: {}",
                std::any::type_name::<T>(),
                std::mem::needs_drop::<T>()
            );

            // Produce a valid non-null pointer for the ZST even though we will never use it.
            let ptr = NonNull::<T>::dangling().cast::<u8>();

            Column {
                #[cfg(debug_assertions)]
                enforcer: BorrowEnforcer::new(),

                tracker: UnsafeCell::new(ChangeTracker::new()),
                ty: TypeId::of::<T>(),
                layout,
                len: 0,
                // Set capacity to max to disable allocations.
                cap: usize::MAX,
                data: Some(ptr),
                drop_fn,
            }
        } else {
            tracing::trace!(
                "created new column for `{}`, needs drop: {}",
                std::any::type_name::<T>(),
                std::mem::needs_drop::<T>()
            );

            Column {
                #[cfg(debug_assertions)]
                enforcer: BorrowEnforcer::new(),

                tracker: UnsafeCell::new(ChangeTracker::new()),
                ty: TypeId::of::<T>(),
                layout,
                len: 0,
                cap: 0,
                data: None,
                drop_fn,
            }
        }
    }

    #[inline]
    pub fn tracker_ptr(&self) -> NonNull<ChangeTracker> {
        // Safety: References are never null.
        unsafe { NonNull::new_unchecked(self.tracker.get()) }
    }

    /// Returns the size of an entry in bytes. This includes potential padding.
    ///
    /// In other words, this is equivalent to `std::mem::size_of::<T>()` where
    /// `T` is the type contained in this `Column`.
    pub fn padded_size(&self) -> usize {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        self.layout.pad_to_align().size()
    }

    /// Creates an iterator over this column. Optional filters can be applied using the `F` generic.
    ///
    /// # Panics
    ///
    /// This function panics if the given generic `T` is not the same as the `T` that was used in the call
    /// to `Column::new`.
    pub fn iter<T: 'static, F: Filter>(&self, current_tick: u32) -> ColumnIter<'_, T, F> {
        #[cfg(debug_assertions)]
        let guard = self.enforcer.read();

        assert_eq!(
            TypeId::of::<T>(),
            self.ty,
            "attempt to create column iter with wrong type"
        );

        if let Some(start_ptr) = self.data {
            ColumnIter {
                current_tick,
                tracker: ChangeTrackerIter::new(unsafe { &*self.tracker.get() }),
                curr: Some(start_ptr.cast::<T>()),
                remaining: self.len,
                _marker: PhantomData,

                #[cfg(debug_assertions)]
                _guard: Some(guard),
            }
        } else {
            ColumnIter {
                current_tick,
                tracker: ChangeTrackerIter::empty(),
                curr: None,
                remaining: 0,
                _marker: PhantomData,

                #[cfg(debug_assertions)]
                _guard: Some(guard),
            }
        }
    }

    /// Creates a mutable iterator over this column. Optional filters can be applied using the `F` generic.
    ///
    /// # Panics
    ///
    /// This function panics if the given generic `T` is not the same as the `T` that was used in the call
    /// to `Column::new`.
    pub fn iter_mut<T: 'static, F: Filter>(
        &self,
        last_tick: u32,
        current_tick: u32,
    ) -> ColumnIterMut<'_, T, F> {
        #[cfg(debug_assertions)]
        let guard = self.enforcer.write();

        assert_eq!(
            TypeId::of::<T>(),
            self.ty,
            "attempt to create column iter with wrong type"
        );

        if let Some(start_ptr) = self.data {
            ColumnIterMut {
                index: 0,
                changes: ChangeTrackerIter::new(unsafe { &*self.tracker.get() }),
                last_tick,
                current_tick,

                curr: Some(start_ptr.cast::<T>()),
                remaining: self.len,
                _marker: PhantomData,

                #[cfg(debug_assertions)]
                _guard: Some(guard),
            }
        } else {
            ColumnIterMut {
                index: 0,
                changes: ChangeTrackerIter::new(unsafe { &*self.tracker.get() }),
                last_tick,
                current_tick,

                curr: None,
                remaining: 0,
                _marker: PhantomData,

                #[cfg(debug_assertions)]
                _guard: Some(guard),
            }
        }
    }

    /// Reserves capacity for at least `n` additional entries.
    #[expect(
        clippy::missing_panics_doc,
        reason = "exists for soundness reasons but realistically should never be triggered"
    )]
    pub fn reserve(&mut self, n: usize) {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        if n == 0 || self.layout.size() == 0 {
            // Do nothing for ZSTs and
            // don't bother allocating for 0 size. This also ensures that we do not try to allocate
            // an empty array of zero size.
            return;
        }

        self.tracker.get_mut().reserve(n);

        let cap = self.cap + n;

        let (old_layout, _) = self
            .layout
            .repeat_ext(self.cap)
            .expect("invalid array layout");
        let (new_layout, _) = self.layout.repeat_ext(cap).expect("invalid array layout");

        assert!(
            isize::try_from(new_layout.size()).is_ok(),
            "allocation too large"
        );

        let ptr = if let Some(ptr) = self.data {
            // Safety:
            //
            // This is safe because layout has a non-zero size, which is upheld by the assertion.
            // Additionally, the given layout is the same as the one used in the original allocation since it is
            // stored in the Column unchanged. Furthermore, the pointer used to reallocate is the one
            // that was originally allocated using `alloc` and the new size is less than or equal to `isize::MAX`.
            unsafe { std::alloc::realloc(ptr.as_ptr(), old_layout, new_layout.size()) }
        } else {
            // Safety:
            //
            // This is safe because layout has a non-zero size, which is upheld by the assertion and
            // by the check for `n == 0`.
            unsafe { std::alloc::alloc(new_layout) }
        };

        // If this line panics, the `Drop` impl will be called with the unchanged pointer, hence
        // deallocating the data.
        self.data = Some(NonNull::new(ptr).expect("Column::reserve allocation failed"));
        self.cap = cap;
    }

    /// Pushes a new entry into the column.
    ///
    /// # Panics
    ///
    /// This function panics if the given generic `T` is not the same as the `T` that was used in the call
    /// to `Column::new`. This `T` is not stored in the `Column` type to prevent the runtime cost of dynamic dispatch.
    pub fn push<T: 'static>(&mut self, data: T, current_tick: u32) {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        assert_eq!(
            TypeId::of::<T>(),
            self.ty,
            "Column::push called with mismatched types"
        );

        if self.cap <= self.len {
            // Reserve at least 4 slots to reduce allocations at the start.
            let new_slots = self.cap.clamp(4, usize::MAX);
            self.reserve(new_slots);
        }

        let offset = self.layout.pad_to_align().size() * self.len;
        assert!(
            isize::try_from(offset).is_ok(),
            "pointer offset overflow in Column::push"
        );

        // Safety:
        //
        // The computed offset does not overflow `isize` by the assert above and the original pointer
        // `self.data` is derived from an allocation while the offset result is within the allocation due
        // to the check that `self.len < self.cap`.
        let column_ptr = unsafe { self.data.unwrap().add(offset) };

        // Safety:
        //
        // This is safe since the pointer is guaranteed to be valid by the length check above.
        // Additionally `std::ptr::write` semantically moves `data` into the Column, ensuring it does
        // not get dropped. This ensures there is no use after free.
        unsafe {
            std::ptr::write(column_ptr.cast::<T>().as_ptr(), data);
        }

        self.len += 1;
        self.tracker.get_mut().resize(self.len, current_tick);
    }

    /// Obtains a pointer to the given entry in the Column.
    ///
    /// This function returns `None` if the index did not exist.
    ///
    /// This function is not marked unsafe because obtaining the pointer itself is a safe operation.
    /// The reference aliasing rules must be upheld manually when dereferencing this pointer however.
    ///
    /// In other words, if there exists a mutablereference to this `Column`, you cannot dereference the returned
    /// pointer. If there exists a shared reference to this `Column`, you must cast the pointer to a `*const T` and
    /// only use it as a shared pointer. If there exist no references, you can do either.
    ///
    /// # Panics
    ///
    /// This function panics if `T` is not the type that is contained in this table.
    pub fn get_ptr<T: 'static>(&self, index: usize) -> Option<NonNull<T>> {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        assert_eq!(
            TypeId::of::<T>(),
            self.ty,
            "Column::get called with mismatched types"
        );

        if index >= self.len {
            return None;
        }

        let offset = self.layout.size() * index;
        assert!(
            isize::try_from(offset).is_ok(),
            "pointer offset overflow in Column::get"
        );

        // Safety:
        //
        // This call to `NonNull::add` is safe because the offset does not overflow `isize` by the assertion
        // above. Additionally, the pointer `self.data` is a valid allocation. Lastly, due to the check
        // above we know that `index < self.len` and the offset result is within this allocation.
        //
        // By the assertion we also know that the pointer is pointing to some type `T`.
        Some(unsafe { self.data.unwrap().add(offset).cast::<T>() })
    }

    /// Removes the item at index and moves the last item in the Column to the, now empty, slot.
    ///
    /// # Panics
    ///
    /// This function panics if the index is out of bounds.
    pub fn swap_remove(&mut self, idx: usize) {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        assert!(idx < self.len, "Column::swap_remove index out of bounds");

        let data_ptr = self.data.unwrap().as_ptr();
        let dst_offset = self.padded_size() * idx;
        assert!(
            isize::try_from(dst_offset).is_ok(),
            "pointer offset overflow in Column::swap_remove dst pointer"
        );

        // The item to remove and copy into
        //
        // Safety:
        //
        // The offset is guaranteed not to overflow `isize` by the assertion above.
        // Furthermore, `data_ptr` is a valid pointer into an allocation and by the
        // `idx >= self.len` check above, the offset result is also within the allocation.
        let dst_ptr = unsafe { data_ptr.add(dst_offset) };

        // Drop the item if necessary
        self.drop_fn.inspect(|f| {
            // Safety:
            //
            // This call is safe because `dst_ptr` is guaranteed to point to a valid memory block of type `T`,
            // where `T` is the type used in `Column::new`. Additionally it contains at least 1 item, thus the
            // size is correct.
            //
            // Lastly this is a valid function pointer since it is only set in `Column::new`.
            unsafe {
                f(dst_ptr, 1);
            }
        });

        if idx != self.len - 1 {
            let src_offset = self.padded_size() * (self.len() - 1);
            assert!(
                isize::try_from(src_offset).is_ok(),
                "pointer offset overflow in Column::swap_remove src pointer"
            );

            // The last item in the array. Will be copied to the empty slot
            //
            // Safety:
            //
            // The offset is guaranteed not to overflow `isize` by the assertion above.
            // Furthermore, `data_ptr` is a valid pointer into an allocation and by the
            // fact the count is `self.len() - 1`, the offset result is also within the allocation.
            let src_ptr = unsafe { data_ptr.add(src_offset) };

            // Then copy the last item into the now empty slot.
            //
            // Safety:
            //
            // This is safe because `src_ptr` is a valid pointer as described above,
            // and `dst_ptr` is also safe as described above. The size is computed from the layout, which is the exact
            // layout created from `T`.
            unsafe {
                std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, self.layout.size());
            }
        }

        self.len -= 1;
    }

    /// The amount of elements currently contained in the column.
    pub fn len(&self) -> usize {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The capacity of the column.
    pub fn capacity(&self) -> usize {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        self.cap
    }
}

impl Drop for Column {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        if let Some(ptr) = self.data {
            // Drop contents if it matters
            if let Some(drop_fn) = self.drop_fn {
                // Safety:
                //
                // This call is safe because `ptr` is guaranteed to point to a valid memory block of type `T`,
                // where `T` is the type used in `Column::new`. Additionally it contains at least 1 item, thus the
                // size is correct.
                //
                // Lastly this is a valid function pointer since it is only set in `Column::new`.
                unsafe {
                    drop_fn(ptr.as_ptr(), self.len);
                }
            }

            if self.layout.size() != 0 {
                let (layout, _) = self
                    .layout
                    .repeat_ext(self.cap)
                    .expect("invalid array layout");
                // Safety:
                //
                // This is safe because `ptr` has previously been allocated with `alloc` and
                // layout was the layout originally used to create this specific allocation.
                // Furthermore, the type is not a ZST.
                unsafe {
                    std::alloc::dealloc(ptr.as_ptr(), layout);
                }
            }
        }
    }
}
