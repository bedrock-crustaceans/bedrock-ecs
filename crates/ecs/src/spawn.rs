use std::{alloc::Layout, collections::HashMap};

use crate::{archetype::{ArchetypeComponents, Archetypes}, component::{Component, ComponentId}, entity::EntityId, table::Column};

macro_rules! impl_bundle {
    ($($gen:ident),*) => {
        #[allow(unused_parens)]
        unsafe impl<$($gen: Component),*> SpawnBundle for ($($gen),*) {
            fn components() -> ArchetypeComponents {
                let boxed = Box::new([
                    $(ComponentId::of::<$gen>()),*
                ]);

                ArchetypeComponents(boxed)
            }

            fn new_table_map() -> HashMap<ComponentId, Column> {
                HashMap::from([
                    $(
                        (ComponentId::of::<$gen>(), Column::new::<$gen>())
                    ),*
                ])
            }

            #[allow(unused_variables)]
            fn insert_into(self, storage: &mut HashMap<ComponentId, Column>) {
                #[allow(non_snake_case)]
                let ($($gen),*) = self;
                $(
                    let id = ComponentId::of::<$gen>();
                    storage
                        .get_mut(&id)
                        .expect("Failed to insert component from ComponentBundle")
                        .push($gen);
                )*
            }
        }
    }
}

pub unsafe trait SpawnBundle: 'static {
    /// Returns a list of components in this group.
    fn components() -> ArchetypeComponents;
    /// Creates a new table map to store in the archetype.
    fn new_table_map() -> HashMap<ComponentId, Column>;
    /// Inserts into an existing archetype.
    fn insert_into(self, storage: &mut HashMap<ComponentId, Column>);
}


impl_bundle!();
impl_bundle!(C0);
impl_bundle!(C0, C1);
impl_bundle!(C0, C1, C2);
impl_bundle!(C0, C1, C2, C3);
impl_bundle!(C0, C1, C2, C3, C4);