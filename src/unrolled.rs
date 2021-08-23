use std::iter::FromIterator;
use std::marker::PhantomData;

use itertools::Itertools;

use crate::shared::ArcConstructor;
// use crate::shared::BoxConstructor;
use crate::shared::RcConstructor;
use crate::shared::SmartPointer;
use crate::shared::SmartPointerConstructor;

const CAPACITY: usize = 8;

/*

[1, 2, 3, 4, 5, 6, 7, 8] -> [9, 10, 11, 12]

*/

// pub enum UnrolledList<T: Clone, S: RefCountedConstructor<UnrolledCell<T, S>>> {
//     Cons(UnrolledCell<T, S>),
//     Nil,
// }

#[derive(Hash, Clone)]
pub struct UnrolledCell<T: Clone, S: SmartPointerConstructor<Self>> {
    pub index: usize,
    pub elements: Vec<T>,
    pub cdr: Option<S::RC>,
}

// impl<T: Clone + std::fmt::Debug, S: RefCountedConstructor<Self>> std::fmt::Debug
//     for UnrolledCell<T, S>
// {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("UnrolledCell")
//             .field("index", &self.index)
//             .field("elements", &self.elements)
//             // .field("cdr", &S::fmt(&self.cdr, f))
//             .finish()
//     }
// }

impl<T: Clone + std::fmt::Debug, S: SmartPointerConstructor<Self>> std::fmt::Debug
    for UnrolledCell<T, S>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

impl<T: Clone, S: SmartPointerConstructor<Self>> UnrolledCell<T, S> {
    pub fn new() -> Self {
        UnrolledCell {
            index: 0,
            elements: Vec::new(),
            cdr: None,
        }
    }

    pub fn car(&self) -> Option<&T> {
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
        }
    }
}

pub struct Iter<T: Clone, S: SmartPointerConstructor<UnrolledCell<T, S>>> {
    cur: Option<S::RC>,
    index: usize,
    _inner: PhantomData<T>,
}

impl<T: Clone, S: SmartPointerConstructor<UnrolledCell<T, S>>> Iterator for Iter<T, S> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(_self) = &self.cur {
            if self.index > 0 {
                let return_value = _self.elements.get(self.index - 1).cloned();
                self.index -= 1;
                return_value
            } else {
                self.cur = _self.cdr.clone();
                self.index = self.cur.as_ref().map(|x| x.elements.len()).unwrap_or(0);
                let ret = self.cur.as_ref().and_then(|x| x.car().cloned());
                if ret.is_some() {
                    self.index -= 1;
                }
                ret
            }
        } else {
            None
        }
    }
}

// and we'll implement IntoIterator
impl<T: Clone, S: SmartPointerConstructor<Self>> IntoIterator for UnrolledCell<T, S> {
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

// and we'll implement IntoIterator for references
// TODO
impl<T: Clone, S: SmartPointerConstructor<UnrolledCell<T, S>>> IntoIterator
    for &UnrolledCell<T, S>
{
    type Item = T;
    type IntoIter = Iter<Self::Item, S>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            index: self.index,
            cur: Some(S::RC::new(self.clone())), // TODO
            _inner: PhantomData,
        }
    }
}

// and we'll implement FromIterator
// impl<T: Clone, S: SmartPointerConstructor<Self>> FromIterator<T> for UnrolledCell<T, S> {
//     fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
//         let mut pairs: Vec<UnrolledCell<T, S>> = iter
//             .into_iter()
//             .chunks(CAPACITY)
//             .into_iter()
//             .map(|x| {
//                 let mut elements: Vec<_> = x.collect();
//                 elements.reverse();
//                 UnrolledCell {
//                     index: elements.len(),
//                     elements,
//                     cdr: None,
//                 }
//             })
//             .collect();

//         let mut rev_iter = (0..pairs.len()).into_iter().rev();
//         rev_iter.next();

//         for i in rev_iter {
//             let prev = pairs.pop().unwrap();
//             if let Some(UnrolledCell { cdr, .. }) = pairs.get_mut(i) {
//                 *cdr = Some(S::RC::new(prev))
//             } else {
//                 unreachable!()
//             }
//         }

//         pairs.pop().unwrap()
//     }
// }

impl<T: Clone, S: SmartPointerConstructor<Self>> FromIterator<T> for UnrolledCell<T, S> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let chunks_iter = iter.into_iter().chunks(CAPACITY);

        let mut iter = chunks_iter.into_iter().map(|x| {
            let mut elements: Vec<_> = x.collect();
            elements.reverse();
            UnrolledCell {
                index: elements.len(),
                elements,
                cdr: None,
            }
        });

        // next pointer to go to
        let mut next;

        // Keep track of the last two pairs inside the iterator
        // [ 1 2 3 4 5 6 7 8 9 10 ]
        //  ^^^^
        //    ^^^^
        //      ^^^^
        // let mut keep_alive: (Option<_>, Option<_>) = (None, None);

        // Get the head
        // [1  2  3  4  5]
        // ^^
        let mut head = iter.next().expect("Missing cell in UnrolledCell");
        // let ret_val = head.clone();

        // Get the next value
        // [1  2  3  4  5]
        //    ^^
        // let current =;
        // let mut next = iter.next().map(S::RC::new);
        head.cdr = iter.next().map(S::RC::new);

        let mut current = &mut head.cdr;

        for cell in iter {
            next = Some(S::RC::new(cell));

            if let Some(mut inner) = head.cdr {
                let cur_mut = S::RC::get_mut(&mut inner).expect("Should only have one reference");
                cur_mut.cdr = next.clone();
                head = next;
            } else {
                break;
            }

            // current.cdr = next.clone();

            // keep_alive.0 = Some(current.clone());
            // keep_alive.1 = next.clone();
        }

        // ret_val

        unimplemented!()

        // let mut rev_iter = (0..pairs.len()).into_iter().rev();
        // rev_iter.next();

        // for i in rev_iter {
        //     let prev = pairs.pop().unwrap();
        //     if let Some(UnrolledCell { cdr, .. }) = pairs.get_mut(i) {
        //         *cdr = Some(S::RC::new(prev))
        //     } else {
        //         unreachable!()
        //     }
        // }

        // pairs.pop().unwrap()
    }
}

pub type List<T> = UnrolledCell<T, RcConstructor>;
pub type ArcList<T> = UnrolledCell<T, ArcConstructor>;
// pub type BoxList<T> = UnrolledCell<T, BoxConstructor>;

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

        println!("list: {:?}", list);

        for item in list {
            println!("ITERATING: {}", item);
        }
    }

    // #[test]
    // fn boxing() {
    //     let list: BoxList<_> = vec![1, 2, 3, 4, 5].into_iter().collect();
    //     for item in list {
    //         println!("ITERATING: {}", item);
    //     }
    // }
}
