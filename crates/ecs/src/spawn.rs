use std::{alloc::Layout, any::TypeId, collections::HashMap};

use crate::{archetype::{ArchetypeComponents, Archetypes}, bitset::BitSet, component::{Component, ComponentId, ComponentRegistry}, entity::EntityId, table::{Column, Table}};

#[cfg(debug_assertions)]
use crate::util::debug::RwFlag;

use std::cell::UnsafeCell;

macro_rules! impl_bundle {
    ($($gen:ident),*) => {
        #[allow(unused_parens)]
        #[diagnostic::do_not_recommend]
        unsafe impl<$($gen: Component),*> SpawnBundle for ($($gen),*) {
            #[allow(unused)]
            fn components(reg: &mut ComponentRegistry) -> BitSet {
                let mut bitset = BitSet::new();
                $(
                    let id = *reg.get_or_assign::<$gen>();
                    bitset.set(id);
                )*
                bitset
            }

            #[cfg_attr(
                feature = "tracing",
                tracing::instrument(name = "SpawnBundle::new_table", fields(bundle = std::any::type_name::<Self>()) skip_all)  
            )]
            fn new_table(bitset: BitSet) -> Table {
                const COUNT: usize = (&[$( stringify!($gen) ),*] as &[&str]).len();

                #[allow(unused)]
                let mut table = Table {
                    #[cfg(debug_assertions)]
                    flag: RwFlag::new(),

                    archetype: bitset,
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
                        tracing::info!("inserting type {}", std::any::type_name::<$gen>());
                        table.lookup.insert(TypeId::of::<$gen>(), counter);
                        counter += 1;   
                    )*
                }

                table
            }

            #[allow(unused_variables)]
            #[cfg_attr(
                feature = "tracing", 
                tracing::instrument(name = "SpawnBundle::insert_into", fields(bundle = std::any::type_name::<Self>()) skip_all)
            )]
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
    fn components(reg: &mut ComponentRegistry) -> BitSet;
    /// Creates a new table map to store in the archetype.
    fn new_table(bitset: BitSet) -> Table;
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