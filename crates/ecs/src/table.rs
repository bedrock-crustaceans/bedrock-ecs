use std::{alloc::Layout, any::TypeId, cell::UnsafeCell, collections::HashMap, iter::FusedIterator, marker::PhantomData, ptr::NonNull};

#[cfg(debug_assertions)]
use crate::util::debug::RwFlag;
use crate::{archetype::ArchetypeComponents, component::ComponentId, entity::EntityId, spawn::SpawnGroup, util};

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
    let ptr = ptr as *mut T;
    for i in 0..len {
        std::ptr::drop_in_place(ptr.add(i));
    }
}

/// Stores a collection of a single component type.
#[derive(Debug)]
pub struct Column {
    #[cfg(debug_assertions)]
    flag: RwFlag,
    /// The type ID of the item contained in this Column.
    ty: TypeId,
    /// The layout of the component type.
    layout: Layout,
    /// The amount of items contained in this Column.
    len: usize,
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
    drop_fn: Option<DropFn>
}

impl Column {
    /// Creates a new Column for the type `T`.
    pub fn new<T: 'static>() -> Column {
        // The static requirement on `T` ensures that the type does not contain any references.

        let drop_fn = if std::mem::needs_drop::<T>() {
            Some(drop_wrapper::<T> as DropFn)
        } else {
            None
        };

