#[derive(Debug, Clone)]
struct Dense<T> {
    pub value: T,
    pub sparse_idx: usize
}

#[derive(Debug, Clone)]
pub struct SparseSet<T> {
    dense: Vec<Dense<T>>,
    sparse: Vec<usize>,
    len: usize
}

impl<T> SparseSet<T> {
    pub fn new() -> SparseSet<T> {
        SparseSet::<T>::default()
    }

    pub fn push(&mut self, value: T) -> usize {
        let dense_idx = self.len;
        self.len += 1;

        if dense_idx < self.dense.len() {
            let dense = &mut self.dense[dense_idx];
            
            dense.value = value;
            return dense.sparse_idx;
        }

        let sparse_idx = self.sparse.len();
        self.dense.push(Dense { value, sparse_idx });
        self.sparse.push(dense_idx);

        sparse_idx
    }

    pub fn remove(&mut self, index: usize) -> Option<T> {
        if !self.contains_index(index) {
            return None
        }

        self.len -= 1;

        let dense_idx = self.sparse[index];
        let removed = self.dense.swap_remove(dense_idx);
        let sparse_idx = self.dense[dense_idx].sparse_idx;
        self.sparse[sparse_idx] = index;
        
        Some(removed.value)
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if !self.contains_index(index) {
            return None
        }

        let dense_idx = self.sparse[index];
        Some(&self.dense[dense_idx].value)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if !self.contains_index(index) {
            return None
        }

        let dense_idx = self.sparse[index];
        Some(&mut self.dense[dense_idx].value)
    }

    pub fn contains_index(&self, index: usize) -> bool {
        if index >= self.sparse.len() {
            return false
        }

        let dense_idx = self.sparse[index];
        let current_idx = self.dense[dense_idx].sparse_idx;

        dense_idx < self.len && index == current_idx
    }
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        SparseSet {
            dense: Vec::new(),
            sparse: Vec::new(),
            len: 0
        }
    }
}

#[cfg(test)]
mod test {
    use crate::sparse_set::SparseSet;

    #[test]
    fn sparse_add_remove() {
        let mut set = SparseSet::new();

        set.push(1);
        set.push(2);
        set.push(3);
        set.push(4);

        tracing::info!("{set:?}");

        set.remove(2);

        assert!(set.get(2).is_none());

        tracing::info!("{set:?}");
    }
}