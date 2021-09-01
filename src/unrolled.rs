use crate::shared::{ArcConstructor, RcConstructor, SmartPointer, SmartPointerConstructor};
use itertools::Itertools;
use std::cmp::Ordering;
use std::iter::FromIterator;
use std::marker::PhantomData;

const CAPACITY: usize = 256;

#[derive(Clone, PartialEq)]
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
    pub fn new() -> Self {
        UnrolledList(S::RC::new(UnrolledCell::new()))
    }

    // Get the strong count of the node in question
    fn strong_count(&self) -> usize {
        S::RC::strong_count(&self.0)
    }

    // This is actually like O(n / 64) which is actually quite nice
    // Saves us some time
    pub fn len(&self) -> usize {
        self.node_iter().map(|node| node.elements().len()).sum()
    }

    // Should be O(1) always
    pub fn car(&self) -> Option<T> {
        self.0.car().cloned()
    }

    pub fn cons(value: T, other: Self) -> Self {
        UnrolledCell::cons(value, other)
    }

    // If we can cons in place, do it
    // otherwise, return the reference to the next cell and use that
    // This is a bit unergonomic, but it gives us the option for building something in place
    pub fn cons_mut_in_place(&mut self, value: T) -> Option<UnrolledList<T, C, S>> {
        if self.0.full || self.elements().len() > CAPACITY - 1 {
            Some(UnrolledList(S::RC::new(UnrolledCell {
                index: 1,
                elements: C::RC::new(vec![value]),
                next: Some(self.clone()),
                full: false,
            })))
        } else {
            self.cons_mut(value);
            None
        }
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
        self.0.full || self.0.elements.len() == CAPACITY
    }

    fn does_node_satisfy_invariant(&self) -> bool {
        self.0.full || self.elements().len() <= CAPACITY
    }

    fn assert_list_invariants(&self) {
        assert!(self.does_node_satisfy_invariant())
    }

    pub fn iter<'a>(&'a self) -> IterRef<'a, T, C, S> {
        IterRef {
            cur: Some(self),
            index: self.0.index,
            _inner: PhantomData,
        }
    }

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
        if index < CAPACITY {
            return self.0.elements.get(CAPACITY - index - 1).cloned();
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
        let mut default = UnrolledList::new();
        std::mem::swap(self, &mut default);

        default = default.append(other);
        std::mem::swap(self, &mut default);
    }

    // Functional append
    pub fn append(self, other: Self) -> Self {
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

    fn cons_mut(&mut self, value: T) {
        // self.0.cons_mut(value)

        todo!()
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
pub struct UnrolledCell<
    T: Clone,
    S: SmartPointerConstructor<Self>,
    C: SmartPointerConstructor<Vec<T>>,
> {
    pub index: usize,
    pub elements: C::RC,
    pub next: Option<UnrolledList<T, C, S>>,
    pub full: bool,
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
    pub fn new() -> Self {
        UnrolledCell {
            index: 0,
            elements: C::RC::new(Vec::new()),
            next: None,
            full: false,
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
            self.next.clone()
        }
    }

    fn advance_cursor(&self) -> Self {
        UnrolledCell {
            index: self.index + 1,
            elements: self.elements.clone(),
            next: self.next.clone(),
            full: false,
        }
    }

    // TODO make this better
    pub fn cons_mut(&mut self, value: T) {
        C::make_mut(&mut self.elements).push(value);
        self.index += 1;
    }

    pub fn cons_empty(value: T) -> Self {
        UnrolledCell {
            index: 0,
            elements: C::RC::new(vec![value]),
            next: None,
            full: false,
        }
    }

    pub fn cons_raw(value: T, mut cdr: UnrolledList<T, C, S>) -> UnrolledList<T, C, S> {
        if cdr.0.full || cdr.elements().len() > CAPACITY - 1 {
            UnrolledList(S::RC::new(UnrolledCell {
                index: 1,
                elements: C::RC::new(vec![value]),
                next: Some(cdr),
                full: false,
            }))
        } else {
            cdr.cons_mut(value);
            cdr
        }
    }

    // Spill over the values to a new node
    // otherwise, copy the node and spill over
    pub fn cons(value: T, mut cdr: UnrolledList<T, C, S>) -> UnrolledList<T, C, S> {
        if cdr.0.full || cdr.elements().len() > CAPACITY - 1 {
            UnrolledList(S::RC::new(UnrolledCell {
                index: 1,
                elements: C::RC::new(vec![value]),
                next: Some(cdr),
                full: false,
            }))
        } else {
            let mut inner = S::make_mut(&mut cdr.0);
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

pub struct IterRef<
    'a,
    T: Clone,
    C: SmartPointerConstructor<Vec<T>>,
    S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
> {
    cur: Option<&'a UnrolledList<T, C, S>>,
    index: usize,
    _inner: PhantomData<T>,
}

impl<
        'a,
        T: Clone,
        C: SmartPointerConstructor<Vec<T>>,
        S: SmartPointerConstructor<UnrolledCell<T, S, C>>,
    > Iterator for IterRef<'a, T, C, S>
{
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(_self) = &self.cur {
            if self.index > 0 {
                let return_value = _self.elements().get(self.index - 1);
                self.index -= 1;
                return_value
            } else {
                self.cur = _self.0.next.as_ref();
                self.index = self.cur.as_ref().map(|x| x.elements().len()).unwrap_or(0);
                let ret = self.cur.as_ref().and_then(|x| x.0.car());
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
                self.cur = _self.0.next.clone();
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

        pairs.pop().unwrap()
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
            let mut prev = nodes.pop().unwrap();

            if let Some(UnrolledList(cell)) = nodes.get_mut(i) {
                // Check if this node can fit entirely into the previous one
                if cell.elements.len() + prev.0.elements.len() < CAPACITY {
                    let left_inner = S::make_mut(cell);
                    let right_inner = S::make_mut(&mut prev.0);

                    // println!(
                    //     "Strong count of left vector: {}",
                    //     C::RC::strong_count(&left_inner.elements)
                    // );
                    // println!(
                    //     "String count of right vector: {}",
                    //     C::RC::strong_count(&right_inner.elements)
                    // );

                    let left_vector = C::make_mut(&mut left_inner.elements);
                    let right_vector = C::make_mut(&mut right_inner.elements);

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

        nodes.pop().unwrap()
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

pub type RcList<T> = UnrolledList<T, RcConstructor, RcConstructor>;
pub type ArcList<T> = UnrolledList<T, ArcConstructor, ArcConstructor>;

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
        left = left.append(right.clone());

        for item in left {
            println!("Iterating: {}", item);
        }

        for item in right {
            println!("Iterating: {}", item)
        }
    }

    #[test]
    fn append_large() {
        let mut left: RcList<_> = (0..60).into_iter().collect();
        let right: RcList<_> = (60..100).into_iter().collect();

        left = left.append(right);

        left.assert_invariants();

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

        // for node in left.node_iter() {
        //     println!("elements in node: {:?}", node.elements());
        // }

        // 400
        let right: RcList<_> = (CAPACITY + 100..CAPACITY + 500).into_iter().collect();

        // println!("{:?}", right);
        left = left.append(right);

        for node in left.node_iter() {
            println!("Node elements: {}", node.elements().len())
        }

        assert_eq!(left.get(300).unwrap(), 300);

        // let new = left
        //     .node_iter()
        //     .into_iter()
        //     .chain(right.node_iter())
        //     .collect::<RcList<usize>>();

        // println!("new list: {:?}", new);
        // println!("new list length: {:?}", new.len());

        left.assert_list_invariants();

        println!("length: {}", left.len());

        println!("{:?}", left);
    }

    #[test]
    fn length() {
        let list: RcList<_> = (0..300).into_iter().collect();
        assert_eq!(list.len(), 300);

        println!("list: {:?}", list);
    }

    #[test]
    fn indexing() {
        let list: RcList<_> = (0..300).into_iter().collect();

        for i in 0..300 {
            assert_eq!(list.get(i).unwrap(), i);
        }
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
