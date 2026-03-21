use std::marker::PhantomData;

use generic_array::GenericArray;
use generic_array::typenum::U1;

use crate::event::{Event, Events};
use crate::resource::ResourceId;
use crate::scheduler::{AccessDesc, AccessType};
use crate::sealed::Sealed;
use crate::system::{Param, SystemMeta};
use crate::world::World;

pub struct EventReader<'a, T: Event> {
    _marker: PhantomData<&'a T>,
}

unsafe impl<T: Event> Param for EventReader<'_, T> {
    #[cfg(feature = "generics")]
    type AccessCount = U1;

    type State = ();
    type Output<'a> = EventReader<'a, T>;

    #[cfg(feature = "generics")]
    fn access(_world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount> {
        GenericArray::from((AccessDesc {
            ty: AccessType::Resource(ResourceId::of::<Events<T>>()),
            exclusive: false,
        },))
    }

    #[cfg(not(feature = "generics"))]
    fn access(world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        todo!()
    }

    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut ()) -> EventReader<'w, T> {
        todo!()
    }

    fn init(world: &mut World, _meta: &SystemMeta) {
        // Check whether the event bus exists, otherwise create it.
        if !world.resources.contains::<Events<T>>() {
            world.resources.insert(Events::<T>::new());
        }
    }
}

pub struct EventWriter<'a, T: Event> {
    _marker: PhantomData<&'a mut T>,
}
