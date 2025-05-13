use std::ops::{Index, IndexMut};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct IndexedMap<T>(Vec<Option<T>>);

impl<T> IndexedMap<T> {
    pub(crate) fn new_with_first(start: T) -> Self {
        IndexedMap(vec![Some(start)])
    }

    pub(crate) fn new() -> Self {
        IndexedMap(Vec::new())
    }

    pub(crate) fn set(&mut self, ind: usize, value: T) {
        if self.0.len() <= ind {
            self.0.resize_with(ind + 1, Default::default);
        }
        self.set_unchecked(ind, value);
    }

    fn set_unchecked(&mut self, ind: usize, value: T) {
        self.0[ind] = Some(value);
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter().filter_map(|v| v.as_ref())
    }

    pub(crate) fn enumerate(&self) -> impl Iterator<Item = (usize, &T)> {
        self.0
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.as_ref().map(|v| (i, v)))
    }

    pub(crate) fn get(&self, ind: usize) -> Option<&T> {
        self.0.get(ind).and_then(|v| v.as_ref())
    }

    pub(crate) fn get_mut(&mut self, ind: usize) -> Option<&mut T> {
        self.0.get_mut(ind).and_then(|v| v.as_mut())
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut().filter_map(|v| v.as_mut())
    }

    pub(crate) fn retain<F: FnMut(&T) -> bool>(&mut self, mut f: F) {
        for t in &mut self.0 {
            if let Some(v) = t {
                if !f(v) {
                    *t = None;
                }
            }
        }
        let count = self.0.iter().rev().take_while(|e| e.is_none()).count();
        self.0.truncate(self.0.len() - count);
    }
}

// Unchecked indexing
impl<T> Index<usize> for IndexedMap<T> {
    type Output = T;
    fn index(&self, i: usize) -> &T {
        self.0[i].as_ref().unwrap()
    }
}

impl<T> IndexMut<usize> for IndexedMap<T> {
    fn index_mut(&mut self, i: usize) -> &mut T {
        self.0[i].as_mut().unwrap()
    }
}
