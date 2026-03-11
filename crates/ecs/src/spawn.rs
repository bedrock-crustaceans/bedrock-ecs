use std::{alloc::Layout, any::TypeId, collections::HashMap};

use crate::{archetype::{ArchetypeComponents, Archetypes}, component::{Component, ComponentId}, entity::EntityId, table::{Column, Table}};

#[cfg(debug_assertions)]
use crate::util::debug::RwFlag;

use std::cell::UnsafeCell;

macro_rules! impl_bundle {
    ($($gen:ident),*) => {
        #[allow(unused_parens)]
        #[diagnostic::do_not_recommend]
        unsafe impl<$($gen: Component),*> SpawnBundle for ($($gen),*) {
            fn components() -> Box<[TypeId]> {
                let boxed = Box::new([
                    $(TypeId::of::<$gen>()),*
                ]);

                boxed
            }

            fn new_table() -> Table {
                const COUNT: usize = (&[$( stringify!($gen) ),*] as &[&str]).len();

                let components = Self::components();

                #[allow(unused)]
                let mut table = Table {
                    #[cfg(debug_assertions)]
                    flag: RwFlag::new(),

                    components,
                    entities: UnsafeCell::new(Vec::new()),
                    lookup: HashMap::with_capacity(COUNT),
                    columns: vec![
                        $(
                            Column::new::<$gen>()
                        ),*
                    ]
                };

                #[allow(unused)]
                {
                    let mut counter = 0;
                    $(
                        table.lookup.insert(TypeId::of::<$gen>(), counter);
                        counter += 1;   
                    )*
                }

                table
            }

            #[allow(unused_variables)]
            fn insert_into(self, storage: &mut Vec<Column>) {
                #[allow(non_snake_case)]
                let ($($gen),*) = self;

                #[allow(unused)]
                {
                    let mut counter = 0;
                    $(
                        storage[counter].push($gen);
                        counter += 1;
                    )*
                }
            }
        }
    }
}

pub unsafe trait SpawnBundle: 'static {
    /// Returns a list of components in this group.
    fn components() -> Box<[TypeId]>;
    /// Creates a new table map to store in the archetype.
    fn new_table() -> Table;
    /// Inserts into an existing archetype.
    fn insert_into(self, storage: &mut Vec<Column>);
}


impl_bundle!();
impl_bundle!(A);
impl_bundle!(A, B);
impl_bundle!(A, B, C);
impl_bundle!(A, B, C, D);
impl_bundle!(A, B, C, D, E);
impl_bundle!(A, B, C, D, E, F);
impl_bundle!(A, B, C, D, E, F, G);
impl_bundle!(A, B, C, D, E, F, G, H);
impl_bundle!(A, B, C, D, E, F, G, H, I);
impl_bundle!(A, B, C, D, E, F, G, H, I, J);