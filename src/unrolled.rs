#[cfg(test)]
mod proptests;

use crate::shared::{ArcConstructor, RcConstructor, SmartPointer, SmartPointerConstructor};
use itertools::Itertools;
use std::cmp::Ordering;
use std::iter::{Cloned, FlatMap, Flatten, FromIterator, Map, Rev};
use std::marker::PhantomData;

const CAPACITY: usize = 256;

pub struct List<T: Clone>(UnrolledList<T, RcConstructor, RcConstructor>);
pub struct SharedList<T: Clone>(UnrolledList<T, ArcConstructor, ArcConstructor>);

#[derive(Clone)]
pub struct UnrolledList<
    T: Clone,
    C: SmartPointerConstructor<Vec<T>>,
    S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
>(S::RC);

// Check if these lists are equivalent via the iterator
impl<
        T: Clone + PartialEq,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > PartialEq for UnrolledList<T, C, S>
{
    fn eq(&self, other: &Self) -> bool {
        Iterator::eq(self.into_iter(), other.into_iter())
    }
}

impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > UnrolledList<T, C, S>
{
    pub fn new() -> Self {
        UnrolledList(S::RC::new(UnrolledCell::new()))
    }

    // Get the strong count of the node in question
    pub fn strong_count(&self) -> usize {
        S::RC::strong_count(&self.0)
    }

    // This is actually like O(n / 64) which is actually quite nice
    // Saves us some time
    pub fn len(&self) -> usize {
        self.node_iter().map(|node| node.index()).sum()
    }

    // [0 1 2 3 4 5] -> [6 7 8 9 10]
    // [5 4 3 2 1] <- [10 9 8 7 6]
    // This should be O(n / 256)
    pub fn reverse(self) -> Self {
        let mut node_iter = self.into_node_iter();
        let mut left = node_iter.next().expect("This node should always exist");
        {
            let inner = S::make_mut(&mut left.0);
            let elements_mut = C::make_mut(&mut inner.elements);

            if inner.index < elements_mut.len() {
                elements_mut.truncate(inner.index);
            }

            elements_mut.reverse();
            inner.next = None;
        }

        while let Some(mut right) = node_iter.next() {
            let cell = S::make_mut(&mut right.0);
            let elements_mut = C::make_mut(&mut cell.elements);

            if cell.index < elements_mut.len() {
                elements_mut.truncate(cell.index);
            }

            elements_mut.reverse();
            cell.next = Some(left);
            left = right;
        }

        left
    }

    pub fn last(&self) -> Option<T> {
        self.node_iter()
            .last()
            .map(|x| x.elements().first())
            .flatten()
            .cloned()
    }

    // Should be O(1) always
    pub fn car(&self) -> Option<T> {
        self.0.car().cloned()
    }

    pub fn cons(value: T, other: Self) -> Self {
        UnrolledCell::cons(value, other)
    }

    /// Alias for cons_mut
    pub fn push_front(&mut self, value: T) {
        self.cons_mut(value)
    }

    pub fn cons_mut(&mut self, value: T) {
        let index = self.0.index;
        if self.0.index < self.elements().len() {
            // println!("Inside cons_mut here!");
            // reference.truncate(self.index);
            C::make_mut(&mut S::make_mut(&mut self.0).elements).truncate(index);
        }

        // TODO cdr here is an issue - only moves the offset, no way to know that its full
        // Cause its not actually full
        if self.0.full || self.elements().len() > CAPACITY - 1 {
            // println!("Case 1: {}, {}", self.0.full, self.elements().len());
            // Make dummy node
            // return reference to this new node
            let mut default = UnrolledList(S::RC::new(UnrolledCell {
                index: 1,
                elements: C::RC::new(vec![value]),
                next: Some(self.clone()),
                full: false,
            }));

            std::mem::swap(self, &mut default);
        } else {
            // println!("Case 2");
            // println!("#### before: {:?}", self.elements());

            let inner = S::make_mut(&mut self.0);
            inner.cons_mut(value);

            // println!("#### After: {:?}", self.elements());
        }
    }

    // Should be O(1) always
    // Should also not have to clone
    pub fn cdr(&self) -> Option<UnrolledList<T, C, S>> {
        self.0.cdr()
    }

    // Returns the cdr of the list
    // Returns None if the next is empty - otherwise updates self to be the rest
    pub fn cdr_mut(&mut self) -> Option<&mut Self> {
        if self.0.index > 1 {
            S::make_mut(&mut self.0).index -= 1;
            Some(self)
        } else {
            let inner = S::make_mut(&mut self.0);
            let output = inner.next.take();
            match output {
                Some(x) => {
                    *self = x;
                    Some(self)
                }
                None => {
                    *self = Self::new();
                    None
                }
            }
        }
    }

    fn elements(&self) -> &[T] {
        &self.0.elements
    }

    fn at_capacity(&self) -> bool {
        self.0.full || self.0.elements.len() == CAPACITY
    }

    fn does_node_satisfy_invariant(&self) -> bool {
        self.0.full || self.elements().len() <= CAPACITY
    }

    fn assert_list_invariants(&self) {
        assert!(self.does_node_satisfy_invariant())
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a T> {
        self.into_iter()
    }

    // pub fn iter<'a>(&'a self) -> IterRef<'a, T, C, S> {
    //     IterRef {
    //         cur: Some(self),
    //         index: self.0.index,
    //         _inner: PhantomData,
    //     }
    // }

    fn into_node_iter(self) -> NodeIter<T, C, S> {
        NodeIter {
            cur: Some(self),
            _inner: PhantomData,
        }
    }

    fn node_iter<'a>(&'a self) -> NodeIterRef<'a, T, C, S> {
        NodeIterRef {
            cur: Some(self),
            _inner: PhantomData,
        }
    }

    // TODO investigate using this for the other iterators and see if its faster
    // Consuming iterators
    pub fn test_iter<'a>(&'a self) -> impl Iterator<Item = &'a T> {
        self.node_iter()
            .flat_map(|x| x.elements()[0..x.index()].into_iter().rev())
    }

    // pub fn get_type<'a>(&'a self) {
    //     self.into_node_iter().flat_map(|mut x| {
    //         let cell = S::make_mut(&mut x.0);
    //         let vec = C::make_mut(&mut cell.elements);
    //         let elements = std::mem::take(vec);
    //         elements.into_iter().take(self.index).rev()
    //     })
    // }

    // See what the perf is of this
    pub fn into_test_iter(self) -> impl Iterator<Item = T> {
        self.into_node_iter().flat_map(|mut x| {
            let cell = S::make_mut(&mut x.0);
            let vec = C::make_mut(&mut cell.elements);
            let elements = std::mem::take(vec);
            elements.into_iter().rev()
        })

        // todo!()
    }

    // Every node must have either CAPACITY elements, or be marked as full
    // Debateable whether I want them marked as full
    pub fn assert_invariants(&self) -> bool {
        self.node_iter()
            .all(|x| Self::does_node_satisfy_invariant(&x))
    }

    // TODO document time complexity of this
    // Looks like its O(n / 64)
    // TODO make this not so bad - also how it works with half full nodes
    pub fn get(&self, mut index: usize) -> Option<T> {
        if index < self.0.index {
            return self.0.elements.get(self.0.index - index - 1).cloned();
        } else {
            let mut cur = self.0.next.as_ref();
            index -= self.0.elements.len();
            while let Some(node) = cur {
                if index < node.0.index {
                    let node_cap = node.0.index;
                    return node.0.elements.get(node_cap - index - 1).cloned();
                } else {
                    cur = node.0.next.as_ref();
                    index -= node.0.elements.len();
                }
            }

            None
        }
    }

    // Be able to in place mutate
    pub fn append_mut(&mut self, other: Self) {
        if other.elements().is_empty() {
            return;
        }

        let mut default = UnrolledList::new();
        std::mem::swap(self, &mut default);

        default = default.append(other);
        std::mem::swap(self, &mut default);
    }

    // Functional append
    pub fn append(self, other: Self) -> Self {
        if other.elements().is_empty() {
            return self;
        }

        self.into_node_iter()
            .into_iter()
            .chain(other.into_node_iter())
            .collect()
    }

    // Figure out how in the heck you sort this
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        self.sort_by(Ord::cmp)
    }

    // Figure out how you sort this
    pub fn sort_by<F>(&mut self, cmp: F)
    where
        F: Fn(&T, &T) -> Ordering,
    {
        todo!()
    }

    // Append single value (?)
    // Its super bad and not sure that I would want to support it but here we are
    pub fn push_back(&mut self, value: T) {
        todo!()
    }

    // Extend from an iterator over values
    // TODO optimize this otherwise
    pub fn extend(self, iter: impl IntoIterator<Item = T>) -> Self {
        self.append(iter.into_iter().collect())
    }

    pub fn is_empty(&self) -> bool {
        self.0.elements.is_empty()
    }

    fn index(&self) -> usize {
        self.0.index
    }

    // fn cons_mut(&mut self, value: T) {
    //     // self.0.cons_mut(value)

    //     todo!()
    // }
}

