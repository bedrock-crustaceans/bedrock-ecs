use std::cell::UnsafeCell;

use smallvec::SmallVec;

#[cfg(debug_assertions)]
use crate::util::debug::BorrowEnforcer;

const INLINE_COUNT: usize = 4;

#[derive(Default)]
pub struct PartitionedSignature {
    #[cfg(debug_assertions)]
    enforcers: Vec<BorrowEnforcer>,
    partitions: Vec<UnsafeCell<u64>>,
}

impl PartitionedSignature {
    pub const PARTITION_SIZE: usize = 64;

    pub fn new() -> PartitionedSignature {
        Self::default()
    }

    pub fn with_capacity(cap: usize) -> PartitionedSignature {
        Self {
            #[cfg(debug_assertions)]
            enforcers: Vec::with_capacity(cap),
            partitions: Vec::with_capacity(cap),
        }
    }

    pub fn words_count(&self) -> usize {
        self.partitions.len()
    }

    #[inline]
    pub fn resize(&mut self, bits: usize) {
        let words = bits.div_ceil(64);

        #[cfg(debug_assertions)]
        self.enforcers
            .resize_with(self.enforcers.len() + bits, BorrowEnforcer::new);
        self.partitions
            .resize_with(self.partitions.len() + words, UnsafeCell::default);
    }

    /// # Safety
    ///
    /// This is only safe to call if no other threads access the `u64` block containing `index` at the same
    /// time as this call.
    ///
    /// # Panics
    ///
    /// This method panics if the index is out of range.
    pub unsafe fn set(&self, index: usize) {
        let word = index / 64;
        let bit = index % 64;

        tracing::error!("{word} {}", self.partitions.len());

        #[cfg(debug_assertions)]
        let _guard = self.enforcers[word].write();

        let cell = &self.partitions[word];
        unsafe { *cell.get() |= 1 << bit };
    }
}

/// A set of bits, similar to `Vec<bool>` but more efficient with memory.
#[derive(Debug, Clone, Default, Hash, PartialEq, Eq)]
pub struct Signature {
    words: SmallVec<[u64; INLINE_COUNT]>,
}

impl Signature {
    /// Creates a new, empty bitset.
    pub fn new() -> Signature {
        Signature::default()
    }

    /// Creates a new signature with enough capacity to hold `cap` bits.
    pub fn with_capacity(cap: usize) -> Signature {
        Signature {
            words: SmallVec::with_capacity(cap.div_ceil(64)),
        }
    }

    /// Whether this bitset is empty.
    ///
    /// Empty can mean that it either has no words or all bits are set to 0.
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }

    /// Sets a bit to 1. If this bit is outside of the range of the signature, it will be resized.
    pub fn set(&mut self, index: usize) {
        let word = index / 64;
        if word >= self.words.len() {
            self.words.resize(word + 1, 0);
        }
        let bit = index % 64;
        self.words[word] |= 1 << bit;
    }

    /// Sets a bit to 0. If this index is out of the current range of the signature, no operation will be performed.
    /// Any non-existent bits will automatically be set to 0 on creation.
    pub fn unset(&mut self, index: usize) {
        let word = index / 64;
        if word >= self.words.len() {
            // If this
            return;
        }
        let bit = index % 64;
        self.words[word] &= !(1 << bit);
    }

    /// Counts the amount of components in this signature.
    pub fn count_ones(&self) -> u32 {
        self.words.iter().map(|w| w.count_ones()).sum()
    }

    // Whether `other` is a subset of `self`. This is faster than intersecting and then comparing
    // because this method short-circuits.
    pub fn contains(&self, other: &Self) -> bool {
        self.words
            .iter()
            .zip(other.words.iter())
            .all(|(a, b)| a & b == *b)
    }

    /// Whether `self` and `other` are disjoint.
    /// I.e. if `self` contains component A then `other` does not and vice versa.
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.words
            .iter()
            .zip(other.words.iter())
            .all(|(a, b)| a & b == 0)
    }
}
