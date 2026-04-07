use std::{
    alloc::Layout,
    cell::UnsafeCell,
    marker::PhantomData,
    num::NonZero,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

/// Creates an array of type system integers.
#[cfg(feature = "generics")]
#[macro_export]
macro_rules! create_tarray {
    ($head:ty) => {
        generic_array::typenum::TArr<$head, generic_array::typenum::ATerm>
    };
    ($head:ty, $($tail:ty),*) => {
        generic_array::typenum::TArr<$head, $crate::create_tarray!($($tail),*)>
    }
}

#[repr(transparent)]
#[derive(Debug, Default)]
pub struct SyncUnsafeCell<T: Send + Sync>(UnsafeCell<T>);

impl<T: Send + Sync> SyncUnsafeCell<T> {
    #[inline]
    pub fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }

    #[inline]
    pub fn get(&self) -> *mut T {
        self.0.get()
    }
}

unsafe impl<T: Send + Sync> Send for SyncUnsafeCell<T> {}
unsafe impl<T: Send + Sync> Sync for SyncUnsafeCell<T> {}

pub trait AsConstNonNull<T: Send + Sync + ?Sized> {
    fn as_const_non_null(&self) -> ConstNonNull<T>;
}

impl<T: Send + Sync> AsConstNonNull<T> for Vec<T> {
    #[inline]
    fn as_const_non_null(&self) -> ConstNonNull<T> {
        unsafe { ConstNonNull::new_unchecked(self.as_ptr()) }
    }
}

impl<T: Send + Sync + ?Sized> AsConstNonNull<T> for Box<T> {
    #[inline]
    fn as_const_non_null(&self) -> ConstNonNull<T> {
        unsafe { ConstNonNull::new_unchecked(self.as_ref() as *const T) }
    }
}

#[repr(transparent)]
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct MutNonNull<T: Send + Sync + ?Sized>(NonNull<T>);

impl<T: Send + Sync + ?Sized> Clone for MutNonNull<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<T: Send + Sync + ?Sized> Copy for MutNonNull<T> {}

impl<T: Send + Sync + ?Sized> MutNonNull<T> {
    /// Creates a new [`MutNonNull`], returning `None` if the pointer is null.
    #[inline]
    pub const fn new(ptr: *mut T) -> Option<Self> {
        // `?` is not `const`-stable.
        if ptr.is_null() {
            None
        } else {
            Some(Self(unsafe { NonNull::new_unchecked(ptr) }))
        }
    }

    pub const fn cast<U: Send + Sync>(self) -> MutNonNull<U> {
        MutNonNull(self.0.cast::<U>())
    }

    /// # Safety
    ///
    /// `ptr` must be non-null.
    #[inline]
    pub const unsafe fn new_unchecked(ptr: *mut T) -> Self {
        Self(unsafe { NonNull::new_unchecked(ptr) })
    }

    #[inline]
    pub const fn as_ptr(&self) -> *mut T {
        self.0.as_ptr()
    }
}

impl<T: Send + Sync> MutNonNull<T> {
    pub const fn dangling() -> Self {
        Self(NonNull::<T>::dangling())
    }

    #[inline]
    pub const fn without_provenance(ptr: NonZero<usize>) -> Self {
        Self(NonNull::without_provenance(ptr))
    }

    /// # Safety
    ///
    /// Same conditions as [`ptr::add`].
    ///
    /// [`ptr::add`]: std::ptr::add
    #[inline]
    pub const unsafe fn add(&self, n: usize) -> Self {
        debug_assert!(n < isize::MAX as usize);

        Self(unsafe { self.0.add(n) })
    }

    #[inline]
    pub const unsafe fn offset(&self, n: isize) -> Self {
        Self(unsafe { self.0.offset(n) })
    }
}

impl<T: Send + Sync> From<NonNull<T>> for MutNonNull<T> {
    #[inline]
    fn from(value: NonNull<T>) -> Self {
        Self(value)
    }
}

unsafe impl<T: Send + Sync> Send for MutNonNull<T> {}
unsafe impl<T: Send + Sync> Sync for MutNonNull<T> {}

/// [`NonNull`] but wrapping a `const` pointer instead.
///
/// Internally this is just a [`NonNull`] to make use of niche optimisations,
/// but the public API only allows const usage.
///
/// [`NonNull`]: std::ptr::NonNull
#[repr(transparent)]
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ConstNonNull<T: Send + Sync + ?Sized>(NonNull<T>);

impl<T: Send + Sync + ?Sized> Clone for ConstNonNull<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<T: Send + Sync + ?Sized> Copy for ConstNonNull<T> {}

impl<T: Send + Sync + ?Sized> ConstNonNull<T> {
    /// Creates a new [`ConstNonNull`], returning `None` if the pointer is null.
    #[inline]
    pub const fn new(ptr: *const T) -> Option<Self> {
        // `?` is not `const`-stable.
        if ptr.is_null() {
            None
        } else {
            Some(Self(unsafe { NonNull::new_unchecked(ptr.cast_mut()) }))
        }
    }

    /// # Safety
    ///
    /// `ptr` must be non-null.
    #[inline]
    pub const unsafe fn new_unchecked(ptr: *const T) -> Self {
        Self(unsafe { NonNull::new_unchecked(ptr.cast_mut()) })
    }