// impl<
//         T: Clone + 'static,
//         C: SmartPointerConstructor<Vec<T>> + 'static,
//         S: SmartPointerConstructor<UnrolledCell<T, S, C>> + 'static,
//     > UnrolledList<T, C, S>
// {
//     fn into_test_iter(self) -> impl Iterator<Item = T> {
//         self.into_node_iter().flat_map(|x| {
//             let inner = &x.0.elements;

//             inner.iter().map(|x| x.clone()).rev()
//         })
//     }
// }

// Don't blow the stack
impl<T: Clone, S: SmartPointerConstructor<Self>, C: SmartPointerConstructor<Vec<T>>> Drop
    for UnrolledCell<T, S, C>
{
    fn drop(&mut self) {
        let mut cur = self.next.take().map(|x| x.0);
        loop {
            match cur {
                Some(r) => match S::RC::try_unwrap(r) {
                    Some(UnrolledCell { ref mut next, .. }) => cur = next.take().map(|x| x.0),
                    _ => return,
                },
                _ => return,
            }
        }
    }
}

#[derive(Clone)]
pub struct UnrolledCell<
    T: Clone,
    S: SmartPointerConstructor<Self>,
    C: SmartPointerConstructor<Vec<T>>,
> {
    index: usize,
    elements: C::RC,
    next: Option<UnrolledList<T, C, S>>,
    full: bool,
}

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
    fn new() -> Self {
        UnrolledCell {
            index: 0,
            elements: C::RC::new(Vec::new()),
            next: None,
            full: false,
        }
    }

    // fn is_empty(&self) -> bool {
    //     self.elements.is_empty()
    // }

    // TODO this fails on an empty list
    // Speed this up by fixing the indexing
    fn car(&self) -> Option<&T> {
        if self.index == 0 {
            return None;
        }
        self.elements.get(self.index - 1)
    }

    // TODO fix cdr
    fn cdr(&self) -> Option<UnrolledList<T, C, S>> {
        // println!("index: {}", self.index);
        if self.index > 1 {
            Some(UnrolledList(S::RC::new(self.advance_cursor())))
        } else {
            self.next.clone()
        }
    }

    fn advance_cursor(&self) -> Self {
        UnrolledCell {
            index: self.index - 1,
            elements: C::RC::clone(&self.elements),
            next: self.next.clone(),
            full: self.full,
        }
    }

    // TODO make this better
    fn cons_mut(&mut self, value: T) {
        let reference = C::make_mut(&mut self.elements);

        // If the cursor isn't pointing to the end, wipe out elements that aren't useful to us
        // anymore since we've copied the underlying vector
        // TODO this is in the above level
        // if self.index < reference.len() {
        //     println!("Inside cons_mut here!");
        //     reference.truncate(self.index);
        // }

        reference.push(value);

        self.index += 1;
    }

    // fn cons_empty(value: T) -> Self {
    //     UnrolledCell {
    //         index: 0,
    //         elements: C::RC::new(vec![value]),
    //         next: None,
    //         full: false,
    //     }
    // }

    // fn cons_raw(value: T, mut cdr: UnrolledList<T, C, S>) -> UnrolledList<T, C, S> {
    //     if cdr.0.full || cdr.elements().len() > CAPACITY - 1 {
    //         UnrolledList(S::RC::new(UnrolledCell {
    //             index: 1,
    //             elements: C::RC::new(vec![value]),
    //             next: Some(cdr),
    //             full: false,
    //         }))
    //     } else {
    //         cdr.cons_mut(value);
    //         cdr
    //     }
    // }

    // Spill over the values to a new node
    // otherwise, copy the node and spill over
    fn cons(value: T, mut cdr: UnrolledList<T, C, S>) -> UnrolledList<T, C, S> {
        if cdr.0.full || cdr.elements().len() > CAPACITY - 1 {
            UnrolledList(S::RC::new(UnrolledCell {
                index: 1,
                elements: C::RC::new(vec![value]),
                next: Some(cdr),
                full: false,
            }))
        } else {
            let inner = S::make_mut(&mut cdr.0);
            let elements = C::make_mut(&mut inner.elements);
            inner.index += 1;
            elements.push(value);
            cdr
        }
    }
}

