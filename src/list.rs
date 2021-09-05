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
}
