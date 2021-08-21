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

#[derive(Hash, Clone)]
pub struct UnrolledCell<T: Clone, S: RefCountedConstructor<Self>> {
    pub index: usize,
    pub elements: Vec<T>,
    pub cdr: Option<S::RC>,
}

impl<T: Clone + std::fmt::Debug, S: RefCountedConstructor<Self>> std::fmt::Debug
    for UnrolledCell<T, S>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnrolledCell")
            .field("index", &self.index)
            .field("elements", &self.elements)
            // .field("cdr", &S::fmt(&self.cdr, f))
            .finish()
    }
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
        // println!("Getting value at index: {}", self.index);
        self.elements.get(self.index - 1)
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
        if cdr.elements.len() > CAPACITY - 1 {
            UnrolledCell {
                index: 1,
                elements: vec![value],
                cdr: Some(cdr),
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
        // dbg!("calling next");
        // println!("Has cur: {}", self.cur.is_some());
        if let Some(_self) = &self.cur {
            // dbg!(self.index);
            // println!("self.index > 0: {}", self.index > 0);
            if self.index > 0 {
                // dbg!("getting return value");
                let return_value = _self.elements.get(self.index - 1).cloned();
                self.index -= 1;
                // dbg!(self.index);
                return_value
            } else {
                // println!("Has next: {}", _self.cdr.is_some());
                self.cur = _self.cdr.clone();

                // println!(
                //     "Next node: {:?}",
                //     self.cur.as_ref().map(|x| x.elements.len())
                // );

                self.index = self.cur.as_ref().map(|x| x.elements.len()).unwrap_or(0);

                // println!("Next index: {:?}", self.index);
                let ret = self.cur.as_ref().and_then(|x| {
                    // println!("getting car with element length: {}", x.elements.len());
                    x.car().cloned()
                });

                // println!("value: {}", ret.is_some());
                ret
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
            .map(|x| {
                let mut elements: Vec<_> = x.collect();
                elements.reverse();
                UnrolledCell {
                    index: elements.len(),
                    elements,
                    cdr: None,
                }
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
        let list: List<_> = (0..100usize).into_iter().collect();
        let vec: Vec<_> = (0..100usize).into_iter().collect();

        for item in list.clone() {
            println!("ITERATING: {}", item);
        }

        for (left, right) in list.into_iter().zip(vec.into_iter()) {
            assert_eq!(left, right)
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

        println!("list elements: {:?}", list.elements);

        for item in list {
            println!("{}", item);
        }
    }

    #[test]
    fn small() {
        let list: List<_> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9].into_iter().collect();

        println!("list elements: {:?}", list.elements);

        for item in list {
            println!("ITERATING: {}", item);
        }
    }
}
