use std::any::{Any, TypeId};

use crate::{message::Inbox, resource::Resource};

pub trait Message: Send + Sync + 'static {
    const NAME: &'static str;
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageIndex(pub(crate) usize);

impl MessageIndex {
    pub const FIRST: MessageIndex = MessageIndex(0);
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct MessageId(pub(crate) TypeId);

impl MessageId {
    #[inline]
    pub const fn of<T: Message>() -> MessageId {
        MessageId(TypeId::of::<T>())
    }
}

/// Holds the message buffers for a specific message type.
#[derive(Debug, Clone)]
pub struct Mailbox<T: Message> {
    /// The message buffer for the current tick.
    current: Vec<T>,
    /// The message buffer of the previous tick, this is never pushed to. At the end of the tick
    /// `current` is swapped into `previous`.
    previous: Vec<T>,
    /// The index of the first message in `previous`.
    prev_index: usize,
    /// The index that should be assigned to the next transmitted message.
    next_index: usize,
}

impl<T: Message> Mailbox<T> {
    pub fn new() -> Mailbox<T> {
        Self {
            current: Vec::new(),
            previous: Vec::new(),
            prev_index: 0,
            next_index: 0,
        }
    }

    pub fn allocate(&mut self) -> MessageIndex {
        let curr = self.next_index;
        self.next_index += 1;
        MessageIndex(curr)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.current.len() + self.previous.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.current.is_empty() && self.previous.is_empty()
    }

    /// How many messages there are with a higher index than the given one.
    pub fn count_unread(&self, index: MessageIndex) -> usize {
        let len = self.len() as i64;
        let total_read = index.0 as i64 - self.prev_index as i64;
        (len - total_read).clamp(0, len) as usize
    }

    pub fn get(&self, index: MessageIndex) -> Option<&T> {
        let rel = index.0 - self.prev_index;
        if rel >= self.previous.len() {
            // Message is in current buffer
            self.current.get(rel - self.previous.len())
        } else {
            // Message is in previous buffer
            Some(&self.previous[rel])
        }
    }

    /// Sends a new message and returns its assigned index.
    pub fn send(&mut self, event: T) -> MessageIndex {
        let index = self.allocate();
        self.current.push(event);
        index
    }
}

impl<T: Message> Resource for Mailbox<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

impl<T: Message> Default for Mailbox<T> {
    fn default() -> Mailbox<T> {
        Self::new()
    }
}
