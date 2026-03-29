use std::cell::UnsafeCell;

use smallvec::SmallVec;

#[cfg(debug_assertions)]
use crate::util::debug::BorrowEnforcer;

const INLINE_COUNT: usize = 4;

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

    /// Adds `other` to `Self`.
    pub fn union(&mut self, other: &Self) {
        let max_len = self.words.len().clamp(other.words.len(), usize::MAX);
        if self.words.len() < max_len {
            self.words.resize(max_len, 0);
        }

        self.words
            .iter_mut()
            .zip(other.words.iter())
            .for_each(|(a, b)| *a |= b);
    }

    /// Removes the bits of `other` from `Self`.
    pub fn remove(&mut self, other: &Self) {
        self.words
            .iter_mut()
            .zip(other.words.iter())
            .for_each(|(a, b)| *a &= !b);
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
