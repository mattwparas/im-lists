//! A persistent list.
//!
//! This is a sequence of elements, akin to a cons list. The most common operation is to
//! [`cons`](crate::list::List::cons) to the front (or [`cons_mut`](crate::list::List::cons_mut))
//! The API is designed to be a drop in replacement for an immutable linked list implementation, with instead the backing
//! being an unrolled linked list or VList, depending on your configuration.
//!
//! # Performance Notes
//!
//! Using the mutable functions when possible enables in place mutation. Much of the internal structure is shared,
//! so even immutable functions can be fast, but the mutable functions will be faster.

use std::{cmp::Ordering, iter::FromIterator, marker::PhantomData};

use crate::{
    handler::{DefaultDropHandler, DropHandler},
    shared::{ArcPointer, PointerFamily, RcPointer},
    unrolled::{ConsumingWrapper, IterWrapper, UnrolledCell, UnrolledList},
};

/// A persistent list.
///
/// This list is suitable for either a single threaded or multi threaded environment. The list accepts the smart pointer
/// that you would like to use as a type parameter. There are sensible type aliases for implementations that you can use:
///
/// [`SharedList`] is simply a type alias for `GenericList<T, ArcPointer, 256, 1>`, which is both [`Send`] + [`Sync`]
/// Similarly, [`List`] is just a type alias for `GenericList<T, RcPointer, 256, 1>`. [`SharedVList`] and
/// [`VList`] are type aliases, as well, using the same backing of `GenericList`, however they have a growth factor of 2 - meaning
/// bucket sizes will grow exponentially.
///
/// It's implemented as an unrolled linked list, which is a single linked list which stores a variable
/// amount of elements in each node. The capacity of any individual node for now is set to to be `N` elements, which means that until more than `N` elements
/// are cons'd onto a list, it will remain a vector under the hood. By default, N is sset to 256. There is also a growth rate, `G`, which describes how
/// each successive node will grow in size. With `N = 2`, and `G = 2`, the list will look something like this:
///
/// ```text
/// [0, 1, 2, 3, 4, 5, 6, 7] -> [8, 9, 10, 11] -> [12, 13]
///
/// ```
///
/// The list is also designed to leverage in place mutations whenever possible - if the number of references pointing to either a cell containing a vector
/// or the shared vector is one, then that mutation is done in place. Otherwise, it is copy-on-write, maintaining our persistent invariant.
///
/// ## Performance Notes
///
/// The algorithmic complexity of an unrolled linked list matches that of a normal linked list - however in practice
/// we have a decrease by the factor of the capacity of a node that gives us practical
/// performance wins. For a list that is fully filled, iteration over nodes becomes O(n / N), rather than the typical O(n). If the growth rate is set to 2 (or more),
/// over individual nodes becomes O(log(n)) - which means indexing or finding the last element is O(log(n)) as well.
/// In addition, the unrolled linked list is able to avoid the costly cache misses that a typical linked list
/// suffers from, seeing very realistic performance gains.
///
/// Let *n* be the number of elements in the list, and *m* is the capacity of a node.
/// In the worst case, a node will be on average half filled. In the best case, all nodes are completely full.
/// This means for operations that for a normal linked list may take linear time *Θ(n)*, we get a constant factor
/// decrease of either a factor of *m* or *m / 2*. Similarly, we will see O(log(n)) performance characteristics if the growth rate is set to be larger than 1.
#[repr(transparent)]
pub struct GenericList<
    T: Clone + 'static,
    P: PointerFamily = RcPointer,
    const N: usize = 256,
    const G: usize = 1,
    D: DropHandler<Self> = DefaultDropHandler,
>(UnrolledList<T, P, N, G>, PhantomData<D>);

pub type SharedList<T> = GenericList<T, ArcPointer, 256>;
pub type List<T> = GenericList<T, RcPointer, 256>;

pub type SharedVList<T> = GenericList<T, ArcPointer, 2, 2>;
pub type VList<T> = GenericList<T, RcPointer, 2, 2>;

#[doc(hidden)]
#[derive(Copy, Clone)]
pub struct RawCell<
    T: Clone + 'static,
    P: PointerFamily,
    const N: usize,
    const G: usize,
    D: DropHandler<GenericList<T, P, N, G, D>>,
