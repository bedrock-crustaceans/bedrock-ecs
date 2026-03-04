use std::{alloc::Layout, collections::HashMap};

use crate::{archetype::{ArchetypeComponents, Archetypes}, component::{Component, ComponentId}, entity::EntityId, table::Column};

macro_rules! impl_bundle {
    ($($gen:ident),*) => {
        #[allow(unused_parens)]
        #[diagnostic::do_not_recommend]
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