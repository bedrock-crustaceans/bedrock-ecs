#[cfg(debug_assertions)]
use std::sync::{Arc, atomic::AtomicBool};
use std::{alloc::Layout, any::TypeId, cell::UnsafeCell, collections::HashMap, iter::FusedIterator, marker::PhantomData, ptr::NonNull, sync::atomic::Ordering};

#[cfg(debug_assertions)]
use crate::util::debug::RwFlag;
use crate::{component::ComponentId, entity::{EntityId, EntityMeta}, spawn::ComponentGroup, util::{self}};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ArchetypeId(pub(crate) usize);

impl From<usize> for ArchetypeId {
    fn from(v: usize) -> ArchetypeId {
        ArchetypeId(v)
    }
}

/// A list of components contained in an archetype.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArchetypeComponents(pub(crate) Box<[ComponentId]>);

/// A function pointer to a function that can drop an array of elements.
type DropFn = unsafe fn(ptr: *mut u8, len: usize);

/// Drops `len` items of type `T` contained in `ptr`.
/// 
/// This function is used to invoke the `Drop` implementation on the items in a `Table`.
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
pub struct Table {
    /// The type ID of the item contained in this table.
    ty: TypeId,
    /// The layout of the component type.
    layout: Layout,
    /// The amount of items contained in this table.
    len: usize,
    /// The capacity of this table.
    cap: usize,
    /// An optional pointer to the buffer that holds the table data.
    /// 
    /// This field is `None` if and only if `cap` is 0.
    data: Option<NonNull<u8>>,
    /// The function that is called when an item is dropped.
    /// 
    /// This field is `None` if the item does not require dropping, i.e.
    /// `std::mem::needs_drop<T>` returned false.
    drop_fn: Option<DropFn>
}

impl Table {
    /// Creates a new table for the type `T`.
    pub fn new<T: 'static>(layout: Layout) -> Table {
        // The static requirement on `T` ensures that the type does not contain any references.

        let drop_fn = if std::mem::needs_drop::<T>() {
            Some(drop_wrapper::<T> as DropFn)
        } else {
            None
        };