struct NodeIter<
    T: Clone,
    C: SmartPointerConstructor<Vec<T>>,
    S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
> {
    cur: Option<UnrolledList<T, C, S>>,
    _inner: PhantomData<T>,
}

impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > Iterator for NodeIter<T, C, S>
{
    type Item = UnrolledList<T, C, S>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(_self) = &self.cur {
            let ret_val = self.cur.clone();
            self.cur = _self.0.next.clone();
            ret_val
        } else {
            None
        }
    }
}

struct NodeIterRef<
    'a,
    T: Clone,
    C: SmartPointerConstructor<Vec<T>>,
    S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
> {
    cur: Option<&'a UnrolledList<T, C, S>>,
    _inner: PhantomData<T>,
}

impl<
        'a,
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > Iterator for NodeIterRef<'a, T, C, S>
{
    type Item = &'a UnrolledList<T, C, S>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(_self) = &self.cur {
            let ret_val = self.cur;
            self.cur = _self.0.next.as_ref();
            ret_val
        } else {
            None
        }
    }
}

// pub struct IterRef<
//     'a,
//     T: Clone,
//     C: SmartPointerConstructor<Vec<T>>,
//     S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
// > {
//     cur: Option<&'a UnrolledList<T, C, S>>,
//     index: usize,
//     _inner: PhantomData<T>,
// }

