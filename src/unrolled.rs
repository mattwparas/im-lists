#[cfg(test)]
mod proptests;

use crate::shared::{SmartPointer, SmartPointerConstructor};
use itertools::Itertools;
use std::cmp::Ordering;
use std::iter::{FlatMap, FromIterator, Rev};
use std::marker::PhantomData;

const CAPACITY: usize = 256;

type ConsumingIter<T, C, S> = FlatMap<
    NodeIter<T, C, S>,
    Rev<std::iter::Take<std::vec::IntoIter<T>>>,
    fn(UnrolledList<T, C, S>) -> Rev<std::iter::Take<std::vec::IntoIter<T>>>,
>;

type RefIter<'a, T, C, S> = FlatMap<
    NodeIterRef<'a, T, C, S>,
    Rev<std::slice::Iter<'a, T>>,
    fn(&'a UnrolledList<T, C, S>) -> Rev<std::slice::Iter<'a, T>>,
>;

#[derive(Clone, Eq)]
pub(crate) struct UnrolledList<
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
        Iterator::eq(self.iter(), other.iter())
    }
}

impl<
        T: Clone + PartialOrd,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > PartialOrd for UnrolledList<T, C, S>
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > Default for UnrolledList<T, C, S>
{
    fn default() -> Self {
        Self::new()
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

    // Compare the nodes for pointer equality
    pub fn ptr_eq(&self, other: &Self) -> bool {
        S::RC::ptr_eq(&self.0, &other.0)
    }

    #[cfg(test)]
    fn cell_count(&self) -> usize {
        self.node_iter().count()
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

        for mut right in node_iter {
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

    pub fn last(&self) -> Option<&T> {
        self.node_iter().last().and_then(|x| x.elements().first())
    }

    // Should be O(1) always
    pub fn car(&self) -> Option<T> {
        self.0.car().cloned()
    }

    pub fn cons(value: T, other: Self) -> Self {
        UnrolledCell::cons(value, other)
    }

    pub fn take(&self, mut count: usize) -> Self {
        // If the count of the vector
        if count == 0 {
            return Self::new();
        }

        let mut nodes = Vec::new();

        // If we've asked for more elements than this list contains
        // and there aren't any more to follow, just return this list
        if count > self.0.index && self.0.next.is_none() {
            return self.clone();
        }

        for mut node in self.clone().into_node_iter() {
            if count < node.0.index {
                let inner = S::make_mut(&mut node.0);
                // this is the new tail, point to the end
                inner.next = None;

                // We want to chop off whatever we need to
                let elements_mut = C::make_mut(&mut inner.elements);

                // Grab the end of the vector, this will be the new backing
                let remaining = elements_mut.split_off(inner.index - count);
                inner.index = count;
                *elements_mut = remaining;
                nodes.push(node);
                break;
            } else {
                count -= node.0.elements.len();
                nodes.push(node);
            }
        }

        let mut rev_iter = (0..nodes.len()).into_iter().rev();
        rev_iter.next();

        for i in rev_iter {
            let prev = nodes.pop().unwrap();

            if let Some(UnrolledList(cell)) = nodes.get_mut(i) {
                S::make_mut(cell).next = Some(prev);
            } else {
                unreachable!()
            }
        }

        nodes.pop().unwrap_or_default()
    }

    pub fn tail(&self, mut len: usize) -> Option<Self> {
        // If the count of the vector
        if len == 0 {
            return Some(self.clone());
        }

        for mut node in self.clone().into_node_iter() {
            if len < node.0.index {
                let inner = S::make_mut(&mut node.0);
                // this is the new tail, point to the end
                // inner.next = None;
                inner.index -= len;
                return Some(node);
            } else {
                len -= node.0.elements.len();
            }
        }

        if len == 0 {
            return Some(Self::new());
        }

        // Self::new()
        None
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
            let inner = S::make_mut(&mut self.0);
            inner.cons_mut(value);
        }
    }

    // Should be O(1) always
    // Should also not have to clone
    pub fn cdr(&self) -> Option<UnrolledList<T, C, S>> {
        self.0.cdr()
    }

    // Just pop off the internal value and move the index up
    pub fn pop_front(&mut self) -> Option<T> {
        let cell = S::make_mut(&mut self.0);
        let elements = C::make_mut(&mut cell.elements);

        let ret = elements.pop();

        if ret.is_some() {
            cell.index -= 1;
        }

        // If after we've popped, its empty, move the pointer to the
        // next one (if there is one)
        if cell.index == 0 {
            if let Some(next) = &mut cell.next {
                let mut next = std::mem::take(next);
                std::mem::swap(self, &mut next);
            }
        }

        ret
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

    #[cfg(test)]
    fn does_node_satisfy_invariant(&self) -> bool {
        self.0.full || self.elements().len() <= CAPACITY
    }

    #[cfg(test)]
    fn assert_list_invariants(&self) {
        assert!(self.does_node_satisfy_invariant())
    }

    fn into_node_iter(self) -> NodeIter<T, C, S> {
        NodeIter {
            cur: Some(self),
            _inner: PhantomData,
        }
    }

    fn node_iter(&self) -> NodeIterRef<'_, T, C, S> {
        NodeIterRef {
            cur: Some(self),
            _inner: PhantomData,
        }
    }

    // TODO investigate using this for the other iterators and see if its faster
    // Consuming iterators
    pub fn iter(&self) -> impl Iterator<Item = &'_ T> {
        self.node_iter()
            .flat_map(|x| x.elements()[0..x.index()].iter().rev())
    }

    // Every node must have either CAPACITY elements, or be marked as full
    // Debateable whether I want them marked as full
    #[cfg(test)]
    pub fn assert_invariants(&self) -> bool {
        self.node_iter().all(Self::does_node_satisfy_invariant)
    }

    pub fn get(&self, mut index: usize) -> Option<&T> {
        if index < self.0.index {
            self.0.elements.get(self.0.index - index - 1)
        } else {
            let mut cur = self.0.next.as_ref();
            index -= self.0.elements.len();
            while let Some(node) = cur {
                if index < node.0.index {
                    let node_cap = node.0.index;
                    return node.0.elements.get(node_cap - index - 1);
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
        let list = std::mem::take(self);
        let mut vec = list.into_iter().collect::<Vec<_>>();
        vec.sort_by(cmp);
        *self = vec.into();
    }

    // Append a single value to the end
    pub fn push_back(&mut self, value: T) {
        self.extend(std::iter::once(value))
    }

    pub fn is_empty(&self) -> bool {
        self.0.elements.is_empty()
    }

    fn index(&self) -> usize {
        self.0.index
    }
}

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
pub(crate) struct UnrolledCell<
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

    // Speed this up by fixing the indexing
    fn car(&self) -> Option<&T> {
        if self.index == 0 {
            return None;
        }
        self.elements.get(self.index - 1)
    }

    fn cdr(&self) -> Option<UnrolledList<T, C, S>> {
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

impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > Extend<T> for UnrolledList<T, C, S>
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.append_mut(iter.into_iter().collect())
    }
}

pub(crate) struct NodeIter<
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

pub(crate) struct NodeIterRef<
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

// TODO have this expose tryfold
pub(crate) struct ConsumingWrapper<
    T: Clone,
    C: SmartPointerConstructor<Vec<T>>,
    S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
>(ConsumingIter<T, C, S>);

impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > Iterator for ConsumingWrapper<T, C, S>
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

impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > IntoIterator for UnrolledList<T, C, S>
{
    type Item = T;
    type IntoIter = ConsumingWrapper<T, C, S>;

    fn into_iter(self) -> Self::IntoIter {
        ConsumingWrapper(self.into_node_iter().flat_map(move |mut x| {
            let cell = S::make_mut(&mut x.0);
            let vec = C::make_mut(&mut cell.elements);
            let elements = std::mem::take(vec);
            elements.into_iter().take(x.index()).rev()
        }))
    }
}

// TODO have this also expose TryFold
pub(crate) struct IterWrapper<
    'a,
    T: Clone,
    C: SmartPointerConstructor<Vec<T>>,
    S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
>(RefIter<'a, T, C, S>);

impl<
        'a,
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > Iterator for IterWrapper<'a, T, C, S>
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
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > IntoIterator for &'a UnrolledList<T, C, S>
{
    type Item = &'a T;
    type IntoIter = IterWrapper<'a, T, C, S>;
    // type IntoIter = RefIter<'a, T, C, S>;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        IterWrapper(
            self.node_iter()
                .flat_map(|x| x.elements()[0..x.index()].iter().rev()),
        )
    }
}

// and we'll implement FromIterator
// TODO specialize this for the into version?
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

        pairs.pop().unwrap_or_else(Self::new)
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

        nodes.pop().unwrap_or_else(Self::new)
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

impl<
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > From<&[T]> for UnrolledList<T, C, S>
{
    fn from(vec: &[T]) -> Self {
        vec.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {

    use crate::shared::RcConstructor;

    type RcList<T> = UnrolledList<T, RcConstructor, RcConstructor>;

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
    use crate::shared::RcConstructor;

    type RcList<T> = UnrolledList<T, RcConstructor, RcConstructor>;

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
        assert_eq!(*left.get(300).unwrap(), 300);
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
            assert_eq!(*list.get(i).unwrap(), i);
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
            assert_eq!(i, *list.get(i).unwrap());
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
        assert_eq!(list.last().cloned(), Some(10));
    }

    #[test]
    fn last_multiple_nodes() {
        let list: RcList<_> = (0..2 * CAPACITY).into_iter().collect();
        assert_eq!(list.last().cloned(), Some(CAPACITY * 2 - 1))
    }

    #[test]
    fn take() {
        let list: RcList<usize> = (0..2 * CAPACITY).into_iter().collect();
        let next = list.take(100);

        println!("{:?}", next);
        println!("{:?}", next.elements());

        assert!(Iterator::eq(0..100usize, next.into_iter()))
    }

    #[test]
    fn take_big() {
        let list: RcList<usize> = (0..2 * CAPACITY).into_iter().collect();
        let next = list.take(CAPACITY + 100);
        assert!(Iterator::eq(0..CAPACITY + 100usize, next.into_iter()))
    }

    #[test]
    fn tail() {
        let list: RcList<usize> = (0..2 * CAPACITY).into_iter().collect();
        let next = list.tail(CAPACITY + 100).unwrap();

        println!("next: {:?}", next);
        assert!(Iterator::eq(
            CAPACITY + 100usize..2 * CAPACITY,
            next.into_iter()
        ))
    }

    #[test]
    fn tail_bigger_than_list() {
        let list: RcList<usize> = (0..2 * CAPACITY).into_iter().collect();
        let next = list.tail(CAPACITY * 4);

        assert!(next.is_none())
    }

    #[test]
    fn pop_front() {
        let mut list: RcList<usize> = vec![0, 1, 2, 3].into_iter().collect();
        assert_eq!(list.pop_front().unwrap(), 0);
        assert_eq!(list.pop_front().unwrap(), 1);
        assert_eq!(list.pop_front().unwrap(), 2);
        assert_eq!(list.pop_front().unwrap(), 3);
        assert!(list.pop_front().is_none())
    }

    #[test]
    fn pop_front_capacity() {
        let mut list: RcList<usize> = (0..CAPACITY).into_iter().collect();
        list.push_front(100);
        assert_eq!(list.cell_count(), 2);
        assert_eq!(list.pop_front().unwrap(), 100);
        assert_eq!(list.cell_count(), 1);
    }

    #[test]
    fn append_big() {
        let mut list: RcList<usize> = (0..3).into_iter().collect();
        let big_list: RcList<usize> = (0..CAPACITY - 1).into_iter().collect();

        list.append_mut(big_list);
    }
}

#[cfg(test)]
mod reference_counting_correctness {

    use super::*;
    use crate::shared::RcConstructor;
    type RcList<T> = UnrolledList<T, RcConstructor, RcConstructor>;

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
