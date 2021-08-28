use std::cell::RefCell;
use std::iter::FromIterator;
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;

use itertools::Itertools;

use crate::shared::ArcConstructor;
// use crate::shared::BoxConstructor;
use crate::shared::RcConstructor;
use crate::shared::SmartPointer;
use crate::shared::SmartPointerConstructor;

const CAPACITY: usize = 64;

/*

[1, 2, 3, 4, 5, 6, 7, 8] -> [9, 10, 11, 12]

*/

// pub enum UnrolledList<T: Clone, S: RefCountedConstructor<UnrolledCell<T, S>>> {
//     Cons(UnrolledCell<T, S>),
//     Nil,
// }

#[derive(Clone)]
pub struct UnrolledList<
    T: Clone,
    C: SmartPointerConstructor<Vec<T>>,
    S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
>(S::RC);

impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > UnrolledList<T, C, S>
{
    // Should be O(1) always
    pub fn car(&self) -> Option<T> {
        self.0.car().cloned()
    }

    // Should be O(1) always
    // Should also not have to clone
    pub fn cdr(&self) -> Option<UnrolledList<T, C, S>> {
        self.0.cdr()
    }

    fn elements(&self) -> &[T] {
        &self.0.elements
    }

    fn at_capacity(&self) -> bool {
        self.0.elements.len() == CAPACITY
    }

    // fn get_elements_mut(&mut self) -> &mut Vec<T> {
    //     let mut inner = S::make_mut(&mut self.0);
    //     C::make_mut(&mut inner.elements)
    // }

    fn update_tail_with_other_list(
        &mut self,
        // cur: &mut UnrolledList<T, C, S>,
        mut other: UnrolledList<T, C, S>,
    ) {
        // This doesn't work unless
        debug_assert!(
            self.0.cdr.is_none(),
            "Cannot update tail when active tail exists still!"
        );

        // If we're at capacity, just set the pointer to the next one
        if self.at_capacity() {
            // println!("At capacity, point to next value");
            S::make_mut(&mut self.0).cdr = Some(other);
        } else {
            let left_inner = S::make_mut(&mut self.0);
            let right_inner = S::make_mut(&mut other.0);

            let left_vector = C::make_mut(&mut left_inner.elements);
            let right_vector = C::make_mut(&mut right_inner.elements);

            // Fast path
            // [1, 2, 3, 4, 5] + [6, 7, 8, 9, 10]
            // internally, this is represented as:
            // [5, 4, 3, 2, 1]  [10, 9, 8, 7, 6]
            // iteration goes from back to front
            // so it goes 1 -> 2 -> 3 -> 4 -> 5 ... 6 -> 7 -> 8 -> 9 -> 10
            // So I need to take the vector from the right one [10, 9, 8, 7, 6]
            // And append to that the left vector, replace it in the left one
            if right_vector.len() + left_vector.len() < CAPACITY {
                right_vector.append(left_vector);

                // Swap the locations now after we've done the update
                std::mem::swap(left_vector, right_vector);
                // Adjust the indices accordingly
                left_inner.index = left_vector.len();
                right_inner.index = 0;
            } else {
                // This is the case where there is still space in the left vector,
                // but there are too many elements to move over in the right vector
                // With a capacity of 5:
                // [1, 2, 3] + [4, 5, 6, 7, 8]
                // We want the result to look like:
                // [1, 2, 3, 4, 5] -> [6, 7, 8]
                // Internally, this is represented as:
                // [3, 2, 1] -> [8, 7, 6, 5, 4]
                // And we would like the end result to be
                // [5, 4, 3, 2, 1] -> [8, 7, 6]
                // One way we could accomplish this is to
                // pop off [5, 4] as a vector
                // append [3, 2, 1] to it
                // and then assign it to the left value

                // Find how many spots are remaining in the left vector
                let space_remaining = CAPACITY - left_vector.len();
                // Chop off what will now be the start of our left vector
                let mut new_tail = right_vector.split_off(right_vector.len() - space_remaining);
                // Rearrange accordingly
                new_tail.append(left_vector);
                // Make the left node point to the correct spot
                std::mem::swap(left_vector, &mut new_tail);

                left_inner.index = CAPACITY;
                right_inner.index = right_vector.len();

                // Update this to now point to the other node
                left_inner.cdr = Some(other);
            }
        }
    }

    // Will be O(m) where m = n / 64
    // Not log(n) by any stretch, but for small list implementations, saves us some time
    pub fn append(&mut self, other: UnrolledList<T, C, S>) {
        // Steps -> go to the last pointer
        // Copy on write update the cdr to point to the new one
        // TODO - make this fill in the vector that we're using rather than just update the value if possible
        // BUG - _have_ to fill up the existing vector first, otherwise its possible to goof things up
        // that way we can use up the capacity that already exists in that node
        match self.0.cdr() {
            Some(mut cur) => {
                while let Some(next) = cur.0.cdr() {
                    cur = next;
                }
                cur.update_tail_with_other_list(other);
            }
            None => {
                self.update_tail_with_other_list(other);
            }
        }

        // todo!()
    }

    pub fn is_empty(&self) -> bool {
        self.0.elements.is_empty()
    }

    fn index(&self) -> usize {
        self.0.index
    }

    fn cons_mut(&mut self, value: T) {
        // self.0.cons_mut(value)

        todo!()
    }
}