// impl<
//         'a,
//         T: Clone,
//         C: SmartPointerConstructor<Vec<T>>,
//         S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
//     > Iterator for IterRef<'a, T, C, S>
// {
//     type Item = &'a T;
//     fn next(&mut self) -> Option<Self::Item> {
//         if let Some(_self) = &self.cur {
//             if self.index > 0 {
//                 let return_value = _self.elements().get(self.index - 1);
//                 self.index -= 1;
//                 return_value
//             } else {
//                 self.cur = _self.0.next.as_ref();
//                 self.index = self.cur.as_ref().map(|x| x.elements().len()).unwrap_or(0);
//                 let ret = self.cur.as_ref().and_then(|x| x.0.car());
//                 if ret.is_some() {
//                     self.index -= 1;
//                 }
//                 ret
//             }
//         } else {
//             None
//         }
//     }
// }

// pub struct Iter<
//     T: Clone,
//     C: SmartPointerConstructor<Vec<T>>,
//     S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
// > {
//     cur: Option<UnrolledList<T, C, S>>,
//     index: usize,
//     _inner: PhantomData<T>,
// }

// impl<
//         T: Clone,
//         C: SmartPointerConstructor<Vec<T>>,
//         S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
//     > Iterator for Iter<T, C, S>
// {
//     type Item = T;
//     fn next(&mut self) -> Option<Self::Item> {
//         if let Some(_self) = &self.cur {
//             if self.index > 0 {
//                 let return_value = _self.elements().get(self.index - 1).cloned();
//                 self.index -= 1;
//                 return_value
//             } else {
//                 self.cur = _self.0.next.clone();
//                 self.index = self.cur.as_ref().map(|x| x.elements().len()).unwrap_or(0);
//                 let ret = self.cur.as_ref().and_then(|x| x.car());
//                 if ret.is_some() {
//                     self.index -= 1;
//                 }
//                 ret
//             }
//         } else {
//             None
//         }
//     }
// }

// and we'll implement IntoIterator
// impl<
//         T: Clone,
//         C: SmartPointerConstructor<Vec<T>>,
//         S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
//     > IntoIterator for UnrolledList<T, C, S>
// {
//     type Item = T;
//     type IntoIter = Iter<Self::Item, C, S>;

