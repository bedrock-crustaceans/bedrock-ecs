use std::{alloc::{Layout}};

pub fn repeat_layout(layout: Layout, n: usize) -> Layout {
    let size = layout.size();
    let align = layout.align();

    let padded = (size + align - 1) & !(align - 1);
    let total = padded.checked_mul(n).expect("Array capacity has overflowed");

    Layout::from_size_align(total, align).expect("Invalid array layout")
}

#[cfg(debug_assertions)]
pub mod debug {
    use std::sync::atomic::{AtomicU8, Ordering};

    const UNLOCKED: u8 = 0;
    const READ: u8 = 1;
    const WRITE: u8 = 2;

    pub struct RwGuard<'a, const WRITE: bool>(&'a RwFlag);

    impl<'a, const WRITE: bool> Drop for RwGuard<'a, WRITE> {
        fn drop(&mut self) {
            self.0.unlock_guardless();
        }
    }

    #[derive(Debug, Default)]
    pub struct RwFlag {
        state: AtomicU8
    }

    impl RwFlag {
        pub fn new() -> RwFlag {
            RwFlag {
                state: AtomicU8::new(UNLOCKED)
            }
        }

        pub fn state(&self) -> u8 {
            self.state.load(Ordering::SeqCst)
        }

        pub fn read(&self) -> RwGuard<'_, false> {
            let prev = self.state.fetch_max(1, Ordering::SeqCst);
            assert_eq!(prev, UNLOCKED, "RwFlag was write, cannot read");
            // tracing::info!("Lock guard read");
            
            RwGuard(self)
        }

        pub fn write(&self) -> RwGuard<'_, true> {
            let prev = self.state.fetch_add(2, Ordering::SeqCst);
            assert_eq!(prev, UNLOCKED, "RwFlag was not unlocked, cannot write");
            // tracing::info!("Lock guard write");

            RwGuard(self)
        }

        pub fn read_guardless(&self) {
            let prev = self.state.fetch_max(1, Ordering::SeqCst);
            assert_eq!(prev, UNLOCKED, "RwFlag was write, cannot read");
            // tracing::info!("Lock guardless read");
        }

        pub fn write_guardless(&self) {
            let prev = self.state.fetch_add(2, Ordering::SeqCst);
            assert_eq!(prev, UNLOCKED, "RwFlag was not unlocked, cannot write");
            // tracing::info!("Lock guardless write");
        }

        pub fn unlock_guardless(&self) {
            let prev = self.state.fetch_min(0, Ordering::SeqCst);
            assert_ne!(prev, UNLOCKED, "Cannot unlock RwFlag twice");
            // tracing::info!("Unlock");
        }
    }
}