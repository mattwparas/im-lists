use std::iter::FromIterator;

use crate::{shared::RcConstructor, unrolled::UnrolledList};

pub struct List<T: Clone>(UnrolledList<T, RcConstructor, RcConstructor>);

impl<T: Clone> List<T> {
    pub fn new() -> Self {
        List(UnrolledList::new())
    }

    pub fn strong_count(&self) -> usize {
        self.0.strong_count()
    }

    pub fn cell_count(&self) -> usize {
        self.0.cell_count()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn reverse(mut self) -> Self {
        self.0 = self.0.reverse();
        self
    }

    pub fn last(&self) -> Option<T> {
        self.0.last()
    }

    pub fn car(&self) -> Option<T> {
        self.0.car()
    }

    pub fn cdr(&self) -> Option<List<T>> {
        self.0.cdr().map(List)
    }

    pub fn cons(value: T, other: List<T>) -> List<T> {
        Self(UnrolledList::cons(value, other.0))
    }

    pub fn cons_mut(&mut self, value: T) {
        self.0.cons_mut(value)
    }

    pub fn push_front(&mut self, value: T) {
        self.0.push_front(value)
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a T> {
        self.0.iter()
    }

    pub fn get(&self, index: usize) -> Option<T> {
        self.0.get(index)
    }

    pub fn append(self, other: Self) -> Self {
        List(self.0.append(other.0))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn extend(self, iter: impl IntoIterator<Item = T>) -> Self {
        List(self.0.extend(iter))
    }
}

// and we'll implement FromIterator
impl<T: Clone> FromIterator<T> for List<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        List(UnrolledList::from_iter(iter))
    }
}

impl<T: Clone> FromIterator<List<T>> for List<T> {
    fn from_iter<I: IntoIterator<Item = List<T>>>(iter: I) -> Self {
        List(UnrolledList::from_iter(iter.into_iter().map(|x| x.0)))
    }
}

impl<T: Clone> From<Vec<T>> for List<T> {
    fn from(vec: Vec<T>) -> Self {
        List(vec.into_iter().collect())
    }
}

// impl<T: Clone> IntoIterator for List<T> {
//     type Item = T;
//     type IntoIter = FlatMap<
//         NodeIter<T, C, S>,
//         Rev<std::iter::Take<std::vec::IntoIter<T>>>,
//         fn(UnrolledList<T, C, S>) -> Rev<std::iter::Take<std::vec::IntoIter<T>>>,
//     >;

//     fn into_iter(self) -> Self::IntoIter {
//         self.0.into_iter()
//     }
// }

// impl<'a, T: Clone> IntoIterator for &'a List<T> {
//     type Item = &'a T;
//     type IntoIter = FlatMap<
//         NodeIterRef<'a, T, C, S>,
//         Rev<std::slice::Iter<'a, T>>,
//         fn(&'a UnrolledList<T, C, S>) -> Rev<std::slice::Iter<'a, T>>,
//     >;

//     #[inline(always)]
//     fn into_iter(self) -> Self::IntoIter {
//         self.node_iter()
//             .flat_map(|x| x.elements()[0..x.index()].into_iter().rev())
//     }
// }