        let layout = Layout::new::<T>();
        if std::mem::size_of::<T>() == 0 {
            // Produce a valid non-null pointer for the ZST even though we will never use it.
            let ptr = NonNull::<T>::dangling().cast::<u8>();

            Column {
                #[cfg(debug_assertions)]
                flag: RwFlag::new(),
                ty: TypeId::of::<T>(),
                layout,
                len: 0,
                // Set capacity to max to disable allocations.
                cap: usize::MAX,
                data: Some(ptr),
                drop_fn
            }
        } else {
            Column {
                #[cfg(debug_assertions)]
                flag: RwFlag::new(),
                ty: TypeId::of::<T>(),
                layout,
                len: 0,
                cap: 0,
                data: None,
                drop_fn
            }
        }
    }

    /// Returns the size of an entry in bytes. This includes potential padding.
    /// 
    /// In other words, this is equivalent to `std::mem::size_of::<T>()` where
    /// `T` is the type contained in this `Column`.
    pub fn entry_size(&self) -> usize {
        self.layout.size()
    }

    /// Reserves capacity for `n` additional entries.
    pub fn reserve(&mut self, n: usize) {
        assert_ne!(self.layout.size(), 0, "Column::reserve should not be called for ZSTs");

        #[cfg(debug_assertions)]
        let _guard = self.flag.write();

        if n == 0 {
            // Don't bother allocating for 0 size. This also ensures that we do not try to allocate
            // an empty array of zero size.
            return
        }

        let cap = self.cap + n;
        let new_layout = util::repeat_layout(self.layout, cap);

        assert!(new_layout.size() <= isize::MAX as usize, "Allocation too large");

        let ptr = if let Some(ptr) = self.data {
            // Safety:
            // 
            // This is safe because layout has a non-zero size, which is upheld by the assertion.
            // Additionally, the given layout is the same as the one used in the original allocation since it is
            // stored in the Column unchanged. Furthermore, the pointer used to reallocate is the one
            // that was originally allocated using `alloc` and the new size is less than or equal to `isize::MAX`.
            unsafe {
                std::alloc::realloc(ptr.as_ptr(), self.layout, new_layout.size())
            }
        } else {
            // Safety:
            // 
            // This is safe because layout has a non-zero size, which is upheld by the assertion and
            // by the check for `n == 0`.
            unsafe {
                std::alloc::alloc(new_layout)
            }
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
    pub fn push<T: 'static>(&mut self, data: T) {
        #[cfg(debug_assertions)]
        let mut _guard = self.flag.write();

        assert_eq!(
            TypeId::of::<T>(),
            self.ty,
            "Column::push called with mismatched types"
        );

        if self.cap <= self.len {
            #[cfg(debug_assertions)]
            drop(_guard);

            // Reserve at least 4 slots to reduce allocations at the start.
            let new_slots = self.cap.clamp(4, usize::MAX);
            self.reserve(new_slots);

            #[cfg(debug_assertions)]
            {
                _guard = self.flag.write();
            }
        }

        let offset = self.layout.size() * self.len;
        assert!(offset <= isize::MAX as usize, "Pointer offset overflow in Column::push");

        // Safety:
        //
        // The computed offset does not overflow `isize` by the assert above and the original pointer
        // `self.data` is derived from an allocation while the offset result is within the allocation due
        // to the check that `self.len < self.cap`.
        let Column_ptr = unsafe {
            self.data.unwrap().add(offset)
        };
        
        // Safety:
        //
        // This is safe since the pointer is guaranteed to be valid by the length check above.
        // Additionally `std::ptr::write` semantically moves `data` into the Column, ensuring it does
        // not get dropped. This ensures there is no use after free.
        unsafe {
            std::ptr::write(Column_ptr.cast::<T>().as_ptr(), data);
        }

        self.len += 1;
    }

    /// Obtains a pointer to the given entry in the Column.
    /// 
    /// This function returns `None` if the index did not exist. 
    /// 
    /// This function is not marked unsafe because obtaining the pointer itself is a safe operation.
    /// The reference aliasing rules must be upheld manually when dereferencing this pointer however.
    /// 
    /// In other words, if there exists a muColumn reference to this `Column`, you cannot dereference the returned
    /// pointer. If there exists an immuColumn reference to this `Column`, you must cast the pointer to a `*const T` and
    /// only use it as an immuColumn pointer. If there exist no references, you can do either.
    pub fn get<T: 'static>(&self, index: usize) -> Option<NonNull<T>> {
        assert_eq!(
            TypeId::of::<T>(),
            self.ty,
            "Column::get called with mismatched types"
        );

        if index >= self.len {
            return None
        }

        let offset = self.layout.size() * index;
        assert!(offset <= isize::MAX as usize, "Pointer offset overflow in Column::get");

        // Safety:
        //
        // This call to `NonNull::add` is safe because the offset does not overflow `isize` by the assertion
        // above. Additionally, the pointer `self.data` is a valid allocation. Lastly, due to the check
        // above we know that `index < self.len` and the offset result is within this allocation.
        //
        // By the assertion we also know that the pointer is pointing to some type `T`.
        Some(unsafe {
            self.data.unwrap().add(offset).cast::<T>()
        })
    }

    /// Removes the item at index and moves the last item in the Column to the, now empty, slot.
    /// 
    /// # Panics
    /// 
    /// This function panics if the index is out of bounds.
    pub fn swap_remove(&mut self, idx: usize) {
        #[cfg(debug_assertions)]
        let _guard = self.flag.write();

        assert!(idx < self.len, "Column::swap_remove index out of bounds");

        let data_ptr = self.data.unwrap().as_ptr();
        let dst_offset = self.entry_size() * idx;
        assert!(dst_offset <= isize::MAX as usize, "Pointer offset overflow in Column::swap_remove dst pointer");

        // The item to remove and copy into
        //
        // Safety:
        //
        // The offset is guaranteed not to overflow `isize` by the assertion above.
        // Furthermore, `data_ptr` is a valid pointer into an allocation and by the
        // `idx >= self.len` check above, the offset result is also within the allocation.
        let dst_ptr = unsafe {            
            data_ptr.add(dst_offset)
        };

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
            let src_offset = self.entry_size() * (self.len() - 1);
            assert!(src_offset <= isize::MAX as usize, "Pointer offset overflow in Column::swap_remove src pointer");

            // The last item in the array. Will be copied to the empty slot
            //
            // Safety:
            //
            // The offset is guaranteed not to overflow `isize` by the assertion above.
            // Furthermore, `data_ptr` is a valid pointer into an allocation and by the
            // fact the count is `self.len() - 1`, the offset result is also within the allocation.
            let src_ptr = unsafe {
                data_ptr.add(src_offset)
            };

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

    /// The amount of elements currently contained in the Column.
    pub fn len(&self) -> usize {
        #[cfg(debug_assertions)]
        let _guard = self.flag.read();
        self.len
    }

    /// The capacity of the Column.
    pub fn capacity(&self) -> usize {
        #[cfg(debug_assertions)]
        let _guard = self.flag.read();
        self.cap
    }
}

