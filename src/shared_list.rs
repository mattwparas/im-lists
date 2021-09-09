use std::iter::FromIterator;

// #[cfg(test)]
// use crate::public_api_tests;

use crate::{
    shared::ArcConstructor,
    unrolled::{ConsumingWrapper, IterWrapper, UnrolledList},
};

/// A persistent, thread safe, List.
///
/// This list is suitable for a multi threaded environment. If do not need an immutable list that can be shared
/// across threads (i.e., is [`Send`] + [`Sync`], see [`List`](crate::list::List)).
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

#[derive(PartialEq)]
pub struct SharedList<T: Clone>(UnrolledList<T, ArcConstructor, ArcConstructor>);

impl<T: Clone> SharedList<T> {
    /// Construct an empty list.
    pub fn new() -> Self {
        SharedList(UnrolledList::new())
    }

    /// Get the number of strong references pointing to this list
    ///
    /// Time: O(1)
    pub fn strong_count(&self) -> usize {
        self.0.strong_count()
    }

    /// Get the number of cells that comprise this list
    pub fn cell_count(&self) -> usize {
        self.0.cell_count()
    }

    /// Get the length of the list
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::shared_list;
    /// let list = shared_list![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
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
    /// # use im_lists::shared_list;
    /// let list = shared_list![1, 2, 3, 4, 5].reverse();
    /// assert_eq!(list, shared_list![5, 4, 3, 2, 1])
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
    /// # use im_lists::shared_list;
    /// let list = shared_list![1, 2, 3, 4, 5];
    /// assert_eq!(list.last(), Some(5));
    /// ```
    pub fn last(&self) -> Option<T> {
        self.0.last()
    }

    /// Get the first element of the list.
    /// Returns None if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::shared_list;
    /// # use im_lists::shared_list::SharedList;
    /// let list = shared_list![1, 2, 3, 4, 5];
    /// let car = list.car();
    /// assert_eq!(car, Some(1));
    ///
    /// let list: SharedList<usize> = shared_list![];
    /// let car = list.car();
    /// assert!(car.is_none());
    /// ```
    pub fn car(&self) -> Option<T> {
        self.0.car()
    }

    /// Get the "rest" of the elements as a list, excluding the first element
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::shared_list;
    /// let list = shared_list![1, 2, 3, 4, 5];
    /// let cdr = list.cdr().unwrap();
    /// assert_eq!(cdr, shared_list![2, 3, 4, 5]);
    ///
    /// let list = shared_list![5];
    /// let cdr = list.cdr();
    /// assert!(cdr.is_none());
    /// ```
    pub fn cdr(&self) -> Option<SharedList<T>> {
        self.0.cdr().map(SharedList)
    }

    /// Gets the cdr of the list, mutably.
    /// Returns None if the next is empty - otherwise updates self to be the rest
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::shared_list;
    /// let mut list = shared_list![1, 2, 3, 4, 5];
    /// list.cdr_mut().expect("This list has a tail");
    /// assert_eq!(list, shared_list![2, 3, 4, 5]);
    ///
    ///
    /// let mut list = shared_list![1, 2, 3];
    /// assert!(list.cdr_mut().is_some());
    /// assert_eq!(list, shared_list![2, 3]);
    /// assert!(list.cdr_mut().is_some());
    /// assert_eq!(list, shared_list![3]);
    /// assert!(list.cdr_mut().is_none());
    /// assert_eq!(list, shared_list![]);
    /// ```
    pub fn cdr_mut(&mut self) -> Option<&mut Self> {
        match self.0.cdr_mut() {
            Some(_) => Some(self),
            None => None,
        }
    }

    /// Pushes an element onto the front of the list, returning a new list
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::shared_list::SharedList;
    /// let list = SharedList::cons(1, SharedList::cons(2, SharedList::cons(3, SharedList::cons(4, SharedList::new()))));
    /// assert_eq!(list, shared_list![1, 2, 3, 4]);
    /// ```
    pub fn cons(value: T, other: SharedList<T>) -> SharedList<T> {
        Self(UnrolledList::cons(value, other.0))
    }

    /// Mutably pushes an element onto the front of the list, in place
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::shared_list;
    /// let mut list = shared_list![];
    /// list.cons_mut(3);
    /// list.cons_mut(2);
    /// list.cons_mut(1);
    /// list.cons_mut(0);
    /// assert_eq!(list, shared_list![0, 1, 2, 3])
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
    /// # use im_lists::shared_list;
    /// let mut list = shared_list![];
    /// list.push_front(3);
    /// list.push_front(2);
    /// list.push_front(1);
    /// list.push_front(0);
    /// assert_eq!(list, shared_list![0, 1, 2, 3])
    /// ```
    pub fn push_front(&mut self, value: T) {
        self.0.push_front(value)
    }

    /// Constructs an iterator over the list
    pub fn iter(&self) -> impl Iterator<Item = &'_ T> {
        self.0.iter()
    }

    /// Get a reference to the value at index `index` in a list.
    /// Returns `None` if the index is out of bounds.
    pub fn get(&self, index: usize) -> Option<T> {
        self.0.get(index)
    }

    /// Append the list other to the end of the current list. Returns a new list.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::shared_list;
    /// let left = shared_list![1usize, 2, 3];
    /// let right = shared_list![4usize, 5, 6];
    /// assert_eq!(left.append(right), shared_list![1, 2, 3, 4, 5, 6])
    /// ```
    pub fn append(self, other: Self) -> Self {
        SharedList(self.0.append(other.0))
    }

    /// Append the list 'other' to the end of the current list in place.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::shared_list;
    /// let mut left = shared_list![1usize, 2, 3];
    /// let right = shared_list![4usize, 5, 6];
    /// left.append_mut(right);
    /// assert_eq!(left, shared_list![1, 2, 3, 4, 5, 6])
    /// ```
    pub fn append_mut(&mut self, other: Self) {
        self.0.append_mut(other.0);
    }

    /// Checks whether a list is empty
    ///
    /// # Examples
    /// ```
    /// # #[macro_use] extern crate im_lists;
    /// # use im_lists::shared_list;
    /// # use im_lists::shared_list::SharedList;
    /// let mut list = SharedList::new();
    /// assert!(list.is_empty());
    /// list.cons_mut("applesauce");
    /// assert!(!list.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl_traits!(SharedList, ArcConstructor);

#[cfg(test)]
mod api_tests {
    use super::*;

    public_api_tests!(shared_list_api_tests, SharedList, shared_list);
}
