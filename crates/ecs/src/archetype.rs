use std::{alloc::Layout, collections::HashMap, ptr::NonNull};

use crate::{component::ComponentId, entity::{EntityId, EntityMeta}, spawn::ComponentGroup, util::{self}};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ArchetypeId(pub(crate) usize);

impl From<usize> for ArchetypeId {
    fn from(v: usize) -> ArchetypeId {
        ArchetypeId(v)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArchetypeComponents(pub(crate) Box<[ComponentId]>);

type DropFn = unsafe fn(ptr: *mut u8, len: usize);

unsafe fn drop_wrapper<T>(ptr: *mut u8, len: usize) {
    let ptr = ptr as *mut T;
    for i in 0..len {
        std::ptr::drop_in_place(ptr.add(i));
    }
}

#[derive(Debug)]
pub struct Table {
    layout: Layout,
    len: usize,
    cap: usize,
    data: Option<NonNull<u8>>,
    drop_fn: Option<DropFn>
}

impl Table {
    pub fn new<T>(layout: Layout) -> Table {
        let drop_fn = if std::mem::needs_drop::<T>() {
            Some(drop_wrapper::<T> as DropFn)
        } else {
            None
        };

        Table {
            layout,
            len: 0,
            cap: 0,
            data: None,
            drop_fn
        }
    }

    pub fn element_size(&self) -> usize {
        self.layout.size()
    }

    pub fn reserve(&mut self, additional: usize) {
        let cap = self.cap + additional;
        let layout = util::repeat_layout(self.layout, cap);

        let ptr = if let Some(ptr) = self.data {
            unsafe {
                std::alloc::realloc(ptr.as_ptr(), layout, layout.size())
            }
        } else {
            unsafe {
                std::alloc::alloc(layout)
            }
        };

        // If this line panics, the `Drop` impl will be called with the unchanged pointer, hence
        // deallocating the data.
        self.data = Some(NonNull::new(ptr).expect("Allocating new archetype table failed"));
        self.cap = cap;
    }

    pub fn push(&mut self, data: *const u8) {
        if self.cap <= self.len {
            // Allocate new memory
            if self.cap == 0 {
                // Immediately reserve 4 slots if vec is empty rather than going to 2.
                self.reserve(4);
            } else {
                // Double vec capacity.
                self.reserve(self.cap);
            }
        }

        let size = self.layout.size();
        let table_ptr = unsafe {
            self.data.unwrap().add(self.len * size)
        };
        
        unsafe {
            std::ptr::copy_nonoverlapping(data, table_ptr.as_ptr(), size);
        }

        self.len += 1;
    }

    pub fn swap_remove(&mut self, idx: usize) {
        println!("Swap removing idx {idx}");

        if idx >= self.len {
            panic!("Index out of bounds in Table::swap_remove");
        }

        // If it is the last item, just decrease the length
        if idx == self.len - 1 {
            self.len -= 1;

            // Drop item if required
            if let Some(drop_fn) = self.drop_fn {
                let offset = idx * self.layout.size();
                let ptr = unsafe {
                    self.data.unwrap().add(offset)
                };

                unsafe {
                    drop_fn(ptr.as_ptr(), 1);
                }
            }

            return
        }

        let data_ptr = self.data.unwrap();

        println!("Element size is {}", self.element_size());

        // The item to remove and copy into
        let dst_ptr = unsafe {
            let offset = self.element_size() * idx;
            data_ptr.add(offset)
        };

        // Drop the item if necessary
        if let Some(drop_fn) = self.drop_fn {
            unsafe {
                drop_fn(dst_ptr.as_ptr(), 1);
            }
        }

        // The last item in the array. Will be copied to the empty slot
        let src_ptr = unsafe {
            let offset = self.element_size() * (self.len() - 1);
            data_ptr.add(offset)
        };

        // Then copy the last item into the now empty slot.
        unsafe {
            std::ptr::copy_nonoverlapping(src_ptr.as_ptr(), dst_ptr.as_ptr(), self.layout.size());
        }

        self.len -= 1;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }
}

impl Drop for Table {
    fn drop(&mut self) {
        if let Some(ptr) = self.data {
            // Drop contents if it matters
            if let Some(drop_fn) = self.drop_fn {
                unsafe {
                    drop_fn(ptr.as_ptr(), self.len)
                }
            }

            let layout = util::repeat_layout(self.layout, self.cap);
            unsafe {
                std::alloc::dealloc(ptr.as_ptr(), layout);
            }
        }
    }
}

#[derive(Debug)]
pub struct Archetype {
    components: ArchetypeComponents,    
    // The `entities` and `columnns` fields are perfectly aligned, i.e.
    // an the entity at index 5 in `entities` will have its components stored at index
    // 5 in the `columns` field.
    entities: Vec<EntityId>,
    table: Table
}

impl Archetype {
    pub fn new<T>(components: ArchetypeComponents, layout: Layout) -> Archetype {
        Archetype {
            components,
            entities: Vec::new(),
            table: Table::new::<T>(layout)
        }
    }

    pub fn spawn<G: ComponentGroup>(&mut self, entity: EntityId, group: G) {
        self.entities.push(entity);
        
        let ptr = &group as *const _ as *const u8;
        self.table.push(ptr);

        // Forget memory to prevent freeing the memory that has been copied to the table.
        std::mem::forget(group);
    }

    pub fn despawn(&mut self, entity: EntityId) {
        if let Some(idx) = self.entities.iter().find(|x| **x == entity).copied() {
            let idx = idx.0;
            self.entities.swap_remove(idx);
            self.table.swap_remove(idx);
        }
    }
}

#[derive(Default, Debug)]
pub struct Archetypes {
    archetypes: Vec<Archetype>,
    lookup: HashMap<ArchetypeComponents, ArchetypeId>
}

impl Archetypes {
    pub fn new() -> Archetypes {
        Archetypes::default()
    }

    pub fn insert<G: ComponentGroup>(&mut self, id: EntityId, group: G) {
        let comps = G::archetype();
        let layout = G::layout();

        let idx = self.lookup.get(&comps).copied().unwrap_or_else(|| {
            let archetype = Archetype::new::<G>(comps.clone(), layout);
            self.archetypes.push(archetype);

            let id = ArchetypeId::from(self.archetypes.len() - 1);
            self.lookup.insert(comps, id);

            id
        });

        let archetype = &mut self.archetypes[idx.0];
        archetype.spawn(id, group);
    }

    pub fn remove<G: ComponentGroup>(&mut self, id: EntityId) {
        let comps = G::archetype();
        if let Some(idx) = self.lookup.get(&comps) {
            let archetype = &mut self.archetypes[idx.0];
            archetype.despawn(id);
        }
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
        archetypes.remove::<Test>(EntityId(1));

        // println!("{archetypes:?}");
        println!("Dropping archetypes");
    }
}