#[derive(Clone)]
pub struct UnrolledCell<
    T: Clone,
    S: SmartPointerConstructor<Self>,
    C: SmartPointerConstructor<Vec<T>>,
> {
    pub index: usize,
    // Consider wrapping the vec in either an Rc or Arc
    // Then on clone, do the whole copy on write nonsense
    pub elements: C::RC,
    pub cdr: Option<UnrolledList<T, C, S>>,
}

// impl<T: Clone, S: SmartPointerConstructor<Self>> Deref for UnrolledCell<T, S> {
//     type Target = [T];

//     fn deref(&self) -> &Self::Target {
//         todo!
//     }
// }

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

impl<
        T: Clone + std::fmt::Debug,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > std::fmt::Debug for UnrolledList<T, C, S>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

impl<T: Clone, S: SmartPointerConstructor<Self>, C: SmartPointerConstructor<Vec<T>>>
    UnrolledCell<T, S, C>
{
    pub fn new() -> Self {
        UnrolledCell {
            index: 0,
            elements: C::RC::new(Vec::new()),
            cdr: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn car(&self) -> Option<&T> {
        self.elements.get(self.index - 1)
    }

    pub fn cdr(&self) -> Option<UnrolledList<T, C, S>> {
        if self.index < self.elements.len() {
            Some(UnrolledList(S::RC::new(self.advance_cursor())))
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
        println!("Strong count: {}", C::RC::strong_count(&self.elements));
        // println!("Elements: {:?}", self.elements);

        C::make_mut(&mut self.elements)
            // .expect("More than one reference in cons_mut")
            .push(value);
        // self.elements.push(value);
        self.index += 1;
    }

    pub fn cons_empty(value: T) -> Self {
        UnrolledCell {
            index: 0,
            elements: C::RC::new(vec![value]),
            cdr: None,
        }
    }

    pub fn cons_raw(value: T, mut cdr: UnrolledList<T, C, S>) -> UnrolledList<T, C, S> {
        if cdr.elements().len() > CAPACITY - 1 {
            UnrolledList(S::RC::new(UnrolledCell {
                index: 1,
                elements: C::RC::new(vec![value]),
                cdr: Some(cdr),
            }))
        } else {
            cdr.cons_mut(value);
            cdr
        }
    }

    // Spill over the values to a new node
    // otherwise, copy the node and spill over
    pub fn cons(value: T, mut cdr: UnrolledList<T, C, S>) -> UnrolledList<T, C, S> {
        if cdr.elements().len() > CAPACITY - 1 {
            UnrolledList(S::RC::new(UnrolledCell {
                index: 1,
                elements: C::RC::new(vec![value]),
                cdr: Some(cdr),
            }))
        } else {
            // let mut new = S::unwrap(&cdr);

            // let mut inner = S::RC::get_mut(&mut cdr).expect("Testing this should work");

            // cdr.0

            let mut inner = S::make_mut(&mut cdr.0);

            let elements = C::make_mut(&mut inner.elements);

            // let elements = C::RC::get_mut(&mut cdr.0).expect("More than one reference in cons");

            inner.index += 1;
            elements.push(value);

            todo!()

            // new.cons_mut(value);
            // new
        }
    }
}

pub struct Iter<
    T: Clone,
    C: SmartPointerConstructor<Vec<T>>,
    S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
> {
    cur: Option<UnrolledList<T, C, S>>,
    index: usize,
    _inner: PhantomData<T>,
}

impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > Iterator for Iter<T, C, S>
{
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(_self) = &self.cur {
            if self.index > 0 {
                let return_value = _self.elements().get(self.index - 1).cloned();
                self.index -= 1;
                return_value
            } else {
                self.cur = _self.0.cdr.clone();
                self.index = self.cur.as_ref().map(|x| x.elements().len()).unwrap_or(0);
                let ret = self.cur.as_ref().and_then(|x| x.car());
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
impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > IntoIterator for UnrolledList<T, C, S>
{
    type Item = T;
    type IntoIter = Iter<Self::Item, C, S>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            index: self.0.index,
            cur: Some(self),
            _inner: PhantomData,
        }
    }
}

// and we'll implement IntoIterator
impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > IntoIterator for &UnrolledList<T, C, S>
{
    type Item = T;
    type IntoIter = Iter<Self::Item, C, S>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            index: self.0.index,
            cur: Some(self.clone()),
            _inner: PhantomData,
        }
    }
}

// and we'll implement FromIterator
impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > FromIterator<T> for UnrolledList<T, C, S>
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut pairs: Vec<UnrolledList<_, _, _>> = iter
            .into_iter()
            .chunks(CAPACITY)
            .into_iter()
            .map(|x| {
                let mut elements: Vec<_> = x.collect();
                elements.reverse();
                UnrolledList(S::RC::new(UnrolledCell {
                    index: elements.len(),
                    elements: C::RC::new(elements),
                    cdr: None,
                }))
            })
            .collect();

        let mut rev_iter = (0..pairs.len()).into_iter().rev();
        rev_iter.next();

        for i in rev_iter {
            let prev = pairs.pop().unwrap();

            if let Some(UnrolledList(cell)) = pairs.get_mut(i) {
                // todo!()
                S::RC::get_mut(cell)
                    .expect("Only one owner allowed in construction")
                    .cdr = Some(prev);
            } else {
                unreachable!()
            }

            // if let Some(UnrolledCell { cdr, .. }) = pairs.get_mut(i) {
            //     *cdr = Some(S::RC::new(prev))
            // } else {
            //     unreachable!()
            // }
        }

        pairs.pop().unwrap()
    }
}

