use nonmax::NonMaxUsize;

#[derive(Debug, Clone)]
struct SparseContainer<V> {
    sparse_index: NonMaxUsize,
    data: V,
}

#[derive(Debug, Clone)]
pub struct SparseSet<V> {
    dense: Vec<SparseContainer<V>>,
    sparse: Vec<Option<NonMaxUsize>>,
}

impl<V> SparseSet<V> {
    pub fn new() -> Self {
        Self {
            dense: Vec::new(),
            sparse: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn insert(&mut self, index: NonMaxUsize, value: V) -> Option<V> {
        match self.sparse.get(index.get()) {
            Some(Some(dense)) => {
                // Replace the existing index and return the old one
                let removed = std::mem::replace(&mut self.dense[dense.get()].data, value);
                Some(removed)
            }
            _ => {
                // Insert a new index
                self.dense.push(SparseContainer {
                    sparse_index: index,
                    data: value,
                });

                // Check if sparse array is large enough
                if index.get() >= self.sparse.len() {
                    // Try to double size or set it to `index` if doubling is not enough.
                    let new_size = (2 * self.sparse.len()).clamp(index.get() + 1, usize::MAX);
                    self.sparse.resize(new_size, None);
                }

                self.sparse[index.get()] = NonMaxUsize::new(self.dense.len() - 1);

                None
            }
        }
    }

    pub fn remove(&mut self, index: NonMaxUsize) -> Option<V> {
        let dense_idx = match self.sparse.get(index.get()) {
            Some(Some(v)) => *v,
            _ => return None,
        };

        self.sparse[index.get()] = None;
        let removed = self.dense.swap_remove(dense_idx.get());

        if !self.dense.is_empty() {
            let sparse_index = self.dense[dense_idx.get()].sparse_index.get();
            self.sparse[sparse_index] = Some(dense_idx);
        }

        Some(removed.data)
    }

    pub fn contains(&self, index: NonMaxUsize) -> bool {
        match self.sparse.get(index.get()) {
            Some(Some(_)) => true,
            _ => false,
        }
    }

    pub fn get(&self, index: NonMaxUsize) -> Option<&V> {
        let dense_idx = match self.sparse.get(index.get()) {
            Some(Some(v)) => *v,
            _ => return None,
        };

        Some(&self.dense[dense_idx.get()].data)
    }

    pub fn get_mut(&mut self, index: NonMaxUsize) -> Option<&mut V> {
        let dense_idx = match self.sparse.get(index.get()) {
            Some(Some(v)) => *v,
            _ => return None,
        };

        Some(&mut self.dense[dense_idx.get()].data)
    }
}

impl<V> Default for SparseSet<V> {
    fn default() -> Self {
        Self::new()
    }
}