impl Drop for Column {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        let _guard = self.flag.write();

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
                let layout = util::repeat_layout(self.layout, self.cap);
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

/// Iterates over components in a column.
pub struct ColumnIter<'a, T: 'static> {
    index: usize,
    column: &'a Column,
    _marker: PhantomData<&'a T>
}

impl<'a, T: 'static> ColumnIter<'a, T> {
    pub unsafe fn new(
        column: &'a Column
    ) -> ColumnIter<'a, T> {
        #[cfg(debug_assertions)]
        column.flag.read_guardless();

        ColumnIter {
            index: 0,
            column,
            _marker: PhantomData
        }
    }
}

impl<'a, T: 'static> Iterator for ColumnIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        let ptr = self.column.get::<T>(self.index)?;
        self.index += 1;

        Some(unsafe {
            &*ptr.as_ptr().cast_const()
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, T> ExactSizeIterator for ColumnIter<'a, T> {
    fn len(&self) -> usize {
        self.column.len()
    }
}

impl<'a, T> FusedIterator for ColumnIter<'a, T> {}

#[cfg(debug_assertions)]
impl<'a, T> Drop for ColumnIter<'a, T> {
    fn drop(&mut self) {
        self.column.flag.unlock_guardless();
    }
}

/// Iterates over components in an archetype.
pub struct ColumnIterMut<'a, T: 'static> {
    index: usize,
    column: &'a mut Column,
    _marker: PhantomData<&'a T>
}

impl<'a, T: 'static> ColumnIterMut<'a, T> {
    pub unsafe fn new(
        Column: &'a mut Column
    ) -> ColumnIterMut<'a, T> {
        #[cfg(debug_assertions)]
        Column.flag.write_guardless();

        ColumnIterMut {
            index: 0,
            column: Column,
            _marker: PhantomData
        }
    }
}

impl<'a, T: 'static> Iterator for ColumnIterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<&'a mut T> {
        todo!()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, T> ExactSizeIterator for ColumnIterMut<'a, T> {
    fn len(&self) -> usize {
        self.column.len()
    }
}

impl<'a, T> FusedIterator for ColumnIterMut<'a, T> {}

#[cfg(debug_assertions)]
impl<'a, T> Drop for ColumnIterMut<'a, T> {
    fn drop(&mut self) {
        self.column.flag.unlock_guardless();
    }
}

#[derive(Debug)]
pub struct Table {
    #[cfg(debug_assertions)]
    flag: RwFlag,

    components: ArchetypeComponents,    
    // The `entities` and `columnns` fields are perfectly aligned, i.e.
    // an the entity at index 5 in `entities` will have its components stored at index
    // 5 in the `columns` field.
    entities: UnsafeCell<Vec<EntityId>>,
    map: HashMap<ComponentId, Column>
}

impl Table {
    pub fn new<G: SpawnGroup>() -> Table {
        Table {
            #[cfg(debug_assertions)]
            flag: RwFlag::new(),

            components: G::components(),
            entities: UnsafeCell::new(Vec::new()),
            map: G::new_table_map()
        }
    }

    pub fn insert<G: SpawnGroup>(&mut self, entity: EntityId, components: G) {
        let entities = self.entities.get_mut();
        entities.push(entity);

        components.insert_into(&mut self.map);
    }

    pub fn col(&self, id: &ComponentId) -> &Column {
        self.map.get(&id).expect("Column not found in table")
    }

    /// # Safety
    /// 
    /// This function is only safe to call if there exist no muColumn references of this `Archetype`.
    /// Calls to this function must be externally synchronised. Not abiding by these conditions
    /// induces immediate UB.
    pub unsafe fn len(&self) -> usize {
        let len = unsafe {
            &*(self.entities.get() as *const Vec<EntityId>)
        }.len();

        len
    }


}
