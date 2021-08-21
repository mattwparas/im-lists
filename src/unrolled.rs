use std::iter::FromIterator;
use std::marker::PhantomData;

use itertools::Itertools;

use crate::shared::RcConstructor;
use crate::shared::RefCounted;
use crate::shared::RefCountedConstructor;

const CAPACITY: usize = 8;

/*

[1, 2, 3, 4, 5, 6, 7, 8] -> [9, 10, 11, 12]

*/

pub enum UnrolledList<T: Clone, S: RefCountedConstructor<UnrolledCell<T, S>>> {
    Cons(UnrolledCell<T, S>),
    Nil,
}

#[derive(Hash, Debug, Clone)]
pub struct UnrolledCell<T: Clone, S: RefCountedConstructor<Self>> {
    pub index: usize,
    pub elements: Vec<T>,
    pub cdr: Option<S::RC>,
}

impl<T: Clone, S: RefCountedConstructor<Self>> UnrolledCell<T, S> {
    pub fn new() -> Self {
        UnrolledCell {
            index: 0,
            elements: Vec::new(),
            cdr: None,
        }
    }

    pub fn car(&self) -> Option<&T> {
        self.elements.get(self.index)
    }

    pub fn cdr(&self) -> Option<S::RC> {
        if self.index < self.elements.len() {
            Some(S::RC::new(self.advance_cursor()))
        } else {
            self.cdr.clone()
        }
    }

    fn advance_cursor(&self) -> Self {
        UnrolledCell {
            index: self.index + 1,
            elements: self.elements.clone(),
            cdr: self.cdr.clone(),
        }
    }

    pub fn cons_mut(&mut self, value: T) {
        self.elements.push(value);
        self.index += 1;
    }

    pub fn cons_empty(value: T) -> Self {
        UnrolledCell {
            index: 0,
            elements: vec![value],
            cdr: None,
        }
    }

    // Spill over the values to a new node
    // otherwise, copy the node and spill over
    pub fn cons(value: T, cdr: S::RC) -> Self {
        if cdr.elements.len() > CAPACITY {
            UnrolledCell {
                index: 0,
                elements: vec![value],
                cdr: None,
            }
        } else {
            let mut new = S::unwrap(&cdr);
            new.cons_mut(value);
            new

            // let mut other = cdr.unwrap();

            // other.clone()
        }
    }
}

pub struct Iter<T: Clone, S: RefCountedConstructor<UnrolledCell<T, S>>> {
    cur: Option<S::RC>,
    index: usize,
    _inner: PhantomData<T>,
}

impl<T: Clone, S: RefCountedConstructor<UnrolledCell<T, S>>> Iterator for Iter<T, S> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(_self) = &self.cur {
            dbg!(self.index);
            if self.index > 0 {
                let return_value = _self.elements.get(self.index).cloned();
                self.index -= 1;
                return_value
            } else {
                self.cur = _self.cdr.clone();
                self.index = self.cur.as_ref().map(|x| x.index).unwrap_or(0);
                self.cur.as_ref().and_then(|x| x.car().cloned())
            }
        } else {
            None
        }
    }
}

// and we'll implement IntoIterator
impl<T: Clone, S: RefCountedConstructor<Self>> IntoIterator for UnrolledCell<T, S> {
    type Item = T;
    type IntoIter = Iter<Self::Item, S>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            index: self.index,
            cur: Some(S::RC::new(self)),
            _inner: PhantomData,
        }
    }
}

// and we'll implement FromIterator
impl<T: Clone, S: RefCountedConstructor<Self>> FromIterator<T> for UnrolledCell<T, S> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut pairs: Vec<UnrolledCell<T, S>> = iter
            .into_iter()
            .chunks(CAPACITY)
            .into_iter()
            .map(|x| UnrolledCell {
                index: 0,
                elements: x.collect(),
                cdr: None,
            })
            .collect();

        let mut rev_iter = (0..pairs.len()).into_iter().rev();
        rev_iter.next();

        for i in rev_iter {
            let prev = pairs.pop().unwrap();
            if let Some(UnrolledCell { cdr, .. }) = pairs.get_mut(i) {
                *cdr = Some(S::RC::new(prev))
            } else {
                unreachable!()
            }
        }

        pairs.pop().unwrap()
    }
}

pub type List<T> = UnrolledCell<T, RcConstructor>;

#[cfg(test)]
mod tests {

    use super::*;
    use std::rc::Rc;

    #[test]
    fn basic_iteration() {
        let list: List<_> = (0..100).into_iter().collect();

        println!("Running test!");

        for item in list {
            println!("{}", item);
        }
    }

    #[test]
    fn consing() {
        let list: List<usize> = List::cons(
            1,
            Rc::new(List::cons(
                2,
                Rc::new(List::cons(
                    3,
                    Rc::new(List::cons(
                        4,
                        Rc::new(List::cons(
                            5,
                            Rc::new(List::cons(
                                6,
                                Rc::new(List::cons(
                                    7,
                                    Rc::new(List::cons(8, Rc::new(List::cons_empty(9)))),
                                )),
                            )),
                        )),
                    )),
                )),
            )),
        );

        for item in list {
            println!("{}", item);
        }
    }
}