//     fn into_iter(self) -> Self::IntoIter {
//         Iter {
//             index: self.0.index,
//             cur: Some(self),
//             _inner: PhantomData,
//         }
//     }
// }

// and we'll implement IntoIterator
// impl<
//         T: Clone,
//         C: SmartPointerConstructor<Vec<T>>,
//         S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
//     > IntoIterator for &UnrolledList<T, C, S>
// {
//     type Item = T;
//     type IntoIter = Iter<Self::Item, C, S>;

//     fn into_iter(self) -> Self::IntoIter {
//         Iter {
//             index: self.0.index,
//             cur: Some(self.clone()),
//             _inner: PhantomData,
//         }
//     }
// }

pub struct ConsumingIterWrapper<
    T: Clone,
    C: SmartPointerConstructor<Vec<T>>,
    S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
> {
    inner: FlatMap<
        NodeIter<T, C, S>,
        Rev<std::iter::Take<std::vec::IntoIter<T>>>,
        fn(UnrolledList<T, C, S>) -> Rev<std::iter::Take<std::vec::IntoIter<T>>>,
    >,
}

impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > Iterator for ConsumingIterWrapper<T, C, S>
{
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > IntoIterator for UnrolledList<T, C, S>
{
    type Item = T;
    type IntoIter = ConsumingIterWrapper<T, C, S>;

    fn into_iter(self) -> Self::IntoIter {
        ConsumingIterWrapper {
            inner: self.into_node_iter().flat_map(move |mut x| {
                let cell = S::make_mut(&mut x.0);
                let vec = C::make_mut(&mut cell.elements);
                let elements = std::mem::take(vec);
                elements.into_iter().take(x.index()).rev()
            }),
        }
    }
}

pub struct IterWrapper<
    'a,
    T: Clone,
    C: SmartPointerConstructor<Vec<T>>,
    S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
> {
    inner: FlatMap<
        NodeIterRef<'a, T, C, S>,
        Rev<std::slice::Iter<'a, T>>,
        fn(&'a UnrolledList<T, C, S>) -> Rev<std::slice::Iter<'a, T>>,
    >,
}

impl<
        'a,
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > Iterator for IterWrapper<'a, T, C, S>
{
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<
        'a,
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > IntoIterator for &'a UnrolledList<T, C, S>
{
    type Item = &'a T;
    type IntoIter = IterWrapper<'a, T, C, S>;

    fn into_iter(self) -> Self::IntoIter {
        IterWrapper {
            inner: self
                .node_iter()
                .flat_map(|x| x.elements()[0..x.index()].into_iter().rev()),
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
                let full = elements.len() == CAPACITY;
                UnrolledList(S::RC::new(UnrolledCell {
                    index: elements.len(),
                    elements: C::RC::new(elements),
                    next: None,
                    full,
                }))
            })
            .collect();

        let mut rev_iter = (0..pairs.len()).into_iter().rev();
        rev_iter.next();

        for i in rev_iter {
            let prev = pairs.pop().unwrap();

            if let Some(UnrolledList(cell)) = pairs.get_mut(i) {
                S::RC::get_mut(cell)
                    .expect("Only one owner allowed in construction")
                    .next = Some(prev);
            } else {
                unreachable!()
            }
        }

        pairs.pop().unwrap_or(Self::new())
    }
}

impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > FromIterator<UnrolledList<T, C, S>> for UnrolledList<T, C, S>
{
    fn from_iter<I: IntoIterator<Item = UnrolledList<T, C, S>>>(iter: I) -> Self {
        // Links up the nodes
        let mut nodes: Vec<_> = iter.into_iter().collect();

        let mut rev_iter = (0..nodes.len()).into_iter().rev();
        rev_iter.next();

        for i in rev_iter {
            // TODO need to truncate the front of this one
            let mut prev = nodes.pop().unwrap();

            if let Some(UnrolledList(cell)) = nodes.get_mut(i) {
                // Check if this node can fit entirely into the previous one
                if cell.elements.len() + prev.0.elements.len() < CAPACITY {
                    let left_inner = S::make_mut(cell);
                    let right_inner = S::make_mut(&mut prev.0);

                    let left_vector = C::make_mut(&mut left_inner.elements);
                    let right_vector = C::make_mut(&mut right_inner.elements);

                    // Drop the useless elements
                    if left_inner.index < left_vector.len() {
                        left_vector.truncate(left_inner.index);
                    }

                    // TODO
                    if right_inner.index < right_vector.len() {
                        right_vector.truncate(right_inner.index);
                    }

                    // Perform the actual move of the values
                    right_vector.append(left_vector);

                    // Swap the locations now after we've done the update
                    std::mem::swap(left_vector, right_vector);
                    // Adjust the indices accordingly
                    left_inner.index = left_vector.len();
                    right_inner.index = 0;

                    // Update this node to now point to the right nodes tail
                    std::mem::swap(&mut left_inner.next, &mut right_inner.next);
                } else {
                    S::make_mut(cell).next = Some(prev);
                }
            } else {
                unreachable!()
            }
        }

        nodes.pop().unwrap_or(Self::new())
    }
}

impl<
        'a,
        T: 'a + Clone,
        C: 'a + SmartPointerConstructor<Vec<T>>,
        S: 'a + SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > FromIterator<&'a UnrolledList<T, C, S>> for UnrolledList<T, C, S>
{
    fn from_iter<I: IntoIterator<Item = &'a UnrolledList<T, C, S>>>(iter: I) -> Self {
        iter.into_iter().cloned().collect()
    }
}

impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > From<Vec<T>> for UnrolledList<T, C, S>
{
    fn from(vec: Vec<T>) -> Self {
        vec.into_iter().collect()
    }
}

pub type RcList<T> = UnrolledList<T, RcConstructor, RcConstructor>;
pub type ArcList<T> = UnrolledList<T, ArcConstructor, ArcConstructor>;

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn basic_iteration() {
        let list: RcList<_> = (0..100usize).into_iter().collect();
        let vec: Vec<_> = (0..100usize).into_iter().collect();

        Iterator::eq(list.into_iter(), vec.into_iter());
    }

    #[test]
    fn small() {
        let list: RcList<_> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9].into_iter().collect();
        Iterator::eq(list.into_iter(), (1..=9).into_iter());
    }

    #[test]
    fn append() {
        let mut left: RcList<_> = vec![1, 2, 3, 4, 5].into_iter().collect();
        let right: RcList<_> = vec![6, 7, 8, 9, 10].into_iter().collect();
        left = left.append(right.clone());
        left.assert_invariants();
        Iterator::eq(left.into_iter(), (1..=10).into_iter());
    }

    #[test]
    fn append_large() {
        let mut left: RcList<_> = (0..60).into_iter().collect();
        let right: RcList<_> = (60..100).into_iter().collect();

        left = left.append(right);

        left.assert_invariants();

        Iterator::eq(left.into_iter(), (0..100).into_iter());
    }
}

