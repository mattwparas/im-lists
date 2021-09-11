//! A persistent list.
//!
//! This is a sequence of elements, akin to a cons list. The most common operation is to
//! [`cons`](crate::list::List::cons) to the front (or [`cons_mut`](crate::list::List::cons_mut))
//! The API is designed to be a drop in replacement for an immutable linked list implementation, with instead the backing
//! being an unrolled linked list.
//!
//! # Performance Notes
//!
//! Using the mutable functions when possible enables in place mutation. Much of the internal structure is shared,
//! so even immutable functions can be fast, but the mutable functions will be faster.

use std::{cmp::Ordering, iter::FromIterator};

use crate::{
    shared::RcConstructor,
    unrolled::{ConsumingWrapper, IterWrapper, UnrolledList},
};

/// A persistent list.
///
/// This list is suitable for a single threaded environment. If you would like an immutable list that can be shared
/// across threads (i.e., is [`Send`] + [`Sync`], see [`SharedList`](crate::shared_list::SharedList)).
///
/// It's implemented as an unrolled linked list, which is a single linked list which stores a variable
/// amount of elements in each node. The capacity of any individual node for now is set to to be 256 elements, which means that until more than 256 elements are cons'd onto a list, it will remain a vector under the hood.
///
/// The list is also designed to leverage in place mutations whenever possible - if the number of references pointing to either a cell containing a vector or the shared vector is one, then that mutation is done in place. Otherwise, it is copy-on-write, maintaining our persistent invariant.
///
/// ## Performance Notes
///
/// The algorithmic complexity of an unrolled linked list matches that of a normal linked list - however in practice
/// we have a (somewhat - this is more complex) constant factor of the capacity of a node that gives us practical
/// performance wins. For a list that is fully filled, iteration becomes O(n / 256), rather than the typical O(n).
/// In addition, the unrolled linked list is able to avoid the costly cache misses that a typical linked list
/// suffers from, seeing very realistic performance gains.
#[derive(Clone)]
pub struct List<T: Clone>(UnrolledList<T, RcConstructor, RcConstructor>);

impl<T: Clone> List<T> {
    /// Construct an empty list.
    pub fn new() -> Self {
        List(UnrolledList::new())
    }

    /// Get the number of strong references pointing to this list
    ///
    /// Time: O(1)
    pub fn strong_count(&self) -> usize {
        self.0.strong_count()
    }

    /// Get the length of the list
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let list = list![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    /// assert_eq!(list.len(), 10);
    /// ```
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Reverses the input list and returns a new list
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let list = list![1, 2, 3, 4, 5].reverse();
    /// assert_eq!(list, list![5, 4, 3, 2, 1])
    /// ```
    pub fn reverse(mut self) -> Self {
        self.0 = self.0.reverse();
        self
    }

    /// Get the last element of the list.
    /// Returns None if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let list = list![1, 2, 3, 4, 5];
    /// assert_eq!(list.last().cloned(), Some(5));
    /// ```
    pub fn last(&self) -> Option<&T> {
        self.0.last()
    }

    /// Get the first element of the list.
    /// Returns None if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// # use im_lists::list::List;
    /// let list = list![1, 2, 3, 4, 5];
    /// let car = list.car();
    /// assert_eq!(car, Some(1));
    ///
    /// let list: List<usize> = list![];
    /// let car = list.car();
    /// assert!(car.is_none());
    /// ```
    pub fn car(&self) -> Option<T> {
        self.0.car()
    }

    /// Returns a reference to the first element of the list.
    /// Returns None if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// # use im_lists::list::List;
    /// let list = list![1, 2, 3, 4, 5];
    /// let first = list.first().cloned();
    /// assert_eq!(first, Some(1));
    ///
    /// let list: List<usize> = list![];
    /// let first = list.first();
    /// assert!(first.is_none());
    /// ```
    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }

    /// Get the "rest" of the elements as a list, excluding the first element
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let list = list![1, 2, 3, 4, 5];
    /// let cdr = list.cdr().unwrap();
    /// assert_eq!(cdr, list![2, 3, 4, 5]);
    ///
    /// let list = list![5];
    /// let cdr = list.cdr();
    /// assert!(cdr.is_none());
    /// ```
    pub fn cdr(&self) -> Option<List<T>> {
        self.0.cdr().map(List)
    }

    /// Get the "rest" of the elements as a list.
    /// Alias for [`cdr`](crate::list::List::cdr)
    pub fn rest(&self) -> Option<List<T>> {
        self.cdr()
    }

    /// Gets the cdr of the list, mutably.
    /// Returns None if the next is empty - otherwise updates self to be the rest
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let mut list = list![1, 2, 3, 4, 5];
    /// list.cdr_mut().expect("This list has a tail");
    /// assert_eq!(list, list![2, 3, 4, 5]);
    ///
    ///
    /// let mut list = list![1, 2, 3];
    /// assert!(list.cdr_mut().is_some());
    /// assert_eq!(list, list![2, 3]);
    /// assert!(list.cdr_mut().is_some());
    /// assert_eq!(list, list![3]);
    /// assert!(list.cdr_mut().is_none());
    /// assert_eq!(list, list![]);
    /// ```
    pub fn cdr_mut(&mut self) -> Option<&mut Self> {
        match self.0.cdr_mut() {
            Some(_) => Some(self),
            None => None,
        }
    }

