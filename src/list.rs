use crate::shared::{ArcConstructor, RcConstructor, RefCounted, RefCountedConstructor};
use std::{iter::FromIterator, marker::PhantomData};

#[derive(Clone, Hash, Debug)]
pub struct ConsCell<T: Clone, S: RefCountedConstructor<Self>> {
    pub car: T,
    pub cdr: Option<S::RC>,
}

impl<T: Clone, S: RefCountedConstructor<Self>> ConsCell<T, S> {
    pub fn new(car: T, cdr: Option<S::RC>) -> Self {
        ConsCell { car, cdr }
    }

    pub fn cons(car: T, cdr: S::RC) -> Self {
        ConsCell::new(car, Some(cdr))
    }

    pub fn car(&self) -> T {
        self.car.clone()
    }

    pub fn cdr(&self) -> &Option<S::RC> {
        &self.cdr
    }
}

impl<T: Clone, S: RefCountedConstructor<Self>> Drop for ConsCell<T, S> {
    // don't want to blow the stack with destructors,
    // but also don't want to walk the whole list.
    // So walk the list until we find a non-uniquely owned item
    fn drop(&mut self) {
        let mut cur = self.cdr.take();
        loop {
            match cur {
                Some(r) => match S::RC::try_unwrap(r) {
                    Some(ConsCell {
                        car: _,
                        cdr: ref mut next,
                    }) => cur = next.take(),
                    _ => return,
                },
                _ => return,
            }
        }
    }
}

pub struct Iter<T: Clone, S: RefCountedConstructor<ConsCell<T, S>>> {
    cur: Option<S::RC>,
    _inner: PhantomData<T>,
}

impl<T: Clone, S: RefCountedConstructor<ConsCell<T, S>>> Iterator for Iter<T, S> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(_self) = &self.cur {
            let ret_val = Some(_self.car());
            self.cur = _self.cdr.as_ref().map(S::RC::clone);
            ret_val
        } else {
            None
        }
    }
}

// and we'll implement IntoIterator
impl<T: Clone, S: RefCountedConstructor<Self>> IntoIterator for ConsCell<T, S> {
    type Item = T;
    type IntoIter = Iter<Self::Item, S>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            cur: Some(S::RC::new(self)),
            _inner: PhantomData,
        }
    }
}

// and we'll implement FromIterator
impl<T: Clone, S: RefCountedConstructor<Self>> FromIterator<T> for ConsCell<T, S> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut pairs: Vec<ConsCell<T, S>> = iter
            .into_iter()
            .map(|car| ConsCell::new(car, None))
            .collect();

        let mut rev_iter = (0..pairs.len()).into_iter().rev();
        rev_iter.next();

        for i in rev_iter {
            let prev = pairs.pop().unwrap();
            if let Some(ConsCell { cdr, .. }) = pairs.get_mut(i) {
                *cdr = Some(S::RC::new(prev))
            } else {
                unreachable!()
            }
        }

        pairs.pop().unwrap()
    }
}

pub type RcLinkedList<T> = ConsCell<T, RcConstructor>;
pub type ArcLinkedList<T> = ConsCell<T, ArcConstructor>;

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn basic_iteration() {
        // let list = vec![1, 2, 3, 4, 5]
        //     .into_iter()
        //     .collect::<ConsCell<i32, Rc<_>>>();

        // let list: RcLinkedList<_> = vec![1, 2, 3, 4, 5].into_iter().collect();

        // for item in list {
        //     println!("{}", item)
        // }

        // let cell: ConsCell<usize, Rc<ConsCell<usize, _>>> =
        //     ConsCell::new(10, Some(Rc::new(ConsCell::new(20, None))));

        // unimplemented!()
    }
}