#[cfg(test)]
mod iterator_tests {
    use super::*;

    #[test]
    fn basic_construction() {
        // Assert the left and the right are equivalent after iterating
        let list: RcList<_> = (0..1000).into_iter().collect();
        let equivalent_vector: Vec<_> = (0..1000).into_iter().collect();

        for (left, right) in list.into_iter().zip(equivalent_vector) {
            assert_eq!(left, right);
        }
    }

    // Asserts that the iterators are the same
    #[test]
    fn iterates_all_elements() {
        let list: RcList<_> = (0..1000).into_iter().collect();
        let equivalent_vector: Vec<_> = (0..1000).into_iter().collect();

        assert_eq!(
            list.into_iter().count(),
            equivalent_vector.into_iter().count()
        );
    }

    // Asserts that the iterator correctly iterates everything
    #[test]
    fn iterates_correct_amount() {
        let count = 1000;
        let list: RcList<_> = (0..count).into_iter().collect();

        assert_eq!(list.into_iter().count(), count)
    }

    // TODO verify that this is actually what we want to happen
    // In some ways this might not be the performance that we want
    // Profile to make sure
    #[test]
    fn node_appending_coalescing_works() {
        // 356
        // 256 + 100
        let mut left: RcList<_> = (0..CAPACITY + 100).into_iter().collect();

        // 400
        let right: RcList<_> = (CAPACITY + 100..CAPACITY + 500).into_iter().collect();

        left = left.append(right);

        // Should have 4 nodes at this point
        assert_eq!(left.node_iter().count(), 4);

        // 300 should be at 300
        assert_eq!(left.get(300).unwrap(), 300);
        left.assert_list_invariants();
    }

