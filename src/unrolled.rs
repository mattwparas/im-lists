#[cfg(test)]
mod proptests;

use smallvec::SmallVec;

use crate::shared::PointerFamily;

use std::cmp::Ordering;
use std::iter::{FlatMap, FromIterator, Rev};
use std::marker::PhantomData;

type ConsumingIter<T, P, const N: usize, const G: usize> = FlatMap<
    NodeIter<T, P, N, G>,
    Rev<std::iter::Take<std::vec::IntoIter<T>>>,
    fn(UnrolledList<T, P, N, G>) -> Rev<std::iter::Take<std::vec::IntoIter<T>>>,
>;

type RefIter<'a, T, P, const N: usize, const G: usize> = FlatMap<
    NodeIterRef<'a, T, P, N, G>,
    Rev<std::slice::Iter<'a, T>>,
    fn(&'a UnrolledList<T, P, N, G>) -> Rev<std::slice::Iter<'a, T>>,
>;

type DrainingConsumingIter<T, P, const N: usize, const G: usize> = FlatMap<
    DrainingNodeIter<T, P, N, G>,
    Rev<std::iter::Take<std::vec::IntoIter<T>>>,
    fn(UnrolledList<T, P, N, G>) -> Rev<std::iter::Take<std::vec::IntoIter<T>>>,
>;

#[derive(Eq)]
pub(crate) struct UnrolledList<T: Clone, P: PointerFamily, const N: usize, const G: usize = 1>(
    P::Pointer<UnrolledCell<T, P, N, G>>,
);

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> Clone
    for UnrolledList<T, P, N, G>
{
    fn clone(&self) -> Self {
        Self(P::clone(&self.0))
    }
}

// Check if these lists are equivalent via the iterator
impl<T: Clone + PartialEq, P: PointerFamily, const N: usize, const G: usize> PartialEq
    for UnrolledList<T, P, N, G>
{
    fn eq(&self, other: &Self) -> bool {
        Iterator::eq(self.iter(), other.iter())
    }
}

impl<T: Clone + PartialOrd, P: PointerFamily, const N: usize, const G: usize> PartialOrd
    for UnrolledList<T, P, N, G>
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> Default
    for UnrolledList<T, P, N, G>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> UnrolledList<T, P, N, G> {
    pub fn new() -> Self {
        UnrolledList(P::new(UnrolledCell::new()))
    }

    pub fn new_with_capacity() -> Self {
        UnrolledList(P::new(UnrolledCell::new_with_capacity()))
    }

    // Get the strong count of the node in question
    pub fn strong_count(&self) -> usize {
        P::strong_count(&self.0)
    }

    // Compare the nodes for pointer equality
    pub fn ptr_eq(&self, other: &Self) -> bool {
        P::ptr_eq(&self.0, &other.0)
    }

    pub fn shared_ptr_eq(&self, other: &Self) -> bool {
        P::ptr_eq(&self.0.elements, &other.0.elements) && self.0.index == other.0.index
    }

    pub fn as_ptr_usize(&self) -> usize {
        P::as_ptr(&self.0) as usize
    }

    pub fn elements_as_ptr_usize(&self) -> usize {
        P::as_ptr(&self.0.elements) as usize
    }

    pub fn draining_iterator(self) -> DrainingConsumingWrapper<T, P, N, G> {
        DrainingConsumingWrapper(self.into_draining_node_iter().flat_map(|x| {
            let index = x.index();

            P::try_unwrap(x.0)
                .map(|mut cell| {
                    P::get_mut(&mut cell.elements)
                        .map(|vec| std::mem::take(vec).into_iter().take(index).rev())
                        .unwrap_or_else(|| Vec::new().into_iter().take(0).rev())
                })
                .unwrap_or_else(|| Vec::new().into_iter().take(0).rev())
        }))
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
            let inner = P::make_mut(&mut left.0);
            let elements_mut = P::make_mut(&mut inner.elements);

            if inner.index < elements_mut.len() {
                elements_mut.truncate(inner.index);
            }

            elements_mut.reverse();
            inner.next = None;
        }

        for mut right in node_iter {
            let cell = P::make_mut(&mut right.0);
            let elements_mut = P::make_mut(&mut cell.elements);

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
                let inner = P::make_mut(&mut node.0);
                // this is the new tail, point to the end
                inner.next = None;

                // We want to chop off whatever we need to
                let elements_mut = P::make_mut(&mut inner.elements);

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

        let mut rev_iter = (0..nodes.len()).rev();
        rev_iter.next();

        for i in rev_iter {
            let prev = nodes.pop().unwrap();

            if let Some(UnrolledList(cell)) = nodes.get_mut(i) {
                P::make_mut(cell).next = Some(prev);
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
                let inner = P::make_mut(&mut node.0);
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
            P::make_mut(&mut P::make_mut(&mut self.0).elements).truncate(index);
        }

        // TODO cdr here is an issue - only moves the offset, no way to know that its full
        // Cause its not actually full
        if self.elements().len() > self.size() - 1 {
            // Make dummy node
            // return reference to this new node
            let mut default = UnrolledList(P::new(UnrolledCell {
                index: 1,
                elements: P::new(vec![value]),
                next: Some(self.clone()),
                size: self.size() * UnrolledCell::<T, P, N, G>::GROWTH_RATE,
            }));

            std::mem::swap(self, &mut default);
        } else {
            let inner = P::make_mut(&mut self.0);
            inner.cons_mut(value);
        }
    }

    // Should be O(1) always
    // Should also not have to clone
    pub fn cdr(&self) -> Option<UnrolledList<T, P, N, G>> {
        self.0.cdr()
    }

    // Just pop off the internal value and move the index up
    pub fn pop_front(&mut self) -> Option<T> {
        let cell = P::make_mut(&mut self.0);
        let elements = P::make_mut(&mut cell.elements);

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

    pub(crate) fn cdr_exists(&self) -> bool {
        self.0.index > 1 || self.0.next.is_some()
    }

    // Returns the cdr of the list
    // Returns None if the next is empty - otherwise updates self to be the rest
    pub fn cdr_mut(&mut self) -> Option<&mut Self> {
        if self.0.index > 1 {
            P::make_mut(&mut self.0).index -= 1;
            Some(self)
        } else {
            let inner = P::make_mut(&mut self.0);
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

    pub(crate) fn elements(&self) -> &[T] {
        &self.0.elements
    }

    fn size(&self) -> usize {
        self.0.size
    }

    #[cfg(test)]
    fn does_node_satisfy_invariant(&self) -> bool {
        self.elements().len() <= self.size()
    }

    #[cfg(test)]
    fn assert_list_invariants(&self) {
        assert!(self.does_node_satisfy_invariant())
    }

    pub(crate) fn into_draining_node_iter(self) -> DrainingNodeIter<T, P, N, G> {
        DrainingNodeIter {
            cur: Some(self),
            _inner: PhantomData,
        }
    }

    pub(crate) fn into_node_iter(self) -> NodeIter<T, P, N, G> {
        NodeIter {
            cur: Some(self),
            _inner: PhantomData,
        }
    }

    pub(crate) fn node_iter(&self) -> NodeIterRef<'_, T, P, N, G> {
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

    pub fn index(&self) -> usize {
        self.0.index
    }
}

// Don't blow the stack
impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> Drop for UnrolledCell<T, P, N, G> {
    fn drop(&mut self) {
        let mut cur = self.next.take().map(|x| x.0);
        loop {
            match cur {
                Some(r) => match P::try_unwrap(r) {
                    Some(UnrolledCell { ref mut next, .. }) => cur = next.take().map(|x| x.0),
                    _ => return,
                },
                _ => return,
            }
        }
    }
}

pub(crate) struct UnrolledCell<T: Clone, P: PointerFamily, const N: usize, const G: usize> {
    index: usize,
    elements: P::Pointer<Vec<T>>,
    next: Option<UnrolledList<T, P, N, G>>,
    size: usize,
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> Clone
    for UnrolledCell<T, P, N, G>
{
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            elements: P::clone(&self.elements),
            next: self.next.clone(),
            size: self.size,
        }
    }
}

impl<T: Clone + std::fmt::Debug, P: PointerFamily, const N: usize, const G: usize> std::fmt::Debug
    for UnrolledList<T, P, N, G>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> UnrolledCell<T, P, N, G> {
    const GROWTH_RATE: usize = if G == 0 { 1 } else { G };

    fn new() -> Self {
        UnrolledCell {
            index: 0,
            elements: P::new(Vec::new()),
            next: None,
            size: N,
        }
    }

    fn new_with_capacity() -> Self {
        UnrolledCell {
            index: 0,
            elements: P::new(Vec::with_capacity(N)),
            next: None,
            size: N,
        }
    }

    // Speed this up by fixing the indexing
    fn car(&self) -> Option<&T> {
        if self.index == 0 {
            return None;
        }
        self.elements.get(self.index - 1)
    }

    // This _does_ create a boxed representation of the next item. Its possible we don't actually
    // need to do this, but for now we do
    fn cdr(&self) -> Option<UnrolledList<T, P, N, G>> {
        if self.index > 1 {
            Some(UnrolledList(P::new(self.advance_cursor())))
        } else {
            self.next.clone()
        }
    }

    fn advance_cursor(&self) -> Self {
        UnrolledCell {
            index: self.index - 1,
            elements: P::clone(&self.elements),
            next: self.next.clone(),
            size: self.size,
        }
    }

    // TODO make this better
    fn cons_mut(&mut self, value: T) {
        let reference = P::make_mut(&mut self.elements);

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
    fn cons(value: T, mut cdr: UnrolledList<T, P, N, G>) -> UnrolledList<T, P, N, G> {
        let size = cdr.size();

        if cdr.elements().len() > size - 1 {
            UnrolledList(P::new(UnrolledCell {
                index: 1,
                elements: P::new(vec![value]),
                next: Some(cdr),
                size: size * Self::GROWTH_RATE,
            }))
        } else {
            let inner = P::make_mut(&mut cdr.0);
            let elements = P::make_mut(&mut inner.elements);
            inner.index += 1;
            elements.push(value);
            cdr
        }
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> Extend<T>
    for UnrolledList<T, P, N, G>
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.append_mut(iter.into_iter().collect())
    }
}

pub(crate) struct DrainingNodeIter<T: Clone, P: PointerFamily, const N: usize, const G: usize> {
    cur: Option<UnrolledList<T, P, N, G>>,
    _inner: PhantomData<T>,
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> Iterator
    for DrainingNodeIter<T, P, N, G>
{
    type Item = UnrolledList<T, P, N, G>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(_self) = std::mem::take(&mut self.cur) {
            if let Some(next) = _self.0.next.as_ref() {
                // If we can, drop these values!
                if next.strong_count() == 1 && P::strong_count(&next.0.elements) == 1 {
                    self.cur = _self.0.next.clone();

                    // self.cur = std::mem::take(&mut _self.0.next);

                    // std::mem::swap(&mut self.cur, &mut _self.0.next);
                } else {
                    self.cur = None
                }
            } else {
                self.cur = None
            }

            Some(_self)
        } else {
            None
        }
    }
}

pub(crate) struct NodeIter<T: Clone, P: PointerFamily, const N: usize, const G: usize> {
    cur: Option<UnrolledList<T, P, N, G>>,
    _inner: PhantomData<T>,
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> Iterator for NodeIter<T, P, N, G> {
    type Item = UnrolledList<T, P, N, G>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(_self) = std::mem::take(&mut self.cur) {
            self.cur = _self.0.next.clone();
            Some(_self)
        } else {
            None
        }
    }
}

pub(crate) struct NodeIterRef<'a, T: Clone, P: PointerFamily, const N: usize, const G: usize> {
    cur: Option<&'a UnrolledList<T, P, N, G>>,
    _inner: PhantomData<T>,
}

impl<'a, T: Clone, P: PointerFamily, const N: usize, const G: usize> Iterator
    for NodeIterRef<'a, T, P, N, G>
{
    type Item = &'a UnrolledList<T, P, N, G>;
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

pub(crate) struct DrainingConsumingWrapper<
    T: Clone,
    P: PointerFamily,
    const N: usize,
    const G: usize,
>(DrainingConsumingIter<T, P, N, G>);

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> Iterator
    for DrainingConsumingWrapper<T, P, N, G>
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

// TODO have this expose tryfold
pub(crate) struct ConsumingWrapper<T: Clone, P: PointerFamily, const N: usize, const G: usize>(
    ConsumingIter<T, P, N, G>,
);

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> Iterator
    for ConsumingWrapper<T, P, N, G>
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

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> IntoIterator
    for UnrolledList<T, P, N, G>
{
    type Item = T;
    type IntoIter = ConsumingWrapper<T, P, N, G>;

    fn into_iter(self) -> Self::IntoIter {
        ConsumingWrapper(self.into_node_iter().flat_map(move |mut x| {
            let cell = P::make_mut(&mut x.0);
            let vec = P::make_mut(&mut cell.elements);
            let elements = std::mem::take(vec);
            elements.into_iter().take(x.index()).rev()
        }))
    }
}

// TODO have this also expose TryFold
pub(crate) struct IterWrapper<'a, T: Clone, P: PointerFamily, const N: usize, const G: usize>(
    RefIter<'a, T, P, N, G>,
);

impl<'a, T: Clone, P: PointerFamily, const N: usize, const G: usize> Iterator
    for IterWrapper<'a, T, P, N, G>
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

impl<'a, T: Clone, P: PointerFamily, const N: usize, const G: usize> IntoIterator
    for &'a UnrolledList<T, P, N, G>
{
    type Item = &'a T;
    type IntoIter = IterWrapper<'a, T, P, N, G>;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        IterWrapper(
            self.node_iter()
                .flat_map(|x| x.elements()[0..x.index()].iter().rev()),
        )
    }
}

struct ExponentialChunks<I, const N: usize, const G: usize>
where
    I: Iterator,
{
    iter: I,
    size: usize,
    length: usize,
    running_sum: usize,
}

impl<I, const N: usize, const G: usize> ExponentialChunks<I, N, G>
where
    I: Iterator,
{
    fn new(iter: I, length: usize, mut size: usize) -> Self {
        let mut running_sum = size;

        while running_sum < length {
            size *= G;
            running_sum += size;
        }

        Self {
            iter,
            size,
            length,
            running_sum: running_sum - size,
        }
    }
}

impl<I, const N: usize, const G: usize> Iterator for ExponentialChunks<I, N, G>
where
    I: Iterator,
{
    type Item = (usize, Vec<I::Item>);

    fn next(&mut self) -> Option<Self::Item> {
        let chunk_size = if self.length > self.running_sum {
            self.length - self.running_sum
        } else {
            self.size
        };

        // let mut chunk = Vec::with_capacity(chunk_size);
        let mut chunk = Vec::new();
        for item in self.iter.by_ref().take(chunk_size) {
            chunk.push(item);
        }

        if chunk.is_empty() {
            return None;
        }

        let result = chunk;
        let size = self.size;

        self.size /= G;
        self.length -= result.len();

        Some((size, result))
    }
}

fn from_vec<T: Clone, P: PointerFamily, const N: usize, const G: usize>(
    vec: Vec<T>,
) -> UnrolledList<T, P, N, G> {
    let length = vec.len();

    let mut pairs: SmallVec<[UnrolledList<_, _, N, G>; 16]> =
        ExponentialChunks::<_, N, G>::new(vec.into_iter(), length, N)
            .map(|(size, x)| {
                let mut elements = x;
                elements.reverse();

                UnrolledList(P::new(UnrolledCell {
                    index: elements.len(),
                    elements: P::new(elements),
                    next: None,
                    size,
                }))
            })
            .collect();

    let mut rev_iter = (0..pairs.len()).rev();
    rev_iter.next();

    for i in rev_iter {
        let prev = pairs.pop().unwrap();

        if let Some(UnrolledList(cell)) = pairs.get_mut(i) {
            P::get_mut::<UnrolledCell<T, P, N, G>>(cell)
                .expect("Only one owner allowed in construction")
                .next = Some(prev);
        } else {
            unreachable!()
        }
    }

    pairs.pop().unwrap_or_else(UnrolledList::new)
}

// and we'll implement FromIterator
// TODO specialize this for the into version?
impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> FromIterator<T>
    for UnrolledList<T, P, N, G>
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let reversed: Vec<_> = iter.into_iter().collect();
        from_vec(reversed)
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize>
    FromIterator<UnrolledList<T, P, N, G>> for UnrolledList<T, P, N, G>
{
    fn from_iter<I: IntoIterator<Item = UnrolledList<T, P, N, G>>>(iter: I) -> Self {
        // Links up the nodes
        let mut nodes: SmallVec<[_; 16]> = iter.into_iter().collect();

        let mut rev_iter = (0..nodes.len()).rev();
        rev_iter.next();

        for i in rev_iter {
            // TODO need to truncate the front of this one
            let mut prev = nodes.pop().unwrap();

            if let Some(UnrolledList(cell)) = nodes.get_mut(i) {
                // Check if this node can fit entirely into the previous one
                if cell.elements.len() + prev.0.elements.len() <= prev.0.size {
                    let left_inner = P::make_mut(cell);
                    let right_inner = P::make_mut(&mut prev.0);

                    let left_vector = P::make_mut(&mut left_inner.elements);
                    let right_vector = P::make_mut(&mut right_inner.elements);

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
                    P::make_mut(cell).next = Some(prev);
                }
            } else {
                unreachable!()
            }
        }

        nodes.pop().unwrap_or_else(Self::new)
    }
}

impl<'a, T: 'a + Clone, P: 'a + PointerFamily, const N: usize, const G: usize>
    FromIterator<&'a UnrolledList<T, P, N, G>> for UnrolledList<T, P, N, G>
{
    fn from_iter<I: IntoIterator<Item = &'a UnrolledList<T, P, N, G>>>(iter: I) -> Self {
        iter.into_iter().cloned().collect()
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> From<Vec<T>>
    for UnrolledList<T, P, N, G>
{
    fn from(vec: Vec<T>) -> Self {
        from_vec(vec)
    }
}

impl<T: Clone, P: PointerFamily, const N: usize, const G: usize> From<&[T]>
    for UnrolledList<T, P, N, G>
{
    fn from(vec: &[T]) -> Self {
        from_vec(vec.to_vec())
    }
}

#[cfg(test)]
mod tests {

    use crate::shared::RcPointer;

    type RcList<T> = UnrolledList<T, RcPointer, 256>;

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
    use crate::shared::RcPointer;

    const CAPACITY: usize = 256;

    type RcList<T> = UnrolledList<T, RcPointer, 256>;

    #[test]
    fn check_size() {
        println!(
            "{}",
            std::mem::size_of::<UnrolledCell<usize, RcPointer, 256, 256>>()
        );
    }

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

        for node in left.node_iter() {
            println!("{:?}", node.elements().len());
        }

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
mod vlist_iterator_tests {

    use super::*;
    use crate::shared::RcPointer;

    const CAPACITY: usize = 4;

    type RcList<T> = UnrolledList<T, RcPointer, 4, 2>;

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

    #[test]
    fn appending_works_as_expected() {
        let mut list: RcList<usize> = Vec::<usize>::new().into_iter().collect::<RcList<_>>();

        list = list.append(vec![0, 0, 0, 0].into_iter().collect());

        assert_eq!(list.cdr().unwrap().len(), 3);
    }

    #[test]
    fn appending_works_as_expected_overflow() {
        let mut list: RcList<usize> = Vec::<usize>::new().into_iter().collect::<RcList<_>>();

        list = list.append(vec![0, 0, 0, 0, 0].into_iter().collect());

        assert_eq!(list.cdr().unwrap().len(), 4);
    }

    #[test]
    fn append_then_pop_front() {
        let mut list: UnrolledList<usize, RcPointer, 4, 4> = Vec::<usize>::new()
            .into_iter()
            .collect::<UnrolledList<_, _, 4, 4>>();

        list = list.append(
            vec![
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]
            .into_iter()
            .collect(),
        );

        list.pop_front();

        assert_eq!(list.len(), 20);
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
        let mut list: RcList<_> = (0..4).into_iter().collect();

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
        let list: RcList<usize> = vec![1, 2, 3, 4, 5].into();

        let cdr = list.cdr().unwrap();

        let expected_cdr: RcList<usize> = vec![2, 3, 4, 5].into_iter().collect();

        assert_eq!(
            cdr.into_iter().collect::<Vec<_>>(),
            expected_cdr.into_iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn cdr_mut() {
        let mut list: RcList<usize> = vec![1, 2, 3usize].into();
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
        let list: RcList<usize> = (0..2 * CAPACITY * 32).into_iter().collect();
        let next = list.take(100);

        assert!(Iterator::eq(0..100usize, next.into_iter()))
    }

    #[test]
    fn take_big() {
        let list: RcList<usize> = (0..2 * CAPACITY * 32).into_iter().collect();
        let next = list.take(CAPACITY + 100);
        assert!(Iterator::eq(0..CAPACITY + 100usize, next.into_iter()))
    }

    #[test]
    fn tail() {
        let list: RcList<usize> = (0..2 * CAPACITY * 32).into_iter().collect();
        let next = list.tail(CAPACITY + 100).unwrap();

        // println!("next: {:?}", next);
        // println!("original: {:?}", list);
        assert!(Iterator::eq(
            CAPACITY + 100usize..2 * CAPACITY * 32,
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
    use crate::shared::RcPointer;
    type RcList<T> = UnrolledList<T, RcPointer, 256>;

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