        if std::mem::size_of::<T>() == 0 {
            // Produce a valid non-null pointer for the ZST even though we will never use it.
            let ptr = NonNull::<T>::dangling().cast::<u8>();

            Table {
                ty: TypeId::of::<T>(),
                layout,
                len: 0,
                // Set capacity to max to disable allocations.
                cap: usize::MAX,
                data: Some(ptr),
                drop_fn
            }
        } else {
            Table {
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
    /// `T` is the type contained in this `Table`.
    pub fn entry_size(&self) -> usize {
        self.layout.size()
    }

    /// Reserves capacity for `n` additional entries.
    pub fn reserve(&mut self, n: usize) {
        assert_ne!(self.layout.size(), 0, "Table::reserve should not be called for ZSTs");

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
            // stored in the table unchanged. Furthermore, the pointer used to reallocate is the one
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
        self.data = Some(NonNull::new(ptr).expect("Table::reserve allocation failed"));
        self.cap = cap;
    }

    /// Pushes a new entry into the table. 
    /// 
    /// # Panics
    /// 
    /// This function panics if the given generic `T` is not the same as the `T` that was used in the call
    /// to `Table::new`. This `T` is not stored in the `Table` type to prevent the runtime cost of dynamic dispatch.
    pub fn push<T: 'static>(&mut self, data: T) {
        assert_eq!(
            TypeId::of::<T>(),
            self.ty,
            "Table::push called with mismatched types"
        );

        if self.cap <= self.len {
            // Reserve at least 4 slots to reduce allocations at the start.
            let new_slots = self.cap.clamp(4, usize::MAX);
            self.reserve(new_slots);
        }

        let offset = self.layout.size() * self.len;
        assert!(offset <= isize::MAX as usize, "Pointer offset overflow in Table::push");

        // Safety:
        //
        // The computed offset does not overflow `isize` by the assert above and the original pointer
        // `self.data` is derived from an allocation while the offset result is within the allocation due
        // to the check that `self.len < self.cap`.
        let table_ptr = unsafe {
            self.data.unwrap().add(offset)
        };
        
        // Safety:
        //
        // This is safe since the pointer is guaranteed to be valid by the length check above.
        // Additionally `std::ptr::write` semantically moves `data` into the table, ensuring it does
        // not get dropped. This ensures there is no use after free.
        unsafe {
            std::ptr::write(table_ptr.cast::<T>().as_ptr(), data);
        }

        self.len += 1;
    }

    /// Obtains a pointer to the given entry in the table.
    /// 
    /// This function returns `None` if the index did not exist. 
    /// 
    /// This function is not marked unsafe because obtaining the pointer itself is a safe operation.
    /// The reference aliasing rules must be upheld manually when dereferencing this pointer however.
    /// 
    /// In other words, if there exists a mutable reference to this `Table`, you cannot dereference the returned
    /// pointer. If there exists an immutable reference to this `Table`, you must cast the pointer to a `*const T` and
    /// only use it as an immutable pointer. If there exist no references, you can do either.
    pub fn get<T: 'static>(&self, index: usize) -> Option<NonNull<T>> {
        assert_eq!(
            TypeId::of::<T>(),
            self.ty,
            "Table::get called with mismatched types"
        );

        if index >= self.len {
            return None
        }

        let offset = self.layout.size() * index;
        assert!(offset <= isize::MAX as usize, "Pointer offset overflow in Table::get");

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

    /// Removes the item at index and moves the last item in the table to the, now empty, slot.
    /// 
    /// # Panics
    /// 
    /// This function panics if the index is out of bounds.
    pub fn swap_remove(&mut self, idx: usize) {
        if idx >= self.len {
            panic!("Table::swap_remove index out of bounds");
        }

        // If it is the last item, just decrease the length
        if idx == self.len - 1 {
            self.len -= 1;

            // Drop item if required
            if let Some(drop_fn) = self.drop_fn {
                let offset = idx * self.layout.size();
                assert!(offset <= isize::MAX as usize, "Pointer offset overflow in Table::swap_remove drop");

                // Safety:
                //
                // The offset is guaranteed to not overflow `isize` by the assertion above.
                // Additionally, `self.data` is a valid pointer into an allocation and by the fact that
                // `idx == self.len - 1` we know that the offset result is also contained in the allocation.
                let ptr = unsafe {
                    self.data.unwrap().add(offset)
                };
                
                // Safety:
                //
                // This call is safe because `ptr` is guaranteed to point to a valid memory block of type `T`,
                // where `T` is the type used in `Table::new`. Additionally it contains at least 1 item, thus the
                // size is correct.
                //
                // Lastly this is a valid function pointer since it is only set in `Table::new`.
                unsafe {
                    drop_fn(ptr.as_ptr(), 1);
                }
            }

            return
        }

        let data_ptr = self.data.unwrap();
        let dst_offset = self.entry_size() * idx;
        assert!(dst_offset <= isize::MAX as usize, "Pointer offset overflow in Table::swap_remove dst pointer");

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
            // where `T` is the type used in `Table::new`. Additionally it contains at least 1 item, thus the
            // size is correct.
            //
            // Lastly this is a valid function pointer since it is only set in `Table::new`.
            unsafe {
                f(dst_ptr.as_ptr(), 1);
            }
        });

        let src_offset = self.entry_size() * (self.len() - 1);
        assert!(src_offset <= isize::MAX as usize, "Pointer offset overflow in Table::swap_remove src pointer");

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
            std::ptr::copy_nonoverlapping(src_ptr.as_ptr(), dst_ptr.as_ptr(), self.layout.size());
        }

        self.len -= 1;
    }

    /// The amount of elements currently contained in the table.
    pub fn len(&self) -> usize {
        self.len
    }

    /// The capacity of the table.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