    #[inline]
    pub const fn as_ptr(&self) -> *const T {
        self.0.as_ptr()
    }
}

impl<T: Send + Sync> ConstNonNull<T> {
    pub const fn dangling() -> Self {
        Self(NonNull::<T>::dangling())
    }

    #[inline]
    pub const fn without_provenance(ptr: NonZero<usize>) -> Self {
        Self(NonNull::without_provenance(ptr))
    }

    /// # Safety
    ///
    /// Same conditions as [`ptr::add`].
    ///
    /// [`ptr::add`]: std::ptr::add
    #[inline]
    pub const unsafe fn add(&self, n: usize) -> Self {
        debug_assert!(n < isize::MAX as usize);

        Self(unsafe { self.0.add(n) })
    }

    #[inline]
    pub const unsafe fn offset(&self, n: isize) -> Self {
        Self(unsafe { self.0.offset(n) })
    }
}

impl<T: Send + Sync> From<NonNull<T>> for ConstNonNull<T> {
    #[inline]
    fn from(value: NonNull<T>) -> Self {
        Self(value)
    }
}

impl<T: Send + Sync> From<MutNonNull<T>> for ConstNonNull<T> {
    #[inline]
    fn from(value: MutNonNull<T>) -> Self {
        Self(value.0)
    }
}

unsafe impl<T: Send + Sync> Send for ConstNonNull<T> {}
unsafe impl<T: Send + Sync> Sync for ConstNonNull<T> {}

pub trait LayoutExt {
    fn repeat_packed_ext(&self, n: usize) -> Option<Layout>;
    fn repeat_ext(&self, n: usize) -> Option<(Layout, usize)>;
}

impl LayoutExt for Layout {
    fn repeat_packed_ext(&self, n: usize) -> Option<Layout> {
        if let Some(size) = self.size().checked_mul(n) {
            Layout::from_size_align(size, self.align()).ok()
        } else {
            None
        }
    }

    fn repeat_ext(&self, n: usize) -> Option<(Layout, usize)> {
        let padded = self.pad_to_align();
        padded.repeat_packed_ext(n).map(|r| (r, padded.size()))
    }
}

#[cfg(debug_assertions)]
pub mod debug {
    use std::{
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        thread::ThreadId,
    };

    #[derive(Default, Debug)]
    pub struct BorrowEnforcer {
        last_call: Mutex<String>,
        shared: Arc<AtomicUsize>,
        exclusive: Arc<AtomicUsize>,
        holder: Arc<Mutex<Option<ThreadId>>>,
    }

    impl BorrowEnforcer {
        pub fn new() -> BorrowEnforcer {
            Self::default()
        }

        /// Adds a reader to the enforcer.
        ///
        /// # Panics
        ///
        /// This function panics if a writer already exists.
        #[must_use = "the read guard must be held across the point where the data is used"]
        #[track_caller]
        pub fn read(&self) -> ReadGuard {
            assert_eq!(
                self.exclusive.load(Ordering::SeqCst),
                0,
                "attempt to read while writer is active. last caller: {}",
                self.last_call.lock().unwrap()
            );

            *self.last_call.lock().unwrap() = std::panic::Location::caller().to_string();
            self.shared.fetch_add(1, Ordering::SeqCst);
            ReadGuard {
                counter: self.shared.clone(),
            }
        }

        /// Adds a writer to the enforcer.
        ///
        /// # Panics
        ///
        /// This function panics if a reader or writer already exists.
        #[must_use = "the write guard must be held across the point where the data is used"]
        #[track_caller]
        pub fn write(&self) -> WriteGuard {
            assert_eq!(
                self.shared.load(Ordering::SeqCst),
                0,
                "attempt to write while readers are active. last caller: {}",
                self.last_call.lock().unwrap()
            );

            let current_id = std::thread::current().id();

            let mut lock = self.holder.lock().unwrap();
            if let Some(id) = *lock {
                assert_eq!(
                    id,
                    current_id,
                    "attempt to write while writer is active. last caller: {}",
                    self.last_call.lock().unwrap()
                );
            }

            *self.last_call.lock().unwrap() = std::panic::Location::caller().to_string();

            *lock = Some(std::thread::current().id());
            self.exclusive.fetch_add(1, Ordering::SeqCst);

            WriteGuard {
                counter: self.exclusive.clone(),
                holder: self.holder.clone(),
            }
        }
    }

    pub struct ReadGuard {
        counter: Arc<AtomicUsize>,
    }

    impl Clone for ReadGuard {
        fn clone(&self) -> ReadGuard {
            self.counter.fetch_add(1, Ordering::SeqCst);

            ReadGuard {
                counter: self.counter.clone(),
            }
        }
    }

    impl Drop for ReadGuard {
        fn drop(&mut self) {
            self.counter.fetch_sub(1, Ordering::SeqCst);
        }
    }

    pub struct WriteGuard {
        counter: Arc<AtomicUsize>,
        holder: Arc<Mutex<Option<ThreadId>>>,
    }

    impl Drop for WriteGuard {
        fn drop(&mut self) {
            self.counter.fetch_sub(1, Ordering::SeqCst);

            let mut lock = self.holder.lock().unwrap();
            *lock = None;
        }
    }
}
