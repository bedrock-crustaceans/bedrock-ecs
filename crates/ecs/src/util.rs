use std::{
    marker::PhantomData,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{EcsError, EcsResult};

#[doc(hidden)]
pub trait __AssertSyncHelper where Self: Sync {}
#[doc(hidden)]
pub trait __AssertSendHelper where Self: Send {}

/// Asserts that the given type implements `Send`.
#[macro_export]
macro_rules! assert_send {
    ($x: ident) => {
        impl crate::util::__AssertSendHelper for $x {}
    }
}

/// Asserts that the given type implements `Sync`.
#[macro_export]
macro_rules! assert_sync {
    ($x: ident) => {
        impl crate::util::__AssertSyncHelper for $x {}
    }
}

/// Asserts that the given type is `dyn` compatible.
#[macro_export]
macro_rules! assert_dyn_compatible {
    ($x: ident) => {
        paste::paste! {
            #[doc(hidden)]
            type [<__AssertDynCompatible $x>] = dyn $x;
        }
    }
}

pub enum WriteLock {}

impl LockKind for WriteLock {
    const READ_ONLY: bool = false;
}

pub enum ReadLock {}

impl LockKind for ReadLock {
    const READ_ONLY: bool = true;
}

pub trait LockKind {
    const READ_ONLY: bool;
}

pub struct LockGuard<'lock, K: LockKind> {
    lock: &'lock PersistentLock,
    _marker: PhantomData<K>,
}

impl<'lock, K: LockKind> Drop for LockGuard<'lock, K> {
    fn drop(&mut self) {
        if K::READ_ONLY {
            // Safety: By the fact this guard exists, the lock must be held.
            // It is therefore safe to unlock it.
            unsafe {
                self.lock.force_release_read();
            }
        } else {
            // Safety: By the fact this guard exists, the lock must be held.
            // It is therefore safe to unlock it.
            unsafe {
                self.lock.force_release_write();
            }
        }
    }
}

pub struct PersistentLock {
    pub(crate) counter: AtomicUsize,
}

impl PersistentLock {
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }

    pub fn read(&self) -> EcsResult<LockGuard<ReadLock>> {
        if self.counter.load(Ordering::SeqCst) == usize::MAX {
            // Lock is already being used for writing.
            return Err(EcsError::StorageLocked(
                "write lock active, cannot acquire read lock",
            ));
        }

        self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(LockGuard {
            lock: self,
            _marker: PhantomData,
        })
    }

    pub unsafe fn force_release_read(&self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }

    pub fn write(&self) -> EcsResult<LockGuard<WriteLock>> {
        if self.counter.load(Ordering::SeqCst) != 0 {
            // Lock is already being used for reading or writing.
            return Err(EcsError::StorageLocked(
                "read or write lock active, cannot acquire write lock",
            ));
        }

        self.counter.store(usize::MAX, Ordering::SeqCst);
        Ok(LockGuard {
            lock: self,
            _marker: PhantomData,
        })
    }

    pub unsafe fn force_release_write(&self) {
        self.counter.store(0, Ordering::SeqCst);
    }
}