    #[test]
    fn length() {
        let list: RcList<_> = (0..300).into_iter().collect();
        assert_eq!(list.len(), 300);
    }

    #[test]
    fn indexing() {
        let list: RcList<_> = (0..300).into_iter().collect();

        for i in 0..300 {
            assert_eq!(list.get(i).unwrap(), i);
        }
    }

    #[test]
    fn cdr_iterative() {
        let mut list: Option<RcList<_>> = Some((0..1000).into_iter().collect());
        let mut i = 0;

        while let Some(car) = list.as_ref().map(|x| x.car()).flatten() {
            assert_eq!(i, car);
            list = list.unwrap().cdr();
            i += 1;
        }
    }

    #[test]
    fn cons_mut_new_node() {
        let mut list: RcList<_> = (0..CAPACITY).into_iter().collect();

        // Should have 1 node at this point
        assert_eq!(list.node_iter().count(), 1);

        // Consing should move to a new node
        list.cons_mut(1000);

        // This should be 2 at this point
        assert_eq!(list.node_iter().count(), 2);
    }

    #[test]
    fn cons_mut_list() {
        let mut list: RcList<_> = RcList::new();

        for i in (0..1000).into_iter().rev() {
            list.cons_mut(i);
        }

        for i in 0..1000 {
            assert_eq!(i, list.get(i).unwrap());
        }
    }

    #[test]
    fn empty_list() {
        let list: RcList<usize> = <Vec<usize>>::new().into_iter().collect();
        assert!(list.is_empty());
    }

    #[test]
    fn cdr_works_successfully() {
        let list: RcList<usize> = vec![1, 2, 3, 4, 5].into_iter().collect();

        let cdr = list.cdr().unwrap();

        let expected_cdr: RcList<usize> = vec![2, 3, 4, 5].into_iter().collect();

        assert_eq!(
            cdr.into_iter().collect::<Vec<_>>(),
            expected_cdr.into_iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn cdr_mut() {
        let mut list: RcList<usize> = vec![1, 2, 3usize].into_iter().collect();
        assert!(list.cdr_mut().is_some());
        assert_eq!(list.len(), 2);
        assert!(list.cdr_mut().is_some());
        assert_eq!(list.len(), 1);
        assert!(list.cdr_mut().is_none());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn reverse() {
        let list: RcList<usize> = (0..500).into_iter().collect();
        let reversed = list.reverse();

        assert!(Iterator::eq(
            (0..500).into_iter().rev(),
            reversed.into_iter()
        ));
    }

    #[test]
    fn last() {
        let list: RcList<usize> = RcList::new();
        assert!(list.last().is_none());
    }

    #[test]
    fn last_single_node() {
        let list: RcList<_> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10].into();
        assert_eq!(list.last(), Some(10));
    }

    #[test]
    fn last_multiple_nodes() {
        let list: RcList<_> = (0..2 * CAPACITY).into_iter().collect();
        assert_eq!(list.last(), Some(CAPACITY * 2 - 1))
    }
}

#[cfg(test)]
mod reference_counting_correctness {

    use super::*;

    #[derive(Clone)]
    enum Value {
        List(RcList<usize>),
    }

    #[test]
    fn test_append() {
        fn function_call(args: &mut [Value]) -> Value {
            let arg2 = args[1].clone();
            let mut arg1 = &mut args[0];

            match (&mut arg1, arg2) {
                (Value::List(left), Value::List(right)) => {
                    assert_eq!(left.strong_count(), 1);
                    assert_eq!(right.strong_count(), 2);

                    left.append_mut(right);

                    assert_eq!(left.strong_count(), 1);
                }
            }

            arg1.clone()
        }

        let mut args = vec![
            Value::List(vec![0, 1, 2, 3, 4, 5].into_iter().collect()),
            Value::List(vec![6, 7, 8, 9, 10].into_iter().collect()),
        ];

        let Value::List(result) = function_call(args.as_mut_slice());

        assert_eq!(result.strong_count(), 2);

        // Drop everything from the stack
        args.clear();
        assert_eq!(result.strong_count(), 1);
    }
}