    /// Gets the rest of the list, mutably.
    /// Alias for [`cdr_mut`](crate::list::List::cdr_mut)
    pub fn rest_mut(&mut self) -> Option<&mut Self> {
        self.cdr_mut()
    }

    /// Pushes an element onto the front of the list, returning a new list
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list::List;
    /// let list = List::cons(1, List::cons(2, List::cons(3, List::cons(4, List::new()))));
    /// assert_eq!(list, list![1, 2, 3, 4]);
    /// ```
    pub fn cons(value: T, other: List<T>) -> List<T> {
        Self(UnrolledList::cons(value, other.0))
    }

    /// Mutably pushes an element onto the front of the list, in place
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let mut list = list![];
    /// list.cons_mut(3);
    /// list.cons_mut(2);
    /// list.cons_mut(1);
    /// list.cons_mut(0);
    /// assert_eq!(list, list![0, 1, 2, 3])
    /// ```
    pub fn cons_mut(&mut self, value: T) {
        self.0.cons_mut(value)
    }

    /// Alias for cons_mut
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let mut list = list![];
    /// list.push_front(3);
    /// list.push_front(2);
    /// list.push_front(1);
    /// list.push_front(0);
    /// assert_eq!(list, list![0, 1, 2, 3])
    /// ```
    pub fn push_front(&mut self, value: T) {
        self.0.push_front(value)
    }

    /// Mutably pop the first value off of the list
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let mut list = list![1, 2, 3];
    /// assert_eq!(list.pop_front().unwrap(), 1);
    /// assert_eq!(list.pop_front().unwrap(), 2);
    /// assert_eq!(list.pop_front().unwrap(), 3);
    /// assert!(list.pop_front().is_none())
    /// ```
    pub fn pop_front(&mut self) -> Option<T> {
        self.0.pop_front()
    }

    /// Push one value to the back of the list
    ///
    /// Time: O(n)
    ///
    /// # Examples
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let mut list = list![];
    /// list.push_back(0);
    /// list.push_back(1);
    /// list.push_back(2);
    /// list.push_back(3);
    /// assert_eq!(list, list![0, 1, 2, 3])
    /// ```
    pub fn push_back(&mut self, value: T) {
        self.0.push_back(value)
    }

    /// Construct a new list from the first `count` elements from the current list
    ///
    /// # Examples
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let list = list![0, 1, 2, 3, 4, 5];
    /// let new_list = list.take(3);
    /// assert_eq!(new_list, list![0, 1, 2]);
    /// ```
    pub fn take(&self, count: usize) -> Self {
        List(self.0.take(count))
    }

    /// Returns the list after the first `len` elements of lst.
    /// If the list has fewer then `len` elements, then this returns `None`.
    ///
    /// # Examples
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let list = list![0, 1, 2, 3, 4, 5];
    /// let new_list = list.tail(2);
    /// assert_eq!(new_list.unwrap(), list![2, 3, 4, 5]);
    ///
    /// let no_list = list.tail(100);
    /// assert!(no_list.is_none())
    /// ```
    pub fn tail(&self, len: usize) -> Option<Self> {
        self.0.tail(len).map(List)
    }

    /// Constructs an iterator over the list
    pub fn iter(&self) -> impl Iterator<Item = &'_ T> {
        self.0.iter()
    }

    /// Get a reference to the value at index `index` in a list.
    /// Returns `None` if the index is out of bounds.
    pub fn get(&self, index: usize) -> Option<&T> {
        self.0.get(index)
    }

    /// Append the list `other` to the end of the current list. Returns a new list.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let left = list![1usize, 2, 3];
    /// let right = list![4usize, 5, 6];
    /// assert_eq!(left.append(right), list![1, 2, 3, 4, 5, 6])
    /// ```
    pub fn append(self, other: Self) -> Self {
        List(self.0.append(other.0))
    }

    /// Append the list 'other' to the end of the current list in place.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let mut left = list![1usize, 2, 3];
    /// let right = list![4usize, 5, 6];
    /// left.append_mut(right);
    /// assert_eq!(left, list![1, 2, 3, 4, 5, 6])
    /// ```
    pub fn append_mut(&mut self, other: Self) {
        self.0.append_mut(other.0);
    }

    /// Checks whether a list is empty
    ///
    /// # Examples
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// # use im_lists::list::List;
    /// let mut list = List::new();
    /// assert!(list.is_empty());
    /// list.cons_mut("applesauce");
    /// assert!(!list.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Sorts the list
    ///
    /// # Examples
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let mut list = list![4, 2, 6, 3, 1, 5];
    /// list.sort();
    /// assert_eq!(list, list![1, 2, 3, 4, 5, 6]);
    /// ```
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        self.0.sort()
    }

    /// Sorts the list according to the ordering
    ///
    /// # Examples
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::list;
    /// let mut list = list![4, 2, 6, 3, 1, 5];
    /// list.sort_by(Ord::cmp);
    /// assert_eq!(list, list![1, 2, 3, 4, 5, 6]);
    /// ```
    pub fn sort_by<F>(&mut self, cmp: F)
    where
        F: Fn(&T, &T) -> Ordering,
    {
        self.0.sort_by(cmp)
    }
}

impl_traits!(List, RcConstructor);

#[cfg(test)]
mod api_tests {
    use super::*;
    public_api_tests!(list_api_tests, List, list);
}
