use std::any::TypeId;

use crate::{
    archetype::Signature,
    component::{Component, ComponentRegistry},
    table::{Column, Table},
};

use rustc_hash::{FxBuildHasher, FxHashMap};

macro_rules! impl_bundle {
    ($($gen:ident),*) => {
        #[allow(unused_parens)]
        #[diagnostic::do_not_recommend]
        unsafe impl<$($gen: Component),*> SpawnBundle for ($($gen),*) {
            #[allow(unused)]
            fn signature(reg: &mut ComponentRegistry) -> Signature {
                let mut sig = Signature::new();
                $(
                    let id = *reg.get_or_assign::<$gen>();
                    sig.set(id);
                )*
                sig
            }

            #[cfg_attr(
                feature = "tracing",
                tracing::instrument(name = "SpawnBundle::new_table", fields(bundle = std::any::type_name::<Self>()) skip_all)
            )]
            fn new_table(sig: Signature) -> Table {
                const COUNT: usize = (&[$( stringify!($gen) ),*] as &[&str]).len();

                #[allow(unused)]
                let mut table = Table {
                    signature: sig,
                    entities: Vec::new(),
                    lookup: FxHashMap::with_capacity_and_hasher(COUNT, FxBuildHasher::default()),
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
                        tracing::trace!("inserting type {}", std::any::type_name::<$gen>());
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
    fn signature(reg: &mut ComponentRegistry) -> Signature;
    /// Creates a new table map to store in the archetype.
    fn new_table(bitset: Signature) -> Table;
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
