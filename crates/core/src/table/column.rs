use std::alloc::Layout;
use std::any::TypeId;
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::num::NonZero;
use std::ptr::NonNull;

use crate::query::Filter;
use crate::table::ChangeTracker;
use crate::util::{ConstNonNull, LayoutExt};

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
///
/// Each individual element in the array must also satisfy the conditions of [`drop_in_place`].
///
/// [`drop_in_place`]: std::ptr::drop_in_place
unsafe fn drop_wrapper<T>(ptr: *mut u8, len: usize) {
    let ptr = ptr.cast::<T>();
    for i in 0..len {
        // Safety: This is safe by conditions that the caller must uphold.
        unsafe {
            std::ptr::drop_in_place(ptr.add(i));
        }
    }
}

/// A row within a column.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColumnRow(pub(crate) usize);

/// Stores a collection of a single component type.
#[derive(Debug)]
pub struct Column {
    #[cfg(debug_assertions)]
    enforcer: BorrowEnforcer,
    /// Tracks which components have changed in this Column.
    tracker: UnsafeCell<ChangeTracker>,
    /// The type ID of the item contained in this Column.
    pub(crate) ty: TypeId,
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
    #[inline]
    pub fn ty_id(&self) -> TypeId {
        self.ty
    }

    /// Copies the specific component from `self` to `other` without dropping the old component.
    ///
    /// # Panics
    ///
    /// Panics if `row` is not contained in the given column.
    pub unsafe fn copy_component(&self, other: &mut Column, row: usize, current_tick: u32) {
        debug_assert_eq!(self.ty, other.ty);
        debug_assert_eq!(self.layout, other.layout);

        let src_ptr = self.get_erased_ptr(row).unwrap();
        unsafe { other.push_from_ptr(src_ptr, current_tick) };
    }

    /// Copies the element from the given `ptr` into the current column.
    ///
    /// # Safety
    ///
    /// - `ptr` must point to a single valid component of the type that this column
    ///   contains.
    ///
    /// - This function takes ownership of data pointed to by `ptr`, therefore the data should not
    ///   be used anymore.
    ///
    /// - `ptr` must not overlap with the first slot in the unused capacity of this column.
    #[expect(
        clippy::missing_panics_doc,
        reason = "this should realistically never happen and only exists for safety reasons"
    )]
    pub unsafe fn push_from_ptr(&mut self, ptr: NonNull<u8>, current_tick: u32) {
        if self.layout.size() == 0 {
            // ZSTs do not need to be copied.
            self.len += 1;
            return;
        }

        if self.cap <= self.len {
            let new_slots = self.cap.clamp(4, usize::MAX);
            self.reserve(new_slots);
        }

        let offset = self.layout.pad_to_align().size() * self.len;
        assert!(
            isize::try_from(offset).is_ok(),
            "pointer offset overflow in Column::push"
        );

        // Safety: This is safe since the offset does not overflow `isize` by the assert above
        // and the resulting pointer is within the current allocation by the capacity check above.
        let col_ptr = unsafe { self.data.unwrap().add(offset) };

        let size = self.layout.size();

        // Safety: `ptr` is properly aligned and does not overlap with `col_ptr` as required in the safety
        // conditions of this function. `col_ptr` is also a valid and aligned pointer by the code above.
        unsafe {
            std::ptr::copy_nonoverlapping(ptr.as_ptr(), col_ptr.as_ptr(), size);
        }

        self.len += 1;

        // Add space for this new row in the change tracker.
        self.tracker.get_mut().resize(self.len, current_tick);
    }

    /// Create an empty copy of self.
    #[must_use]
    pub fn clone_empty(&self) -> Self {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        Self {
            #[cfg(debug_assertions)]
            enforcer: BorrowEnforcer::new(),

            tracker: UnsafeCell::new(ChangeTracker::new()),

            layout: self.layout,
            ty: self.ty,
            len: 0,
            cap: if self.layout.size() == 0 {
                usize::MAX
            } else {
                0
            },
            data: if self.layout.size() == 0 {
                self.data
            } else {
                None
            },
            drop_fn: self.drop_fn,
        }
    }

    /// Creates a new column for the type `T`. No memory is allocated until the first push.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "Column::new", skip_all)
    )]
    pub fn new<T: 'static>() -> Column {
        // The static requirement on `T` ensures that the type does not contain any references,
        // which could allow the column to outlive the component.

        let drop_fn = if std::mem::needs_drop::<T>() {
            Some(drop_wrapper::<T> as DropFn)
        } else {
            None
        };

        let layout = Layout::new::<T>().align_to(64).unwrap();

        if std::mem::size_of::<T>() == 0 {
            // The allocator does not support "allocating" zero-sized types so we
            // handle them separately here by creating a dangling pointer and just increasing
            // the length of the column any time a ZST is pushed.

            tracing::trace!(
                "created new column for ZST `{}`, needs drop: {}",
                std::any::type_name::<T>(),
                std::mem::needs_drop::<T>()
            );

            // Produce a valid non-null, aligned pointer for the ZST.
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
    pub fn added_base_ptr(&self) -> ConstNonNull<u32> {
        todo!()
        // self.tracker.
    }

    #[inline]
    pub fn change_base_ptr(&self) -> ConstNonNull<u32> {
        todo!()
    }

    #[inline]
    pub fn base_ptr<T: 'static>(&self) -> NonNull<T> {
        assert_eq!(
            TypeId::of::<T>(),
            self.ty,
            "attempt to obtain base pointer with incorrect type"
        );

        self.data.unwrap().cast::<T>()
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

        if self.layout.size() == 0 {
            self.len += 1;
            return;
        }

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

    /// Obtains a pointer to a row in the column, without being aware of the underlying component type.
    ///
    /// This is used to copy components to another column, since columns are only aware of the size of
    /// the data.
    #[expect(
        clippy::missing_panics_doc,
        reason = "this should realistically never panic"
    )]
    pub fn get_erased_ptr(&self, index: usize) -> Option<NonNull<u8>> {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.read();

        if index >= self.len {
            return None;
        }

        if self.layout.size() == 0 {
            // Return a simple dangling pointer.

            // Safety: Alignment is always nonzero, even for ZSTs.
            let align = unsafe { NonZero::new_unchecked(self.layout.align()) };
            return Some(NonNull::without_provenance(align));
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
        Some(unsafe { self.data.unwrap().add(offset) })
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

        if self.layout.size() == 0 {
            return Some(NonNull::dangling());
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
    /// The component is dropped if it needs to be.
    ///
    /// # Panics
    ///
    /// This function panics if the index is out of bounds.
    pub fn swap_remove(&mut self, idx: usize, should_drop: bool) {
        #[cfg(debug_assertions)]
        let _guard = self.enforcer.write();

        assert!(idx < self.len, "Column::swap_remove index out of bounds");

        let data_ptr = self.data.unwrap().as_ptr();
        let dst_offset = self.layout.pad_to_align().size() * idx;
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
        if should_drop && let Some(drop_fn) = self.drop_fn {
            // Safety:
            //
            // This call is safe because `dst_ptr` is guaranteed to point to a valid memory block of type `T`,
            // where `T` is the type used in `Column::new`. Additionally it contains at least 1 item, thus the
            // size is correct.
            //
            // Lastly this is a valid function pointer since it is only set in `Column::new`.
            unsafe {
                drop_fn(dst_ptr, 1);
            }
        }

        if idx != self.len - 1 {
            let src_offset = self.layout.pad_to_align().size() * (self.len - 1);
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