>(*const UnrolledCell<T, P, N, G>, PhantomData<D>);

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>> Clone
    for GenericList<T, P, N, G, D>
{
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>>
    GenericList<T, P, N, G, D>
{
    /// Construct an empty list.
    pub fn new() -> Self {
        GenericList(UnrolledList::new(), PhantomData)
    }

    /// Constructs an empty list with capacity `N`
    pub fn new_with_capacity() -> Self {
        GenericList(UnrolledList::new_with_capacity(), PhantomData)
    }

    /// Get the number of strong references pointing to this list
    ///
    /// Time: O(1)
    pub fn strong_count(&self) -> usize {
        self.0.strong_count()
    }

    /// Compare this list to another for pointer equality
    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
    }

    #[doc(hidden)]
    pub fn inner_ptr(&self) -> &P::Pointer<UnrolledCell<T, P, N, G>> {
        &self.0 .0
    }

    #[doc(hidden)]
    pub fn storage_ptr_eq(&self, other: &Self) -> bool {
        self.0.shared_ptr_eq(&other.0)
    }

    #[doc(hidden)]
    pub fn as_ptr_usize(&self) -> usize {
        self.0.as_ptr_usize()
    }

    #[doc(hidden)]
    pub fn elements_as_ptr_usize(&self) -> usize {
        // Overflow is fine - this should give us a unique value?
        self.0.elements_as_ptr_usize() + self.0.index()
    }

    #[doc(hidden)]
    pub fn identity_tuple(&self) -> (usize, usize) {
        (self.0.elements_as_ptr_usize(), self.0.index())
    }

    // Check the next pointer. If the next pointer is the same,
    // then we otherwise need to check the values of the current node of the list.
    #[doc(hidden)]
    pub fn next_ptr_as_usize(&self) -> Option<usize> {
        self.0.next_ptr_as_usize()
    }

    #[doc(hidden)]
    pub fn current_node_iter(&self) -> impl Iterator<Item = &T> {
        self.0.current_node_iter()
    }

    #[doc(hidden)]
    pub fn node_count(&self) -> usize {
        self.0.cell_count()
    }

    #[doc(hidden)]
    pub fn draining_iterator(
        mut self,
        // default: UnrolledList<T, P, N, G>,
    ) -> impl Iterator<Item = T> {
        std::mem::take(&mut self.0).draining_iterator()
        // std::mem::replace(&mut self.0, default).draining_iterator()
        // todo!()
        // let x = MaybeUninit::new(self);
        // let x = x.as_ptr();

        // unsafe { std::ptr::read(&(*x).0).draining_iterator() }

        // self.0.draining_iterator()
        // ManuallyDrop::take(slot)
        // todo!()
    }

    #[doc(hidden)]
    pub fn nodes(&self) -> Vec<Self> {
        self.0
            .node_iter()
            .map(|x| {
                let mut x = x.clone();
                P::make_mut(&mut x.0).next = None;
                Self(x, PhantomData)
            })
            .collect()
    }

    #[doc(hidden)]
    pub fn as_ptr(&self) -> RawCell<T, P, N, G, D> {
        RawCell(self.0.as_ptr(), PhantomData)
    }

    /// Call a function on a raw pointer
    /// # Safety
    /// This must be called with a valid pointer as returned from as_ptr
    #[doc(hidden)]
    pub unsafe fn call_from_raw<O, F: FnOnce(&Self) -> O>(
        cell: RawCell<T, P, N, G, D>,
        func: F,
    ) -> O {
        let value = unsafe { Self::from_raw(cell) };
        let res = func(&value);
        std::mem::forget(value);
        res
    }

    /// # Safety
    /// This must be called with a valid pointer as returned from as_ptr
    #[doc(hidden)]
    unsafe fn from_raw(cell: RawCell<T, P, N, G, D>) -> Self {
        Self(UnrolledList(P::from_raw(cell.0)), PhantomData)
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
        self.0 = std::mem::take(&mut self.0).reverse();
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
    pub fn cdr(&self) -> Option<GenericList<T, P, N, G, D>> {
        self.0.cdr().map(|x| GenericList(x, PhantomData))
    }

    /// Get the "rest" of the elements as a list.
    /// Alias for [`cdr`](crate::list::List::cdr)
    pub fn rest(&self) -> Option<GenericList<T, P, N, G, D>> {
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

    #[doc(hidden)]
    pub fn cdr_exists(&self) -> bool {
        self.0.cdr_exists()
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
    pub fn cons(value: T, mut other: GenericList<T, P, N, G, D>) -> GenericList<T, P, N, G, D> {
        Self(
            UnrolledList::cons(value, std::mem::take(&mut other.0)),
            PhantomData,
        )
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
        GenericList(self.0.take(count), PhantomData)
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
        self.0.tail(len).map(|x| GenericList(x, PhantomData))
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
    pub fn append(mut self, mut other: Self) -> Self {
        GenericList(
            std::mem::take(&mut self.0).append(std::mem::take(&mut other.0)),
            PhantomData,
        )
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
    pub fn append_mut(&mut self, mut other: Self) {
        self.0.append_mut(std::mem::take(&mut other.0));
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

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>> Default
    for GenericList<T, P, N, G, D>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>> Extend<T>
    for GenericList<T, P, N, G, D>
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.append_mut(iter.into_iter().collect())
    }
}

// and we'll implement FromIterator
impl<T: Clone, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>>
    FromIterator<T> for GenericList<T, P, N, G, D>
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        GenericList(iter.into_iter().collect(), PhantomData)
    }
}

impl<'a, T: 'a + Clone, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>>
    FromIterator<&'a T> for GenericList<T, P, N, G, D>
{
    fn from_iter<I: IntoIterator<Item = &'a T>>(iter: I) -> Self {
        GenericList(iter.into_iter().cloned().collect(), PhantomData)
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>>
    FromIterator<GenericList<T, P, N, G, D>> for GenericList<T, P, N, G, D>
{
    fn from_iter<I: IntoIterator<Item = GenericList<T, P, N, G, D>>>(iter: I) -> Self {
        GenericList(
            iter.into_iter()
                .flat_map(|mut x| std::mem::take(&mut x.0).into_node_iter())
                .collect(),
            PhantomData,
        )
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>> From<Vec<T>>
    for GenericList<T, P, N, G, D>
{
    fn from(vec: Vec<T>) -> Self {
        GenericList(vec.into_iter().collect(), PhantomData)
    }
}

impl<
        T: Clone + std::fmt::Debug,
        P: PointerFamily,
        const N: usize,
        const G: usize,
        D: DropHandler<Self>,
    > std::fmt::Debug for GenericList<T, P, N, G, D>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

/// An iterator over lists with values of type `T`.
pub struct Iter<
    'a,
    T: Clone + 'static,
    P: PointerFamily,
    const N: usize,
    const G: usize,
    D: DropHandler<GenericList<T, P, N, G, D>>,
>(IterWrapper<'a, T, P, N, G>, PhantomData<D>);

impl<
        'a,
        T: Clone + 'static,
        P: PointerFamily,
        const N: usize,
        const G: usize,
        D: DropHandler<GenericList<T, P, N, G, D>>,
    > Iterator for Iter<'a, T, P, N, G, D>
{
    type Item = &'a T;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    #[inline(always)]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.0.fold(init, f)
    }
}

impl<
        'a,
        T: Clone,
        P: PointerFamily,
        const N: usize,
        const G: usize,
        D: DropHandler<GenericList<T, P, N, G, D>>,
    > IntoIterator for &'a GenericList<T, P, N, G, D>
{
    type Item = &'a T;
    type IntoIter = Iter<'a, T, P, N, G, D>;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        Iter((&self.0).into_iter(), PhantomData)
    }
}

/// A consuming iterator over lists with values of type `T`.
pub struct ConsumingIter<
    T: Clone + 'static,
    P: PointerFamily,
    const N: usize,
    const G: usize,
    D: DropHandler<GenericList<T, P, N, G, D>>,
>(ConsumingWrapper<T, P, N, G>, PhantomData<D>);

impl<
        T: Clone,
        P: PointerFamily,
        const N: usize,
        const G: usize,
        D: DropHandler<GenericList<T, P, N, G, D>>,
    > Iterator for ConsumingIter<T, P, N, G, D>
{
    type Item = T;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    #[inline(always)]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.0.fold(init, f)
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>> IntoIterator
    for GenericList<T, P, N, G, D>
{
    type Item = T;
    type IntoIter = ConsumingIter<T, P, N, G, D>;

    #[inline(always)]
    fn into_iter(mut self) -> Self::IntoIter {
        ConsumingIter(std::mem::take(&mut self.0).into_iter(), PhantomData)
    }
}

impl<
        'a,
        T: 'a + Clone,
        P: 'a + PointerFamily,
        const N: usize,
        const G: usize,
        D: 'a + DropHandler<Self>,
    > FromIterator<&'a GenericList<T, P, N, G, D>> for GenericList<T, P, N, G, D>
{
    fn from_iter<I: IntoIterator<Item = &'a GenericList<T, P, N, G, D>>>(iter: I) -> Self {
        iter.into_iter().cloned().collect()
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>> From<&[T]>
    for GenericList<T, P, N, G, D>
{
    fn from(vec: &[T]) -> Self {
        vec.iter().cloned().collect()
    }
}

impl<
        T: Clone + PartialEq,
        P: PointerFamily,
        const N: usize,
        const G: usize,
        D: DropHandler<Self>,
    > PartialEq for GenericList<T, P, N, G, D>
{
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl<T: Clone + Eq, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>> Eq
    for GenericList<T, P, N, G, D>
{
}

impl<
        T: Clone + PartialOrd,
        P: PointerFamily,
        const N: usize,
        const G: usize,
        D: DropHandler<Self>,
    > PartialOrd for GenericList<T, P, N, G, D>
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<T: Clone + Ord, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>> Ord
    for GenericList<T, P, N, G, D>
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>> std::ops::Add
    for GenericList<T, P, N, G, D>
{
    type Output = GenericList<T, P, N, G, D>;

    /// Concatenate two lists
    fn add(self, other: Self) -> Self::Output {
        self.append(other)
    }
}

impl<
        T: Clone,
        P: PointerFamily,
        const N: usize,
        const G: usize,
        D: DropHandler<GenericList<T, P, N, G, D>>,
    > std::ops::Add for &GenericList<T, P, N, G, D>
{
    type Output = GenericList<T, P, N, G, D>;

    /// Concatenate two lists
    fn add(self, other: Self) -> Self::Output {
        self.clone().append(other.clone())
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>>
    std::iter::Sum for GenericList<T, P, N, G, D>
{
    fn sum<I>(it: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        it.fold(Self::new(), |a, b| a + b)
    }
}

impl<
        T: Clone + std::hash::Hash,
        P: PointerFamily,
        const N: usize,
        const G: usize,
        D: DropHandler<Self>,
    > std::hash::Hash for GenericList<T, P, N, G, D>
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for i in self {
            i.hash(state)
        }
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>>
    std::ops::Index<usize> for GenericList<T, P, N, G, D>
{
    type Output = T;
    /// Get a reference to the value at index `index` in the vector.
    ///
    /// Time: O(log n)
    fn index(&self, index: usize) -> &Self::Output {
        match self.get(index) {
            Some(value) => value,
            None => panic!(
                "{}::index: index out of bounds: {} < {}",
                stringify!($list),
                index,
                self.len()
            ),
        }
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize, D: DropHandler<Self>> Drop
    for GenericList<T, P, N, G, D>
{
    fn drop(&mut self) {
        D::drop_handler(self)
    }
}

#[cfg(test)]
mod tests {

    use std::ops::Add;

    use super::*;
    use crate::{list, vlist};

    #[test]
    fn strong_count_empty() {
        let list: List<usize> = List::new();
        assert!(list.strong_count() >= 1);
    }

    #[test]
    fn strong_count() {
        let mut list: List<usize> = List::new();
        list.cons_mut(1);
        assert_eq!(list.strong_count(), 1);
    }

    #[test]
    fn ptr_eq() {
        let left: List<usize> = list![1, 2, 3, 4, 5];
        let right: List<usize> = list![1, 2, 3, 4, 5];

        assert!(!left.ptr_eq(&right));

        let left_clone: List<usize> = left.clone();
        assert!(left.ptr_eq(&left_clone))
    }

    #[test]
    fn cdr_ptr_eq() {
        let left: List<usize> = list![1, 2, 3, 4, 5];

        let new_right = left.cdr().unwrap();
        let new_right2 = left.cdr().unwrap();

        assert!(new_right.identity_tuple() == new_right2.identity_tuple());
    }

    #[test]
    fn len() {
        let list = list![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(list.len(), 10);
    }

    #[test]
    fn reverse() {
        let list = list![1, 2, 3, 4, 5].reverse();
        assert_eq!(list, list![5, 4, 3, 2, 1]);
    }

    #[test]
    fn last() {
        let list = list![1, 2, 3, 4, 5];
        assert_eq!(list.last().cloned(), Some(5));
    }

    #[test]
    fn car() {
        let list = list![1, 2, 3, 4, 5];
        let car = list.car();
        assert_eq!(car, Some(1));

        let list: List<usize> = list![];
        let car = list.car();
        assert!(car.is_none());
    }

    #[test]
    fn first() {
        let list = list![1, 2, 3, 4, 5];
        let car = list.first();
        assert_eq!(car.cloned(), Some(1));

        let list: List<usize> = list![];
        let car = list.first();
        assert!(car.is_none());
    }

    #[test]
    fn cdr() {
        let list = list![1, 2, 3, 4, 5];
        let cdr = list.cdr().unwrap();
        assert_eq!(cdr, list![2, 3, 4, 5]);
        let list = list![5];
        let cdr = list.cdr();
        assert!(cdr.is_none());
    }

    #[test]
    fn cdr_mut() {
        let mut list = list![1, 2, 3, 4, 5];
        list.cdr_mut().expect("This list has a tail");
        assert_eq!(list, list![2, 3, 4, 5]);

        let mut list = list![1, 2, 3];
        assert!(list.cdr_mut().is_some());
        assert_eq!(list, list![2, 3]);
        assert!(list.cdr_mut().is_some());
        assert_eq!(list, list![3]);
        assert!(list.cdr_mut().is_none());
        assert_eq!(list, list![]);
    }

    #[test]
    fn rest_mut() {
        let mut list = list![1, 2, 3, 4, 5];
        list.rest_mut().expect("This list has a tail");
        assert_eq!(list, list![2, 3, 4, 5]);

        let mut list = list![1, 2, 3];
        assert!(list.rest_mut().is_some());
        assert_eq!(list, list![2, 3]);
        assert!(list.rest_mut().is_some());
        assert_eq!(list, list![3]);
        assert!(list.rest_mut().is_none());
        assert_eq!(list, list![]);
    }

    #[test]
    fn cons() {
        let list = List::cons(1, List::cons(2, List::cons(3, List::cons(4, List::new()))));
        assert_eq!(list, list![1, 2, 3, 4]);
    }

    #[test]
    fn cons_mut() {
        let mut list = list![];
        list.cons_mut(3);
        list.cons_mut(2);
        list.cons_mut(1);
        list.cons_mut(0);
        assert_eq!(list, list![0, 1, 2, 3])
    }

    #[test]
    fn push_front() {
        let mut list = list![];
        list.push_front(3);
        list.push_front(2);
        list.push_front(1);
        list.push_front(0);
        assert_eq!(list, list![0, 1, 2, 3])
    }

    #[test]
    fn iter() {
        assert_eq!(list![1usize, 1, 1, 1, 1].iter().sum::<usize>(), 5);
    }

    #[test]
    fn get() {
        let list = list![1, 2, 3, 4, 5];
        assert_eq!(list.get(3).cloned(), Some(4));
        assert!(list.get(1000).is_none());
    }

    #[test]
    fn append() {
        let left = list![1usize, 2, 3];
        let right = list![4usize, 5, 6];
        assert_eq!(left.append(right), list![1, 2, 3, 4, 5, 6])
    }

    #[test]
    fn append_mut() {
        let mut left = list![1usize, 2, 3];
        let right = list![4usize, 5, 6];
        left.append_mut(right);
        assert_eq!(left, list![1, 2, 3, 4, 5, 6])
    }

    #[test]
    fn is_empty() {
        let mut list = List::new();
        assert!(list.is_empty());
        list.cons_mut("applesauce");
        assert!(!list.is_empty());
    }

    #[test]
    fn extend() {
        let mut list = list![1usize, 2, 3];
        let vec = vec![4, 5, 6];
        list.extend(vec);
        assert_eq!(list, list![1, 2, 3, 4, 5, 6])
    }

    #[test]
    fn sort() {
        let mut list = list![5, 4, 3, 2, 1];
        list.sort();
        assert_eq!(list, list![1, 2, 3, 4, 5]);
    }

    #[test]
    fn sort_by() {
        let mut list = list![5, 4, 3, 2, 1];
        list.sort_by(Ord::cmp);
        assert_eq!(list, list![1, 2, 3, 4, 5]);
    }

    #[test]
    fn push_back() {
        let mut list = list![];
        list.push_back(0);
        list.push_back(1);
        list.push_back(2);
        assert_eq!(list, list![0, 1, 2]);
    }

    #[test]
    fn add() {
        let left = list![1, 2, 3, 4, 5];
        let right = list![6, 7, 8, 9, 10];

        assert_eq!(left + right, list![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn sum() {
        let list = vec![list![1, 2, 3], list![4, 5, 6], list![7, 8, 9]];
        assert_eq!(
            list.into_iter().sum::<List<_>>(),
            list![1, 2, 3, 4, 5, 6, 7, 8, 9]
        );
    }

    #[test]
    fn take() {
        let list = list![0, 1, 2, 3, 4, 5];
        let new_list = list.take(3);
        assert_eq!(new_list, list![0, 1, 2]);
    }

    #[test]
    fn tail() {
        let list = list![0, 1, 2, 3, 4, 5];
        let new_list = list.tail(2);
        assert_eq!(new_list.unwrap(), list![2, 3, 4, 5]);

        let no_list = list.tail(100);
        assert!(no_list.is_none())
    }

    #[test]
    fn take_after_cdr() {
        let list = list![0, 1, 2, 3, 4, 5];
        let rest = list.rest().unwrap();

        assert_eq!(rest.take(3), list![1, 2, 3]);
    }

    #[test]
    fn tail_after_cdr() {
        let list = list![0, 1, 2, 3, 4, 5];
        let rest = list.rest().unwrap();

        assert_eq!(rest.tail(2).unwrap(), list![3, 4, 5]);
    }

    #[test]
    fn indexing() {
        let list = vlist![0, 1, 2, 3, 4, 5];

        assert_eq!(4, list[4]);
    }

    #[test]
    fn hash() {
        let mut map = std::collections::HashMap::new();

        map.insert(vlist![0, 1, 2, 3, 4, 5], "hello world!");

        assert_eq!(
            map.get(&vlist![0, 1, 2, 3, 4, 5]).copied(),
            Some("hello world!")
        );
    }

    #[test]
    fn addition() {
        let l = vlist![0, 1, 2, 3, 4, 5];
        let r = vlist![6, 7, 8, 9, 10];

        let combined = l.clone() + r.clone();

        assert_eq!(combined, vlist![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        let combined = l.add(r);

        assert_eq!(combined, vlist![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn from_slice() {
        let slice: &[usize] = &[0, 1, 2, 3, 4, 5];
        let list: VList<usize> = vlist![0, 1, 2, 3, 4, 5];

        assert_eq!(list, slice.into());
    }

    #[test]
    #[should_panic]
    fn index_out_of_bounds() {
        let list: VList<usize> = vlist![0, 1, 2, 3, 4];

        list[5];
    }

    #[test]
    fn ordering() {
        let l: VList<usize> = vlist![0, 1, 2, 3, 4];
        let r: VList<usize> = vlist![1, 2, 3, 4, 5];

        assert!(l < r);
    }

    #[test]
    fn from_iterator() {
        let iter = vec![
            vlist![0, 1, 2, 3, 4],
            vlist![0, 1, 2, 3, 4],
            vlist![0, 1, 2, 3, 4],
        ];

        let combined = iter.iter().collect::<VList<usize>>();

        assert_eq!(
            combined,
            vlist![0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1, 2, 3, 4]
        );
    }

    #[test]
    fn from_iterator_group_lists() {
        let iter = vec![
            vlist![0, 1, 2, 3, 4],
            vlist![0, 1, 2, 3, 4],
            vlist![0, 1, 2, 3, 4],
        ];

        let combined = iter.iter().collect::<VList<VList<usize>>>();

        assert_eq!(
            combined,
            vlist![
                vlist![0, 1, 2, 3, 4],
                vlist![0, 1, 2, 3, 4],
                vlist![0, 1, 2, 3, 4]
            ]
        );
    }

    #[test]
    fn to_string_works_as_intended() {
        let list = vlist![1, 2, 3, 4, 5];

        assert_eq!("[1, 2, 3, 4, 5]", format!("{:?}", list));
    }

    #[test]
    fn cons_grows_as_expected() {
        let list = vlist![1, 2];

        let list = VList::cons(0, list);

        assert_eq!(vlist![0, 1, 2], list);
        assert_eq!(2, list.0.node_iter().count());
    }
}

#[cfg(test)]
mod arc_tests {

    use std::ops::Add;

    use super::*;
    use crate::{shared_list, shared_vlist, vlist};

    #[test]
    fn strong_count_empty() {
        let list: SharedList<usize> = SharedList::new();
        assert!(list.strong_count() >= 1);
    }

    #[test]
    fn strong_count() {
        let mut list: SharedList<usize> = SharedList::new();
        list.cons_mut(1);
        assert_eq!(list.strong_count(), 1);
    }

    #[test]
    fn ptr_eq() {
        let left: SharedList<usize> = shared_list![1, 2, 3, 4, 5];
        let right: SharedList<usize> = shared_list![1, 2, 3, 4, 5];

        assert!(!left.ptr_eq(&right));

        let left_clone: SharedList<usize> = left.clone();
        assert!(left.ptr_eq(&left_clone))
    }

    #[test]
    fn len() {
        let list = shared_list![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(list.len(), 10);
    }

    #[test]
    fn reverse() {
        let list = shared_list![1, 2, 3, 4, 5].reverse();
        assert_eq!(list, shared_list![5, 4, 3, 2, 1]);
    }

    #[test]
    fn last() {
        let list = shared_list![1, 2, 3, 4, 5];
        assert_eq!(list.last().cloned(), Some(5));
    }

    #[test]
    fn car() {
        let list = shared_list![1, 2, 3, 4, 5];
        let car = list.car();
        assert_eq!(car, Some(1));

        let list: SharedList<usize> = shared_list![];
        let car = list.car();
        assert!(car.is_none());
    }

    #[test]
    fn first() {
        let list = shared_list![1, 2, 3, 4, 5];
        let car = list.first();
        assert_eq!(car.cloned(), Some(1));

        let list: SharedList<usize> = shared_list![];
        let car = list.first();
        assert!(car.is_none());
    }

    #[test]
    fn cdr() {
        let list = shared_list![1, 2, 3, 4, 5];
        let cdr = list.cdr().unwrap();
        assert_eq!(cdr, shared_list![2, 3, 4, 5]);
        let list = shared_list![5];
        let cdr = list.cdr();
        assert!(cdr.is_none());
    }

    #[test]
    fn cdr_mut() {
        let mut list = shared_list![1, 2, 3, 4, 5];
        list.cdr_mut().expect("This list has a tail");
        assert_eq!(list, shared_list![2, 3, 4, 5]);

        let mut list = shared_list![1, 2, 3];
        assert!(list.cdr_mut().is_some());
        assert_eq!(list, shared_list![2, 3]);
        assert!(list.cdr_mut().is_some());
        assert_eq!(list, shared_list![3]);
        assert!(list.cdr_mut().is_none());
        assert_eq!(list, shared_list![]);
    }

    #[test]
    fn rest_mut() {
        let mut list = shared_list![1, 2, 3, 4, 5];
        list.rest_mut().expect("This list has a tail");
        assert_eq!(list, shared_list![2, 3, 4, 5]);

        let mut list = shared_list![1, 2, 3];
        assert!(list.rest_mut().is_some());
        assert_eq!(list, shared_list![2, 3]);
        assert!(list.rest_mut().is_some());
        assert_eq!(list, shared_list![3]);
        assert!(list.rest_mut().is_none());
        assert_eq!(list, shared_list![]);
    }

    #[test]
    fn cons() {
        let list = SharedList::cons(
            1,
            SharedList::cons(
                2,
                SharedList::cons(3, SharedList::cons(4, SharedList::new())),
            ),
        );
        assert_eq!(list, shared_list![1, 2, 3, 4]);
    }

    #[test]
    fn cons_mut() {
        let mut list = shared_list![];
        list.cons_mut(3);
        list.cons_mut(2);
        list.cons_mut(1);
        list.cons_mut(0);
        assert_eq!(list, shared_list![0, 1, 2, 3])
    }

    #[test]
    fn push_front() {
        let mut list = shared_list![];
        list.push_front(3);
        list.push_front(2);
        list.push_front(1);
        list.push_front(0);
        assert_eq!(list, shared_list![0, 1, 2, 3])
    }

    #[test]
    fn iter() {
        assert_eq!(shared_list![1usize, 1, 1, 1, 1].iter().sum::<usize>(), 5);
    }

    #[test]
    fn get() {
        let list = shared_list![1, 2, 3, 4, 5];
        assert_eq!(list.get(3).cloned(), Some(4));
        assert!(list.get(1000).is_none());
    }

    #[test]
    fn append() {
        let left = shared_list![1usize, 2, 3];
        let right = shared_list![4usize, 5, 6];
        assert_eq!(left.append(right), shared_list![1, 2, 3, 4, 5, 6])
    }

    #[test]
    fn append_mut() {
        let mut left = shared_list![1usize, 2, 3];
        let right = shared_list![4usize, 5, 6];
        left.append_mut(right);
        assert_eq!(left, shared_list![1, 2, 3, 4, 5, 6])
    }

    #[test]
    fn is_empty() {
        let mut list = List::new();
        assert!(list.is_empty());
        list.cons_mut("applesauce");
        assert!(!list.is_empty());
    }

    #[test]
    fn extend() {
        let mut list = shared_list![1usize, 2, 3];
        let vec = vec![4, 5, 6];
        list.extend(vec);
        assert_eq!(list, shared_list![1, 2, 3, 4, 5, 6])
    }

    #[test]
    fn sort() {
        let mut list = shared_list![5, 4, 3, 2, 1];
        list.sort();
        assert_eq!(list, shared_list![1, 2, 3, 4, 5]);
    }

    #[test]
    fn sort_by() {
        let mut list = shared_list![5, 4, 3, 2, 1];
        list.sort_by(Ord::cmp);
        assert_eq!(list, shared_list![1, 2, 3, 4, 5]);
    }

    #[test]
    fn push_back() {
        let mut list = shared_list![];
        list.push_back(0);
        list.push_back(1);
        list.push_back(2);
        assert_eq!(list, shared_list![0, 1, 2]);
    }

    #[test]
    fn add() {
        let left = shared_list![1, 2, 3, 4, 5];
        let right = shared_list![6, 7, 8, 9, 10];

        assert_eq!(left + right, shared_list![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn sum() {
        let list = vec![
            shared_list![1, 2, 3],
            shared_list![4, 5, 6],
            shared_list![7, 8, 9],
        ];
        assert_eq!(
            list.into_iter().sum::<SharedList<_>>(),
            shared_list![1, 2, 3, 4, 5, 6, 7, 8, 9]
        );
    }

    #[test]
    fn take() {
        let list = shared_list![0, 1, 2, 3, 4, 5];
        let new_list = list.take(3);
        assert_eq!(new_list, shared_list![0, 1, 2]);
    }

    #[test]
    fn tail() {
        let list = shared_list![0, 1, 2, 3, 4, 5];
        let new_list = list.tail(2);
        assert_eq!(new_list.unwrap(), shared_list![2, 3, 4, 5]);

        let no_list = list.tail(100);
        assert!(no_list.is_none())
    }

    #[test]
    fn indexing() {
        let list = shared_vlist![0, 1, 2, 3, 4, 5];

        assert_eq!(4, list[4]);
    }

    #[test]
    fn hash() {
        let mut map = std::collections::HashMap::new();

        map.insert(shared_vlist![0, 1, 2, 3, 4, 5], "hello world!");

        assert_eq!(
            map.get(&shared_vlist![0, 1, 2, 3, 4, 5]).copied(),
            Some("hello world!")
        );
    }

    #[test]
    fn addition() {
        let l = shared_vlist![0, 1, 2, 3, 4, 5];
        let r = shared_vlist![6, 7, 8, 9, 10];

        let combined = l.clone() + r.clone();

        assert_eq!(combined, shared_vlist![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        let combined = l.add(r);

        assert_eq!(combined, shared_vlist![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn from_slice() {
        let slice: &[usize] = &[0, 1, 2, 3, 4, 5];
        let list: SharedVList<usize> = shared_vlist![0, 1, 2, 3, 4, 5];

        assert_eq!(list, slice.into());
    }

    #[test]
    #[should_panic]
    fn index_out_of_bounds() {
        let list: SharedVList<usize> = shared_vlist![0, 1, 2, 3, 4];

        list[5];
    }

    #[test]
    fn ordering() {
        let l: SharedVList<usize> = shared_vlist![0, 1, 2, 3, 4];
        let r: SharedVList<usize> = shared_vlist![1, 2, 3, 4, 5];

        assert!(l < r);
    }

    #[test]
    fn from_iterator() {
        let iter = vec![
            shared_vlist![0, 1, 2, 3, 4],
            shared_vlist![0, 1, 2, 3, 4],
            shared_vlist![0, 1, 2, 3, 4],
        ];

        let combined = iter.iter().collect::<SharedVList<usize>>();

        assert_eq!(
            combined,
            shared_vlist![0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1, 2, 3, 4]
        );
    }

    #[test]
    fn from_iterator_group_lists() {
        let iter = vec![
            shared_vlist![0, 1, 2, 3, 4],
            shared_vlist![0, 1, 2, 3, 4],
            shared_vlist![0, 1, 2, 3, 4],
        ];

        let combined = iter.iter().collect::<SharedVList<SharedVList<usize>>>();

        assert_eq!(
            combined,
            shared_vlist![
                shared_vlist![0, 1, 2, 3, 4],
                shared_vlist![0, 1, 2, 3, 4],
                shared_vlist![0, 1, 2, 3, 4]
            ]
        );
    }

    #[test]
    fn vlist_growth() {
        let list = vlist![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20];

        let counts: Vec<_> = list.0.node_iter().map(|x| x.elements().len()).collect();
        assert_eq!(vec![6, 8, 4, 2], counts);
    }

    #[test]
    fn consuming_iter_with_no_references() {
        let list = vlist![0, 1, 2, 3, 4, 5, 6, 7, 8];

        let result = list.draining_iterator().collect::<Vec<_>>();

        assert_eq!(vec![0, 1, 2, 3, 4, 5, 6, 7, 8], result);
    }

    #[test]
    fn consuming_iter() {
        let list = vlist![0, 1, 2, 3, 4, 5, 6, 7, 8];
        let _second_list = list.cdr();

        let result = list.draining_iterator().collect::<Vec<_>>();

        assert_eq!(Vec::<usize>::new(), result);
    }

    #[test]
    fn list_empty_test() {
        let mut list = (0..10000usize).into_iter().collect::<SharedVList<_>>();

        for _ in 0..10000 {
            list.cdr_mut();

            if list.len() == 0 {
                assert!(list.is_empty())
            } else {
                assert!(!list.is_empty())
            }
        }

        // assert!(list.is_empty())
    }

    #[test]
    fn raw_test() {
        let list = (0..1000usize).into_iter().collect::<SharedVList<_>>();

        // Get the inner pointer, and then otherwise
        // call the drop implementation as neatly as possible.
        let pointer = list.as_ptr();

        // Create value from pointer
        let value = unsafe { SharedVList::from_raw(pointer) };
        std::mem::forget(value);
    }
}
