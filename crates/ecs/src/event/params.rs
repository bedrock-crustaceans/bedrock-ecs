use generic_array::GenericArray;
use generic_array::typenum::U1;
use smallvec::{SmallVec, smallvec};

use crate::event::{Event, Events};
use crate::resource::ResourceId;
use crate::scheduler::{AccessDesc, AccessType};
use crate::sealed::Sealed;
use crate::system::{Param, SystemMeta};
use crate::world::World;

#[derive(Debug, Default)]
pub struct EventReaderState {
    last_read: usize,
}

pub struct EventReader<'a, T: Event> {
    bus: &'a Events<T>,
    state: &'a mut EventReaderState,
}

unsafe impl<T: Event> Param for EventReader<'_, T> {
    #[cfg(feature = "generics")]
    type AccessCount = U1;

    type State = EventReaderState;
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
        smallvec![AccessDesc {
            ty: AccessType::Resource(ResourceId::of::<Events<T>>()),
            exclusive: false
        }]
    }

    fn fetch<'w, S: Sealed>(
        world: &'w World,
        state: &'w mut EventReaderState,
    ) -> EventReader<'w, T> {
        let bus = world.resources.get::<Events<T>>().unwrap_or_else(|| {
            panic!(
                "event bus for type {} does not exist",
                std::any::type_name::<T>()
            )
        });

        EventReader { bus, state }
    }

    fn init(world: &mut World, _meta: &SystemMeta) -> EventReaderState {
        // Check whether the event bus exists, otherwise create it.
        if !world.resources.contains::<Events<T>>() {
            world.resources.insert(Events::<T>::new());
        }

        EventReaderState::default()
    }
}

#[derive(Debug, Default)]
pub struct EventWriterState {}

pub struct EventWriter<'a, T: Event> {
    bus: &'a mut Events<T>,
    state: &'a mut EventWriterState,
}

unsafe impl<T: Event> Param for EventWriter<'_, T> {
    #[cfg(feature = "generics")]
    type AccessCount = U1;

    type State = EventWriterState;
    type Output<'a> = EventWriter<'a, T>;

    #[cfg(feature = "generics")]
    fn access(_world: &mut World) -> GenericArray<AccessDesc, U1> {
        GenericArray::from((AccessDesc {
            ty: AccessType::Resource(ResourceId::of::<Events<T>>()),
            exclusive: true,
        },))
    }

    #[cfg(not(feature = "generics"))]
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        smallvec![AccessDesc {
            ty: AccessType::Resource(ResourceId::of::<Events<T>>()),
            exclusive: true
        }]
    }

    fn fetch<'w, S: Sealed>(
        world: &'w World,
        state: &'w mut EventWriterState,
    ) -> EventWriter<'w, T> {
        let bus_ptr = world.resources.get_ptr::<Events<T>>().unwrap_or_else(|| {
            panic!(
                "event bus for type {} does not exist",
                std::any::type_name::<T>()
            )
        });

        // Safety: This is sound because the scheduler ensures that only this system has exclusive access
        // to the `Events<T>` resource.
        let bus = unsafe { &mut *bus_ptr.as_ptr() };

        EventWriter { bus, state }
    }

    fn init(world: &mut World, _meta: &SystemMeta) -> EventWriterState {
        if !world.resources.contains::<Events<T>>() {
            world.resources.insert(Events::<T>::new());
        }

        EventWriterState::default()
    }
}