// and we'll implement FromIterator
// impl<T: Clone, S: SmartPointerConstructor<Self>> FromIterator<T> for UnrolledCell<T, S> {
//     fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
//         let iter = iter.into_iter().chunks(CAPACITY);

//         let mut cell_iter = iter.into_iter().map(|x| {
//             let mut elements: Vec<_> = x.collect();
//             elements.reverse();
//             UnrolledCell {
//                 index: elements.len(),
//                 elements,
//                 cdr: None,
//             }
//         });

//         let mut head = cell_iter.next().expect("head missing");
//         let mut next = cell_iter.next().map(S::RC::new);

//         head.cdr = next.as_ref().map(S::RC::clone);

//         for cell in cell_iter {
//             let wrapped_cell = Some(S::RC::new(cell));

//             if let Some(inner) = &mut next {
//                 let inner_value =
//                     S::RC::get_mut_unchecked(inner).expect("Pointer should not be null");
//                 inner_value.cdr = wrapped_cell.clone();
//             }
//             next = wrapped_cell;
//         }

//         head
//     }
// }

// pub type List<T> = UnrolledCell<T, RcConstructor, RcConstructor>;
// pub type ArcList<T> = UnrolledCell<T, ArcConstructor, RcConstructor>;

pub type RcList<T> = UnrolledList<T, RcConstructor, RcConstructor>;
pub type ArcList<T> = UnrolledList<T, ArcConstructor, ArcConstructor>;

// pub type BoxList<T> = UnrolledCell<T, BoxConstructor>;

#[cfg(test)]
mod tests {

    use super::*;
    // use std::rc::Rc;

    #[test]
    fn basic_iteration() {
        let list: RcList<_> = (0..100usize).into_iter().collect();
        let vec: Vec<_> = (0..100usize).into_iter().collect();

        for item in list.clone() {
            println!("ITERATING: {}", item);
        }

        for (left, right) in list.into_iter().zip(vec.into_iter()) {
            assert_eq!(left, right)
        }
    }

    // #[test]
    // fn consing() {
    //     let list = RcList::cons()

    //     println!("list elements: {:?}", list.elements);

    //     for item in list {
    //         println!("{}", item);
    //     }
    // }

    #[test]
    fn small() {
        let list: RcList<_> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9].into_iter().collect();

        println!("list elements: {:?}", list.0.elements);

        println!("list: {:?}", list);

        for item in list {
            println!("ITERATING: {}", item);
        }
    }

    #[test]
    fn append() {
        let mut left: RcList<_> = vec![1, 2, 3, 4, 5].into_iter().collect();
        let right: RcList<_> = vec![6, 7, 8, 9, 10].into_iter().collect();

        println!("Left node elements pre append: {:?}", left.elements());
        println!("Left node next: {:?}", left.0.cdr.is_some());
        left.append(right.clone());
        println!("Left node elements post append: {:?}", left.elements());
        println!("Left node next post: {:?}", left.0.cdr.is_some());

        println!("New appended list");
        for item in left {
            println!("Iterating: {}", item);
        }

        println!("Old list");
        for item in right {
            println!("Iterating: {}", item)
        }
    }

    #[test]
    fn append_large() {
        let mut left: RcList<_> = (0..60).into_iter().collect();
        let right: RcList<_> = (60..100).into_iter().collect();

        left.append(right);

        println!("left index: {}", left.index());
        println!("left next elements: {:?}", left.cdr().unwrap().elements());

        println!("{:?}", left);

        for item in left {
            println!("iterating: {}", item);
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