impl Drop for Table {
    fn drop(&mut self) {
        if let Some(ptr) = self.data {
            // Drop contents if it matters
            if let Some(drop_fn) = self.drop_fn {
                // Safety:
                //
                // This call is safe because `ptr` is guaranteed to point to a valid memory block of type `T`,
                // where `T` is the type used in `Table::new`. Additionally it contains at least 1 item, thus the
                // size is correct.
                //
                // Lastly this is a valid function pointer since it is only set in `Table::new`.
                unsafe {
                    drop_fn(ptr.as_ptr(), self.len)
                }
            }

            let layout = util::repeat_layout(self.layout, self.cap);
            // Safety:
            //
            // This is safe because `ptr` has previously been allocated with `alloc` and
            // layout was the layout originally used to create this specific allocation.
            unsafe {
                std::alloc::dealloc(ptr.as_ptr(), layout);
            }
        }
    }
}

/// Iterates over components in an archetype.
pub struct ArchetypeIter<'a, T: 'static> {
    index: usize,
    archetype: &'a Archetype,
    _marker: PhantomData<&'a T>
}

impl<'a, T: 'static> ArchetypeIter<'a, T> {
    pub unsafe fn new(
        archetype: &'a Archetype, 
        #[cfg(debug_assertions)]
        mutable: bool
    ) -> ArchetypeIter<'a, T> {
        #[cfg(debug_assertions)]
        {
            if mutable {
                archetype.flag.write();
            } else {
                archetype.flag.read();
            }
        }

        ArchetypeIter {
            index: 0,
            archetype
        }
    }
}

impl<'a> Iterator for ArchetypeIter<'a> {
    type Item = NonNull<u8>;

