use smallvec::SmallVec;

const INLINE_COUNT: usize = 4;

#[derive(Debug, Clone, Default, Hash, PartialEq, Eq)]
pub struct BitSet {
    bits: SmallVec<[u64; INLINE_COUNT]>
}

impl BitSet {
    pub fn new() -> BitSet {
        BitSet::default()
    }

    pub fn with_capacity(cap: usize) -> BitSet {
        BitSet { bits: SmallVec::with_capacity(cap) }
    }

    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }

    pub fn set(&mut self, index: usize) {
        let word = index / 64;
        if self.bits.len() <= word {
            self.bits.resize(word + 1, 0);
        }

        let bit = index % 64;
        self.bits[word] |= 1 << bit;
    }

    pub fn unset(&mut self, index: usize) {
        let word = index / 64;
        if self.bits.len() <= word {
            // Outside bitset, will already be 0 when accessed.
            return
        }

        let bit = index % 64;
        self.bits[word] &= !(1 << bit);
    }

    pub fn ones(&self) -> u32 {
        self.bits
            .iter()
            .map(|w| w.count_ones())
            .sum()
    }

    // Whether `other` is a subset of `self`. This is faster than intersecting and then comparing
    // because this method short-circuits.
    pub fn is_subset(&self, other: &Self) -> bool {
        self.bits
            .iter()
            .zip(other.bits.iter())
            .all(|(a, b)| {
                (a & b).eq(b)
            })
    }

    /// Finds the intersection between `self` and `other`.
    pub fn intersect(&self, other: &Self) -> Self {
        let intersect = self.bits
            .iter()
            .zip(other.bits.iter())
            .map(|(a, b)| {
                a & b
            })
            .collect::<SmallVec<[u64; INLINE_COUNT]>>();

        BitSet { bits: intersect }
    }
}

#[cfg(test)]
mod test {
    use crate::bitset::BitSet;

    #[test]
    fn test_bitset() {
        let mut set1 = BitSet::new();
        set1.set(3);

        let mut set2 = BitSet::new();
        set2.set(4);
        set2.set(3);

        let inter = set2.intersect(&set1);
        println!("{inter:?}");
    }
}