use std::iter::FusedIterator;

use generic_array::GenericArray;
use generic_array::typenum::U1;

#[cfg(not(feature = "generics"))]
use smallvec::{SmallVec, smallvec};

use crate::message::{Mailbox, Message, MessageIndex};
use crate::resource::ResourceId;
use crate::scheduler::{AccessDesc, AccessType};
use crate::sealed::Sealed;
use crate::system::{Param, SystemMeta};
use crate::world::World;

#[derive(Debug, Default)]
pub struct MessageReceiverState {
    next_index: usize,
}

pub struct MessageReceiver<'a, T: Message> {
    mailbox: &'a Mailbox<T>,
    state: &'a mut MessageReceiverState,
}

impl<T: Message> MessageReceiver<'_, T> {
    /// Returns the index of the next potential message.
    pub fn last_seen(&self) -> MessageIndex {
        MessageIndex(self.state.next_index)
    }
}

impl<'a, T: Message> Iterator for MessageReceiver<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        let index = self.state.next_index;
        let message = self.mailbox.get(MessageIndex(index))?;

        // Only increase after successful read. Otherwise we will skip over future messages every time
        // this call fails.
        self.state.next_index += 1;

        Some(message)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len();
        // `Mailbox` is immutably borrowed while this iterator exists, so messages cannot be pushed to the mailbox.
        // Therefore we know the upper bound will not increase while this iterator exists.
        (remaining, Some(remaining))
    }
}

impl<T: Message> ExactSizeIterator for MessageReceiver<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.mailbox
            .count_unread(MessageIndex(self.state.next_index))
    }
}

// Messages cannot be sent to the mailbox while this iterator exists.
impl<T: Message> FusedIterator for MessageReceiver<'_, T> {}

unsafe impl<T: Message> Param for MessageReceiver<'_, T> {
    #[cfg(feature = "generics")]
    type AccessCount = U1;

    type State = MessageReceiverState;
    type Output<'a> = MessageReceiver<'a, T>;

    #[cfg(feature = "generics")]
    fn access(_world: &mut World) -> GenericArray<AccessDesc, Self::AccessCount> {
        GenericArray::from((AccessDesc {
            ty: AccessType::Resource(ResourceId::of::<Mailbox<T>>()),
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
        state: &'w mut MessageReceiverState,
    ) -> MessageReceiver<'w, T> {
        let bus = world.resources.get::<Mailbox<T>>().unwrap_or_else(|| {
            panic!(
                "event bus for type {} does not exist",
                std::any::type_name::<T>()
            )
        });

        MessageReceiver {
            mailbox: bus,
            state,
        }
    }

    fn init(world: &mut World, _meta: &SystemMeta) -> MessageReceiverState {
        // Check whether the mailbox exists, otherwise create it.
        if !world.resources.contains::<Mailbox<T>>() {
            world.resources.insert(Mailbox::<T>::new());
        }

        MessageReceiverState::default()
    }
}

#[derive(Debug, Default)]
pub struct MessageSenderState {}

pub struct MessageSender<'a, T: Message> {
    mailbox: &'a mut Mailbox<T>,
    state: &'a mut MessageSenderState,
}

impl<T: Message> MessageSender<'_, T> {
    #[inline]
    pub fn send(&mut self, message: T) -> MessageIndex {
        self.mailbox.send(message)
    }
}

unsafe impl<T: Message> Param for MessageSender<'_, T> {
    #[cfg(feature = "generics")]
    type AccessCount = U1;

    type State = MessageSenderState;
    type Output<'a> = MessageSender<'a, T>;

    #[cfg(feature = "generics")]
    fn access(_world: &mut World) -> GenericArray<AccessDesc, U1> {
        GenericArray::from((AccessDesc {
            ty: AccessType::Resource(ResourceId::of::<Mailbox<T>>()),
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
        state: &'w mut MessageSenderState,
    ) -> MessageSender<'w, T> {
        let bus_ptr = world.resources.get_ptr::<Mailbox<T>>().unwrap_or_else(|| {
            panic!(
                "mailbox for type {} does not exist",
                std::any::type_name::<T>()
            )
        });

        // Safety: This is sound because the scheduler ensures that only this system has exclusive access
        // to the `Events<T>` resource.
        let bus = unsafe { &mut *bus_ptr.as_ptr() };

        MessageSender {
            mailbox: bus,
            state,
        }
    }

    fn init(world: &mut World, _meta: &SystemMeta) -> MessageSenderState {
        if !world.resources.contains::<Mailbox<T>>() {
            world.resources.insert(Mailbox::<T>::new());
        }

        MessageSenderState::default()
    }
}
