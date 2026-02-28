use crate::entity::Entity;

pub trait FilterGroup {
    fn filter(entity: &Entity) -> bool;
}

impl FilterGroup for () {
    fn filter(_entity: &Entity) -> bool {
        true
    }
}