    fn next(&mut self) -> Option<NonNull<u8>> {
        let item = unsafe {
            self.archetype.get(self.index)?
        };
        self.index += 1;
        Some(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a> ExactSizeIterator for ArchetypeIter<'a> {
    fn len(&self) -> usize {
        self.archetype.len() - self.index
    }
}

impl<'a> FusedIterator for ArchetypeIter<'a> {}

#[cfg(debug_assertions)]
impl<'a> Drop for ArchetypeIter<'a> {
    fn drop(&mut self) {
        self.archetype.flag.unlock();
    }
}

#[derive(Debug)]
pub struct Archetype {
    #[cfg(debug_assertions)]
    flag: RwFlag,

    components: ArchetypeComponents,    
    // The `entities` and `columnns` fields are perfectly aligned, i.e.
    // an the entity at index 5 in `entities` will have its components stored at index
    // 5 in the `columns` field.
    entities: UnsafeCell<Vec<EntityId>>,
    table: UnsafeCell<Table>
}

impl Archetype {
    pub fn new<T>(components: ArchetypeComponents, layout: Layout) -> Archetype {
        Archetype {
            #[cfg(debug_assertions)]
            flag: RwFlag::new(),

            components,
            entities: UnsafeCell::new(Vec::new()),
            table: UnsafeCell::new(Table::new::<T>(layout))
        }
    }

    /// # Safety
    /// 
    /// This function is only safe to call if there exist no mutable references of this `Archetype`.
    /// Calls to this function must be externally synchronised. Not abiding by these conditions
    /// induces immediate UB.
    pub unsafe fn len(&self) -> usize {
        #[cfg(debug_assertions)]
        self.flag.read();

        let len = unsafe {
            &*(self.entities.get() as *const Vec<EntityId>)
        }.len();

        #[cfg(debug_assertions)]
        self.flag.unlock();

        len
    }

    /// # Safety
    /// 
    /// This function must only be called if there exist no other (mutable or immutable) references to this `Archetype`
    /// Calls to this function must be externally synchronised. Not abiding by these conditions
    /// induces immediate UB.
    pub unsafe fn insert<G: ComponentGroup>(&mut self, entity: EntityId, group: G) {
        #[cfg(debug_assertions)]
        self.flag.write();

        unsafe {
            &mut *self.entities.get()
        }.push(entity);
        
        let ptr = (&raw const group).cast::<u8>();
        unsafe {
            &mut *self.table.get()
        }.push(ptr);

        #[cfg(debug_assertions)]
        self.flag.unlock();

        // Forget memory to prevent freeing the memory that has been copied to the table.
        std::mem::forget(group);
    }

    /// # Safety
    /// 
    /// This function returns a `*mut u8` pointer. 
    /// 
    /// Calls to this function must be externally synchronised. Not abiding by these conditions
    /// induces immediate UB.
    pub unsafe fn get(&self, index: usize) -> Option<NonNull<u8>> {
        self.table.get(index)
    }

    pub fn despawn(&mut self, entity: EntityId) {
        #[cfg(debug_assertions)]
        self.flag.write();

        if let Some(idx) = self.entities.iter().find(|x| **x == entity).copied() {
            let idx = idx.0;
            self.entities.swap_remove(idx);
            self.table.swap_remove(idx);
        }

        #[cfg(debug_assertions)]
        self.flag.unlock();
    }
}

#[derive(Default, Debug)]
pub struct Archetypes {
    archetypes: HashMap<ArchetypeComponents, Archetype>,
    // lookup: HashMap<ArchetypeComponents, ArchetypeId>
}

impl Archetypes {
    pub fn new() -> Archetypes {
        Archetypes::default()
    }

    pub fn insert<G: ComponentGroup>(&mut self, id: EntityId, group: G) {
        // let comps = G::archetype();
        // let layout = G::layout();

        // let idx = self.lookup.get(&comps).copied().unwrap_or_else(|| {
        //     let archetype = Archetype::new::<G>(comps.clone(), layout);
        //     self.archetypes.push(archetype);

        //     let id = ArchetypeId::from(self.archetypes.len() - 1);
        //     self.lookup.insert(comps, id);

        //     id
        // });

        // let archetype = &mut self.archetypes[idx.0];
        // archetype.spawn(id, group);

        let comps = G::archetype();
        let layout = G::layout();

        let archetype = self.archetypes.entry(comps.clone())
            .or_insert_with(|| {
                Archetype::new::<G>(comps, layout)
            });

        archetype.insert(id, group);
    }

    pub fn get(&self, id: &ArchetypeComponents) -> Option<&Archetype> {
        self.archetypes.get(id)
    }
    
    pub fn remove(&mut self, id: &ArchetypeComponents) -> Option<Archetype> {
        self.archetypes.remove(id)
    }

    // pub fn insert(&mut self, id: EntityId, components: ArchetypeComponents, layout: Layout) {
    //     let idx = self.lookup.get(&components).copied().unwrap_or_else(|| {
    //         let archetype = Archetype::new(components.clone(), layout);
    //         self.archetypes.push(archetype);
            
    //         let id = ArchetypeId::from(self.archetypes.len() - 1);
    //         self.lookup.insert(components, id);

    //         id
    //     });

    //     let archetype = &mut self.archetypes[idx.0];
    //     todo!();
    // }
}

#[cfg(test)]
mod test {
    use crate::{archetype::Archetypes, component::Component, entity::EntityId};

    struct Test {
        hello: usize
    }

    impl Component for Test {}

    impl Drop for Test {
        fn drop(&mut self) {
            println!("Test {} has been dropped", self.hello);
        }
    }

    #[test]
    fn create_archetype() {
        let mut archetypes = Archetypes::new();
        
        archetypes.insert(EntityId(0), Test { hello: 0 });
        archetypes.insert(EntityId(1), Test { hello: 1 });
        archetypes.insert(EntityId(2), Test { hello: 2 });
        

        // println!("{archetypes:?}");
        println!("Dropping archetypes");
    }
}