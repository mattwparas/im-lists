#[cfg(test)]
mod proptests;

use crate::shared_vector::{AtomicSharedVector, Vector};
use smallvec::SmallVec;

use crate::shared::PointerFamily;

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::iter::{FlatMap, FromIterator, Rev};
use core::marker::PhantomData;
use core::mem::ManuallyDrop;

type ConsumingIter<T, P, const N: u32, const G: u32> = FlatMap<
    NodeIter<T, P, N, G>,
    // Rev<core::iter::Take<alloc::vec::IntoIter<T>>>,
    MaybeCloned<T, N, G>,
    fn(UnrolledList<T, P, N, G>) -> MaybeCloned<T, N, G>, // Rev<core::iter::Take<alloc::vec::IntoIter<T>>>,
>;

type MaybeCloned<T, const N: u32, const G: u32> =
    Rev<core::iter::Take<crate::shared_vector::IntoIter<T>>>;

type RefIter<'a, T, P, const N: u32, const G: u32> = FlatMap<
    NodeIterRef<'a, T, P, N, G>,
    Rev<core::slice::Iter<'a, T>>,
    fn(&'a UnrolledCell<T, P, N, G>) -> Rev<core::slice::Iter<'a, T>>,
>;

type DrainingConsumingIter<T, P, const N: u32, const G: u32> = FlatMap<
    DrainingNodeIter<T, P, N, G>,
    CellDrain<T>,
    fn(UnrolledList<T, P, N, G>) -> CellDrain<T>,
>;

/// The values drained from a single cell. Inline cells hand back their one
/// value directly
pub enum CellDrain<T> {
    One(Option<T>),
    Chunk(Rev<core::iter::Take<crate::shared_vector::IntoIter<T>>>),
}

impl<T> Iterator for CellDrain<T> {
    type Item = T;

    #[inline(always)]
    fn next(&mut self) -> Option<T> {
        match self {
            CellDrain::One(value) => value.take(),
            CellDrain::Chunk(iter) => iter.next(),
        }
    }
}

// Without std there are no thread locals to share the empty cell from,
// so allocate a new one each time
#[cfg(not(feature = "std"))]
fn empty_list<T: Clone, P: PointerFamily, const N: u32, const G: u32>(
) -> P::Pointer<UnrolledCell<T, P, N, G>> {
    P::new(UnrolledCell::new())
}

#[cfg(feature = "std")]
fn empty_list<T: Clone, P: PointerFamily, const N: u32, const G: u32>(
) -> P::Pointer<UnrolledCell<T, P, N, G>>
where
    P::Pointer<UnrolledCell<T, P, N, G>>: 'static,
{
    let mut output = None;
    generic_singleton::get_or_init_thread_local!(
        || P::new(UnrolledCell::new()),
        |cell: &P::Pointer<UnrolledCell<T, P, N, G>>| {
            output = Some(P::clone(cell));
        }
    );

    output.unwrap()
}

#[derive(Eq)]
#[repr(transparent)]
pub struct UnrolledList<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32>(
    pub(crate) P::Pointer<UnrolledCell<T, P, N, G>>,
);

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> Clone for UnrolledList<T, P, N, G> {
    fn clone(&self) -> Self {
        Self(P::clone(&self.0))
    }
}

// Check if these lists are equivalent via the iterator
impl<T: Clone + PartialEq, P: PointerFamily, const N: u32, const G: u32> PartialEq
    for UnrolledList<T, P, N, G>
{
    fn eq(&self, other: &Self) -> bool {
        Iterator::eq(self.iter(), other.iter())
    }
}

impl<T: Clone + PartialOrd, P: PointerFamily, const N: u32, const G: u32> PartialOrd
    for UnrolledList<T, P, N, G>
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> Default for UnrolledList<T, P, N, G> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> UnrolledList<T, P, N, G> {
    pub fn new() -> Self {
        UnrolledList(empty_list::<T, P, N, G>())
    }

    pub fn as_ptr(&self) -> *const UnrolledCell<T, P, N, G> {
        P::as_ptr(&self.0)
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
        // P::ptr_eq(&self.0.elements, &other.0.elements) && self.0.index == other.0.index
        self.0.elements_ptr_eq(&other.0) && self.0.index() == other.0.index()
    }

    pub fn as_ptr_usize(&self) -> usize {
        P::as_ptr(&self.0) as usize
    }

    pub fn elements_as_ptr_usize(&self) -> usize {
        // P::as_ptr(&self.0.elements) as usize
        self.0.elements_as_ptr() as usize
    }

    pub fn next_ptr_as_usize(&self) -> Option<usize> {
        self.0.next.addr()
    }

    pub fn current_node_iter(&self) -> impl Iterator<Item = &T> {
        self.0.elements_iter().take(self.index()).rev()
    }

    pub fn draining_iterator(self) -> DrainingConsumingWrapper<T, P, N, G> {
        DrainingConsumingWrapper(self.into_draining_node_iter().flat_map(|x| {
            let index = x.index();

            P::try_unwrap(x.0)
                .map(|mut cell| {
                    if let Some(value) = cell.take_inline() {
                        CellDrain::One((index >= 1).then_some(value))
                    } else if cell.elements_is_unique() {
                        CellDrain::Chunk(
                            core::mem::take(cell.buffer_mut())
                                .into_unique()
                                .into_iter()
                                .take(index as _)
                                .rev(),
                        )
                    } else {
                        CellDrain::One(None)
                    }
                })
                .unwrap_or(CellDrain::One(None))
        }))
    }

    // #[cfg(test)]
    pub fn cell_count(&self) -> usize {
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
            // let elements_mut = P::make_mut(&mut inner.elements);

            if inner.index() < inner.elements_len() {
                // elements_mut.truncate(inner.index);
                inner.elements_truncate(inner.index());
            }

            // elements_mut.reverse();
            inner.elements_reverse();
            inner.next.set(None);
        }

        for mut right in node_iter {
            let cell = P::make_mut(&mut right.0);
            // let elements_mut = P::make_mut(&mut cell.elements);

            if cell.index() < cell.elements_len() {
                // elements_mut.truncate(cell.index);

                cell.elements_truncate(cell.index());
            }

            // elements_mut.reverse();
            cell.elements_reverse();
            cell.next.set(Some(left));
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
        if count > self.0.index() && self.0.next.is_none() {
            return self.clone();
        }

        for mut node in self.clone().into_node_iter() {
            if count < node.0.index() {
                let inner = P::make_mut(&mut node.0);
                // this is the new tail, point to the end
                inner.next.set(None);

                // We want to chop off whatever we need to
                // let elements_mut = P::make_mut(&mut inner.elements);

                inner.elements_ensure_unique();

                // Grab the end of the vector, this will be the new backing
                // let remaining = elements_mut.split_off(inner.index - count);

                let at = inner.index() - count;
                let remaining = split_off(inner.buffer_mut(), at);

                inner.set_index(count);

                // *elements_mut = remaining;

                *inner.buffer_mut() = remaining;

                nodes.push(node);
                break;
            } else {
                // Note: We might want to truncate the remaining
                // elements of the vector.
                count -= node.0.index();

                // Landed exactly on the end of this node, so it's the new tail.
                // Carrying on would add an empty node to the end
                if count == 0 {
                    P::make_mut(&mut node.0).next.set(None);
                    nodes.push(node);
                    break;
                }

                nodes.push(node);
            }
        }

        let mut rev_iter = (0..nodes.len()).rev();
        rev_iter.next();

        for i in rev_iter {
            let prev = nodes.pop().unwrap();

            if let Some(UnrolledList(cell)) = nodes.get_mut(i) {
                P::make_mut(cell).next.set(Some(prev));
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
            if len < node.0.index() {
                let inner = P::make_mut(&mut node.0);
                // this is the new tail, point to the end
                // inner.next = None;
                inner.set_index(inner.index() - len);
                return Some(node);
            } else {
                len -= node.0.index();
            }
        }

        if len == 0 {
            return Some(Self::new());
        }

        None
    }

    /// Alias for cons_mut
    pub fn push_front(&mut self, value: T) {
        self.cons_mut(value)
    }

    pub fn cons_mut(&mut self, value: T) {
        let index = self.0.index();
        let mut elements_len = self.elements().len();
        let size = self.size();

        // This is saying: If we are pointing to a cell where the offset
        // has been moved to the right but the underlying data has not
        // yet been truncated, we should attempt to eagerly do so if we own
        // the cell, otherwise we should fall back to a new node.
        let already_unique: Option<&mut UnrolledCell<T, P, N, G>> = if index < elements_len {
            // Consing onto the tail of a longer list. If someone else can see the
            // rest of the buffer, link to it from a new cell instead of copying
            if !self.0.elements_is_unique() {
                self.push_inline_cell(value);
                return;
            }
            // Only truncate if we own the cell - `make_mut` would copy the whole
            // chunk. Otherwise fall through to allocating a new node
            match P::get_mut(&mut self.0) {
                Some(cell) => {
                    cell.elements_truncate(index);
                    elements_len = index;
                    Some(cell)
                }
                None => None,
            }
        } else {
            None
        };

        // TODO cdr here is an issue - only moves the offset, no way to know that its full
        // Cause its not actually full
        if elements_len > size as usize - 1 {
            // Full - start a new cell
            self.push_inline_cell(value);
        } else {
            // Push in place if we own both the cell and its buffer, otherwise
            // start a new cell
            let inner = match already_unique {
                Some(inner) => Some(inner),
                None => P::get_mut(&mut self.0),
            };

            match inner {
                Some(inner) => match inner.elements_try_push(value) {
                    Ok(_) => {
                        inner.set_index(inner.index() + 1);
                    }
                    Err(v) => {
                        self.push_inline_cell(v);
                    }
                },
                None => {
                    self.push_inline_cell(value);
                }
            }
        }
    }

    // Put `value` in a new cell in front of this list. New cells start inline,
    // see `deinline`
    fn push_inline_cell(&mut self, value: T) {
        // Swap the new cell in rather than going through `Self::new()`, which
        // costs a thread-local lookup
        let mut rest = UnrolledList(P::new(UnrolledCell::new_inline(value, None)));
        core::mem::swap(self, &mut rest);

        // Don't link to an empty list
        if !(rest.is_empty() && rest.0.next.is_none()) {
            P::get_mut(&mut self.0)
                .expect("A new cell has only one owner")
                .next
                .set(Some(rest));
        }
    }

    // Should be O(1) always
    // Should also not have to clone
    pub fn cdr(&self) -> Option<UnrolledList<T, P, N, G>> {
        self.0.cdr()
    }

    // Remove the first value of the list and return it
    pub fn pop_front(&mut self) -> Option<T> {
        // If we own the values, move the first one out
        if let Some(cell) = P::get_mut(&mut self.0) {
            if let Some(value) = cell.take_inline() {
                let next = cell.next.take();
                *self = next.unwrap_or_default();
                return Some(value);
            }

            if cell.elements_is_unique() {
                let index = cell.index();
                if index == 0 {
                    return None;
                }

                // The cursor may be behind the end of the buffer, so drop
                // anything past it before popping
                cell.elements_truncate(index);
                let ret = cell.elements_pop();
                cell.set_index(index - 1);

                // If after we've popped, its empty, move the pointer to the
                // next one (if there is one)
                if index == 1 {
                    if let Some(next) = cell.next.take() {
                        *self = next;
                    }
                }

                return ret;
            }
        }

        // Otherwise someone else can see the values, so clone the first one
        // and move the cursor past it
        let ret = self.car();
        if ret.is_some() {
            self.cdr_mut();
        }
        ret
    }

    // Raw pointer to the next cell, with the tag bits masked off
    pub(crate) fn next_cell_ptr(&self) -> Option<*const UnrolledCell<T, P, N, G>> {
        let ptr = self.0.next.ptr();
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }

    pub(crate) fn cdr_exists(&self) -> bool {
        self.0.index() > 1 || self.0.next.is_some()
    }

    // Move the cursor to `index` within the current cell. Copies the cell
    // if it's shared
    pub(crate) fn set_cursor(&mut self, index: usize) {
        if self.0.index() == index {
            return;
        }

        // Inline cells only have the one position
        debug_assert!(
            !self.0.next.is_inline(),
            "an inline cell has only one cursor position"
        );

        let cell = P::make_mut(&mut self.0);
        cell.set_index(index);
    }

    // Walk forward to the cell at `ptr` and set its cursor to `index`, like
    // repeated `cdr_mut`. Returns the partially walked list as `Err` if we hit
    // a shared cell first - it still keeps the target alive
    pub(crate) fn reposition(
        mut self,
        ptr: *const UnrolledCell<T, P, N, G>,
        index: usize,
    ) -> Result<Self, Self> {
        loop {
            if P::as_ptr(&self.0) == ptr {
                self.set_cursor(index);
                return Ok(self);
            }
            let next = match P::get_mut(&mut self.0) {
                Some(inner) => inner.next.take(),
                None => return Err(self),
            };
            match next {
                Some(next) => self = next,
                // `ptr` wasn't in this list
                None => return Err(Self::new()),
            }
        }
    }

    // Returns the cdr of the list
    // Returns None if the next is empty - otherwise updates self to be the rest
    pub fn cdr_mut(&mut self) -> Option<&mut Self> {
        if self.0.index() > 1 {
            // This will allocate a new cell
            let cell = P::make_mut(&mut self.0);
            cell.set_index(cell.index() - 1);
            Some(self)
        } else {
            let inner = P::get_mut(&mut self.0);

            match inner {
                Some(inner) => {
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
                None => match self.0.next.clone_list() {
                    Some(x) => {
                        *self = x;
                        Some(self)
                    }
                    None => {
                        *self = Self::new();
                        None
                    }
                },
            }
        }
    }

    pub(crate) fn elements(&self) -> &[T] {
        self.0.elements()
    }

    fn size(&self) -> u32 {
        self.0.size()
    }

    #[cfg(test)]
    fn does_node_satisfy_invariant(&self) -> bool {
        self.elements().len() <= self.size() as usize
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
            cur: Some(&self.0),
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
    pub fn assert_invariants(&self) {
        for node in self.node_iter() {
            assert!(
                node.elements_len() <= node.size() as usize,
                "node over capacity"
            );
            assert!(
                node.index() <= node.elements_len(),
                "cursor past the end of the node"
            );
        }

        // Only an empty list gets to have an empty node
        if self.0.next.is_some() {
            assert!(
                self.node_iter().all(|node| node.index() > 0),
                "empty node in the chain"
            );
        }
    }

    pub fn get(&self, mut index: usize) -> Option<&T> {
        if index < self.0.index() {
            self.0.elements_get(self.0.index() - index - 1)
        } else {
            let mut cur = self.0.next.cell();
            index -= self.0.index();

            while let Some(node) = cur {
                if index < node.index() {
                    let node_cap = node.index();
                    return node.elements_get(node_cap - index - 1);
                } else {
                    cur = node.next.cell();
                    index -= node.index();
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
        core::mem::swap(self, &mut default);

        default = default.append(other);
        core::mem::swap(self, &mut default);
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
        let list = core::mem::take(self);
        let mut vec = list.into_iter().collect::<Vec<_>>();
        vec.sort_by(cmp);
        *self = vec.into();
    }

    // Append a single value to the end
    pub fn push_back(&mut self, value: T) {
        self.extend(core::iter::once(value))
    }

    pub fn is_empty(&self) -> bool {
        self.0.elements_is_empty() || self.0.index() == 0
    }

    pub fn index(&self) -> usize {
        self.0.index()
    }
}

// Don't blow the stack
impl<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32> Drop
    for UnrolledCell<T, P, N, G>
{
    fn drop(&mut self) {
        // Unions don't drop their fields, so drop whichever one is live
        unsafe {
            if !self.next.is_inline() {
                // The buffer is `ManuallyDrop` too, so drop it directly. Draining
                // leaves an empty buffer behind, so this is always valid
                ManuallyDrop::drop(&mut (*self.storage.chunk).buffer);
            } else if !self.next.is_moved() {
                ManuallyDrop::drop(&mut self.storage.inline);
            }
        }

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

// The link to the next cell. The low bits hold which arm of the cell's
// storage is live, and whether an inline value has been moved out.
// These have to live in the cell itself, since cells moved out by
// `P::try_unwrap` still need to know what to drop
#[repr(transparent)]
pub(crate) struct Link<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32> {
    raw: *const UnrolledCell<T, P, N, G>,
    // Otherwise `P` is only used recursively, which the compiler rejects
    _marker: PhantomData<fn() -> P>,
}

// Tag bits in `Link`:
// - bits 0..=1: which arm of the storage is live. Bit 1 is unused for now
// - bit 2: an inline value has been moved out
// `UnrolledCell` is aligned to 8, so these are free on every target
pub(crate) const VARIANT_CHUNK: usize = 0;
pub(crate) const VARIANT_INLINE: usize = 1;
pub(crate) const VARIANT_MASK: usize = 3;

const MOVED_TAG: usize = 4;
const TAG_MASK: usize = VARIANT_MASK | MOVED_TAG;

// Exposed for generated code, see `GenericList::cell_inline_flag`
pub(crate) const INLINE_TAG: usize = VARIANT_INLINE;
pub(crate) const LINK_TAG_MASK: usize = TAG_MASK;

impl<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32> Link<T, P, N, G> {
    #[inline(always)]
    fn pack(ptr: *const UnrolledCell<T, P, N, G>, variant: usize) -> Self {
        const {
            assert!(
                core::mem::align_of::<UnrolledCell<T, P, N, G>>() > TAG_MASK,
                "cell alignment must leave the tag bits free"
            )
        };
        debug_assert!(
            ptr.addr() & TAG_MASK == 0,
            "cell pointers are aligned, so the low bits are ours"
        );
        Self {
            raw: ptr.map_addr(|addr| addr | variant),
            _marker: PhantomData,
        }
    }

    #[inline(always)]
    pub(crate) fn empty(variant: usize) -> Self {
        Self::pack(core::ptr::null(), variant)
    }

    #[inline(always)]
    pub(crate) fn new(list: Option<UnrolledList<T, P, N, G>>, variant: usize) -> Self {
        match list {
            Some(list) => Self::pack(P::into_raw(list.0), variant),
            None => Self::empty(variant),
        }
    }

    #[inline(always)]
    pub(crate) fn variant(&self) -> usize {
        self.raw.addr() & VARIANT_MASK
    }

    #[inline(always)]
    pub(crate) fn is_inline(&self) -> bool {
        self.variant() == VARIANT_INLINE
    }

    #[inline(always)]
    pub(crate) fn set_variant(&mut self, variant: usize) {
        self.raw = self.raw.map_addr(|addr| (addr & !VARIANT_MASK) | variant);
    }

    #[inline(always)]
    fn ptr(&self) -> *const UnrolledCell<T, P, N, G> {
        self.raw.map_addr(|addr| addr & !TAG_MASK)
    }

    #[inline(always)]
    pub(crate) fn is_moved(&self) -> bool {
        self.raw.addr() & MOVED_TAG != 0
    }

    #[inline(always)]
    pub(crate) fn set_moved(&mut self, on: bool) {
        self.raw = self.raw.map_addr(|addr| {
            if on {
                addr | MOVED_TAG
            } else {
                addr & !MOVED_TAG
            }
        });
    }

    #[inline(always)]
    pub(crate) fn is_some(&self) -> bool {
        !self.ptr().is_null()
    }

    #[inline(always)]
    pub(crate) fn is_none(&self) -> bool {
        self.ptr().is_null()
    }

    // Borrow the next cell
    #[inline(always)]
    pub(crate) fn cell(&self) -> Option<&UnrolledCell<T, P, N, G>> {
        unsafe { self.ptr().as_ref() }
    }

    // Borrow the next cell as a list, without touching the count
    #[inline(always)]
    pub(crate) fn borrow(&self) -> Option<ManuallyDrop<UnrolledList<T, P, N, G>>> {
        let ptr = self.ptr();
        if ptr.is_null() {
            None
        } else {
            Some(ManuallyDrop::new(UnrolledList(unsafe { P::from_raw(ptr) })))
        }
    }

    // Clone the link, keeping the tags
    #[inline(always)]
    pub(crate) fn clone_link(&self) -> Self {
        let mut cloned = Self::new(self.clone_list(), self.variant());
        cloned.set_moved(self.is_moved());
        cloned
    }

    #[inline(always)]
    pub(crate) fn clone_list(&self) -> Option<UnrolledList<T, P, N, G>> {
        self.borrow().map(|list| UnrolledList(P::clone(&list.0)))
    }

    // Take the next cell, leaving the tags in place
    #[inline(always)]
    pub(crate) fn take(&mut self) -> Option<UnrolledList<T, P, N, G>> {
        let ptr = self.ptr();
        self.raw = core::ptr::without_provenance(self.raw.addr() & TAG_MASK);
        if ptr.is_null() {
            None
        } else {
            Some(UnrolledList(unsafe { P::from_raw(ptr) }))
        }
    }

    #[inline(always)]
    pub(crate) fn set(&mut self, list: Option<UnrolledList<T, P, N, G>>) {
        let variant = self.variant();
        drop(self.take());
        *self = Self::new(list, variant);
    }

    pub(crate) fn addr(&self) -> Option<usize> {
        let ptr = self.ptr();
        if ptr.is_null() {
            None
        } else {
            Some(ptr as usize)
        }
    }
}

// A `Link` is just a `P::Pointer`, so it's as `Send`/`Sync` as one
unsafe impl<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32> Send
    for Link<T, P, N, G>
where
    P::Pointer<UnrolledCell<T, P, N, G>>: Send,
{
}

unsafe impl<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32> Sync
    for Link<T, P, N, G>
where
    P::Pointer<UnrolledCell<T, P, N, G>>: Sync,
{
}

impl<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32> Drop for Link<T, P, N, G> {
    fn drop(&mut self) {
        drop(self.take());
    }
}

// A cell with a single element stores it inline, saving the allocation of
// a buffer. Which arm is live is tracked in the tag bits of `next`
#[repr(C)]
pub(crate) union CellStorage<T: Clone + 'static> {
    pub(crate) inline: ManuallyDrop<T>,
    pub(crate) chunk: ManuallyDrop<ChunkHead<T>>,
}

// The cursor, capacity and buffer of a chunked cell
#[repr(C)]
pub(crate) struct ChunkHead<T: Clone + 'static> {
    pub(crate) index: u32,
    pub(crate) size: u32,
    pub(crate) buffer: ManuallyDrop<AtomicSharedVector<T>>,
}

// Aligned to 8 so `Link` has 3 tag bits on 32 bit targets too
#[repr(align(8))]
pub struct UnrolledCell<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32> {
    pub(crate) storage: CellStorage<T>,
    pub(crate) next: Link<T, P, N, G>,
}

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> Clone for UnrolledCell<T, P, N, G> {
    fn clone(&self) -> Self {
        let storage = unsafe {
            if self.next.is_inline() {
                CellStorage {
                    inline: ManuallyDrop::new((*self.storage.inline).clone()),
                }
            } else {
                CellStorage {
                    chunk: ManuallyDrop::new(ChunkHead {
                        index: self.storage.chunk.index,
                        size: self.storage.chunk.size,
                        buffer: ManuallyDrop::new((*self.storage.chunk.buffer).clone()),
                    }),
                }
            }
        };
        Self {
            storage,
            next: self.next.clone_link(),
        }
    }
}

impl<T: Clone + core::fmt::Debug, P: PointerFamily, const N: u32, const G: u32> core::fmt::Debug
    for UnrolledList<T, P, N, G>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> UnrolledCell<T, P, N, G> {
    const GROWTH_RATE: u32 = if G == 0 { 1 } else { G };

    #[inline(always)]
    pub(crate) fn size(&self) -> u32 {
        if self.is_inline() {
            // Room for one more before it has to move to a chunk
            2
        } else {
            unsafe { self.storage.chunk.size }
        }
    }

    #[inline(always)]
    pub(crate) fn index(&self) -> usize {
        if self.is_inline() {
            1
        } else {
            unsafe { self.storage.chunk.index as usize }
        }
    }

    #[inline(always)]
    pub(crate) fn set_index(&mut self, index: usize) {
        if self.is_inline() {
            // Inline cells are always at 1 - `tail` can set that on a boundary
            debug_assert_eq!(index, 1, "an inline cell holds exactly one element");
            return;
        }
        unsafe { (*self.storage.chunk).index = index as u32 };
    }

    #[inline(always)]
    pub(crate) fn set_size(&mut self, size: u32) {
        debug_assert!(!self.is_inline());
        unsafe { (*self.storage.chunk).size = size };
    }

    #[inline(always)]
    fn chunk(&self) -> &ChunkHead<T> {
        debug_assert!(!self.is_inline());
        unsafe { &self.storage.chunk }
    }

    #[inline(always)]
    pub(crate) fn is_inline(&self) -> bool {
        self.next.is_inline()
    }

    #[inline(always)]
    pub(crate) fn buffer(&self) -> &AtomicSharedVector<T> {
        &self.chunk().buffer
    }

    // Move an inline value into a buffer, so that it can be mutated
    #[cold]
    fn deinline(&mut self) {
        debug_assert!(self.next.is_inline() && !self.next.is_moved());
        // A second value is on the way, so make room for exactly two
        let cap = 2;
        // The capacity target still grows from the previous cell, otherwise
        // every cell would stop at two elements
        let target = self
            .next
            .cell()
            .map(|cell| cell.size().saturating_mul(Self::GROWTH_RATE))
            .unwrap_or(N)
            .max(cap as u32);
        // Allocate before moving the value out. If the allocation panics the
        // cell is still inline, and `Drop` would drop the value a second time
        let mut buffer = AtomicSharedVector::with_capacity(cap);
        let value = unsafe { ManuallyDrop::take(&mut self.storage.inline) };
        buffer.push(value);
        self.storage = CellStorage {
            chunk: ManuallyDrop::new(ChunkHead {
                index: 1,
                size: target,
                buffer: ManuallyDrop::new(buffer),
            }),
        };
        self.next.set_variant(VARIANT_CHUNK);
    }

    #[inline(always)]
    pub(crate) fn buffer_mut(&mut self) -> &mut AtomicSharedVector<T> {
        if self.is_inline() {
            self.deinline();
        }
        unsafe { &mut (*self.storage.chunk).buffer }
    }

    // Move the inline value out, marking it so `Drop` skips it
    fn take_inline(&mut self) -> Option<T> {
        if !self.next.is_inline() || self.next.is_moved() {
            return None;
        }
        self.next.set_moved(true);
        Some(unsafe { ManuallyDrop::take(&mut self.storage.inline) })
    }

    #[inline(always)]
    fn inline_ref(&self) -> &T {
        debug_assert!(self.is_inline());
        unsafe { &self.storage.inline }
    }

    #[inline(always)]
    pub(crate) fn elements_len(&self) -> usize {
        if self.is_inline() {
            1
        } else {
            self.buffer().len()
        }
    }

    #[inline(always)]
    pub(crate) fn elements_get(&self, i: usize) -> Option<&T> {
        if self.is_inline() {
            (i == 0).then(|| self.inline_ref())
        } else {
            self.buffer().get(i)
        }
    }

    #[inline(always)]
    pub(crate) fn elements_is_empty(&self) -> bool {
        !self.is_inline() && self.buffer().is_empty()
    }

    #[inline(always)]
    pub(crate) fn elements_iter(&self) -> core::slice::Iter<'_, T> {
        self.elements().iter()
    }

    #[inline(always)]
    pub(crate) fn elements(&self) -> &[T] {
        if self.is_inline() {
            core::slice::from_ref(self.inline_ref())
        } else {
            self.buffer()
        }
    }

    #[inline(always)]
    pub(crate) fn elements_is_unique(&self) -> bool {
        // An inline value is owned by the cell
        self.is_inline() || self.buffer().is_unique()
    }

    #[inline(always)]
    pub(crate) fn elements_as_ptr(&self) -> *const T {
        if self.is_inline() {
            self.inline_ref() as *const T
        } else {
            self.buffer().as_ptr()
        }
    }

    #[inline(always)]
    pub(crate) fn elements_try_push(&mut self, value: T) -> Result<(), T> {
        self.buffer_mut().try_push(value)
    }

    #[inline(always)]
    pub(crate) fn elements_push(&mut self, value: T) {
        self.buffer_mut().push(value)
    }

    #[inline(always)]
    pub(crate) fn elements_pop(&mut self) -> Option<T> {
        self.buffer_mut().pop()
    }

    pub(crate) fn elements_truncate(&mut self, len: usize) {
        if self.is_inline() && len >= 1 {
            return;
        }
        truncate(self.buffer_mut(), len);
    }

    #[inline(always)]
    pub(crate) fn elements_reverse(&mut self) {
        if !self.is_inline() {
            self.buffer_mut().reverse()
        }
    }

    #[inline(always)]
    pub(crate) fn elements_ensure_unique(&mut self) {
        if !self.is_inline() {
            self.buffer_mut().ensure_unique()
        }
    }

    #[inline(always)]
    pub(crate) fn elements_ptr_eq(&self, other: &Self) -> bool {
        if self.is_inline() || other.is_inline() {
            return self.elements_as_ptr() == other.elements_as_ptr();
        }
        self.buffer().ptr_eq(other.buffer())
    }

    fn new() -> Self {
        Self::new_chunk(0, N, AtomicSharedVector::new(), None)
    }

    #[inline(always)]
    fn new_chunk(
        index: u32,
        size: u32,
        buffer: AtomicSharedVector<T>,
        next: Option<UnrolledList<T, P, N, G>>,
    ) -> Self {
        UnrolledCell {
            storage: CellStorage {
                chunk: ManuallyDrop::new(ChunkHead {
                    index,
                    size,
                    buffer: ManuallyDrop::new(buffer),
                }),
            },
            next: Link::new(next, VARIANT_CHUNK),
        }
    }

    #[inline(always)]
    fn new_inline(value: T, next: Option<UnrolledList<T, P, N, G>>) -> Self {
        UnrolledCell {
            storage: CellStorage {
                inline: ManuallyDrop::new(value),
            },
            next: Link::new(next, VARIANT_INLINE),
        }
    }

    fn new_with_capacity() -> Self {
        Self::new_chunk(0, N, AtomicSharedVector::with_capacity(N as _), None)
    }

    // Speed this up by fixing the indexing
    fn car(&self) -> Option<&T> {
        if self.index() == 0 {
            return None;
        }
        self.elements_get(self.index() - 1)
    }

    // This _does_ create a boxed representation of the next item. Its possible we don't actually
    // need to do this, but for now we do
    fn cdr(&self) -> Option<UnrolledList<T, P, N, G>> {
        if self.index() > 1 {
            Some(UnrolledList(P::new(self.advance_cursor())))
        } else {
            self.next.clone_list()
        }
    }

    fn advance_cursor(&self) -> Self {
        // index > 1, so this can't be an inline cell
        debug_assert!(!self.is_inline());
        let mut cell = Self::new_chunk(
            (self.index() - 1) as u32,
            self.size(),
            self.buffer().clone(),
            None,
        );
        cell.next = self.next.clone_link();
        cell
    }

    // Spill over the values to a new node
    // otherwise, copy the node and spill over
    fn cons(value: T, mut cdr: UnrolledList<T, P, N, G>) -> UnrolledList<T, P, N, G> {
        let size = cdr.size();

        let shared = P::strong_count(&cdr.0) > 1;
        // Consing onto the tail of a longer list someone else can see, link to
        // it instead of copying
        let behind_frontier = cdr.0.index() < cdr.elements().len() && !cdr.0.elements_is_unique();

        if shared || behind_frontier || cdr.elements().len() > size as usize - 1 {
            // Don't link to an empty list
            let next = if cdr.is_empty() { None } else { Some(cdr) };
            UnrolledList(P::new(Self::new_inline(value, next)))
        } else {
            let inner = P::make_mut(&mut cdr.0);

            // Drop anything past the cursor first. Then push before bumping the
            // index, since the push is what moves an inline cell into a chunk
            let idx = inner.index();
            if idx < inner.elements().len() {
                inner.elements_truncate(idx);
            }
            inner.elements_push(value);
            inner.set_index(inner.index() + 1);
            cdr
        }
    }
}

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> Extend<T>
    for UnrolledList<T, P, N, G>
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.append_mut(iter.into_iter().collect())
    }
}

pub(crate) struct DrainingNodeIter<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32>
{
    cur: Option<UnrolledList<T, P, N, G>>,
    _inner: PhantomData<T>,
}

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> Iterator
    for DrainingNodeIter<T, P, N, G>
{
    type Item = UnrolledList<T, P, N, G>;
    fn next(&mut self) -> Option<Self::Item> {
        // This is doing allocation. Don't want that.

        if let Some(mut _self) = core::mem::take(&mut self.cur) {
            if let Some(next) = _self.0.next.borrow() {
                // If we can, drop these values!
                // if next.strong_count() == 1 && P::strong_count(&next.0.elements) == 1 {
                if P::strong_count(&next.0) == 1 && next.0.elements_is_unique() {
                    // self.cur = _self.0.next.clone();
                    self.cur = P::get_mut(&mut _self.0).and_then(|x| x.next.take());
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

        // let mut _self = &mut self.cur;
        // let mut ret = None;

        // if let Some(next) = _self.as_mut().and_then(|x| x.0.next.as_ref()) {
        //     // If the next is empty, then we don't want to point to it?
        //     if next.is_empty() {
        //         let mut value = None;
        //         core::mem::swap(&mut self.cur, &mut value);
        //         return value;
        //     }

        //     // If we can, drop these values!
        //     if next.strong_count() == 1 && P::strong_count(&next.0.elements) == 1 {
        //         // self.cur = _self.0.next.clone();
        //         // self.cur = P::get_mut(&mut _self.0).and_then(|x| x.next.take());
        //         // todo!()

        //         let mut value = _self
        //             .as_mut()
        //             .and_then(|x| P::get_mut(&mut x.0).and_then(|x| x.next.take()));

        //         core::mem::swap(&mut self.cur, &mut value);

        //         ret = value
        //     } else {
        //         self.cur = None
        //     }
        // } else {
        //     self.cur = None
        // }

        // ret
    }
}

pub(crate) struct NodeIter<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32> {
    cur: Option<UnrolledList<T, P, N, G>>,
    _inner: PhantomData<T>,
}

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> Iterator for NodeIter<T, P, N, G> {
    type Item = UnrolledList<T, P, N, G>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(_self) = core::mem::take(&mut self.cur) {
            self.cur = _self.0.next.clone_list();
            Some(_self)
        } else {
            None
        }
    }
}

pub(crate) struct NodeIterRef<'a, T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32>
{
    cur: Option<&'a UnrolledCell<T, P, N, G>>,
    _inner: PhantomData<T>,
}

impl<'a, T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32> Iterator
    for NodeIterRef<'a, T, P, N, G>
{
    type Item = &'a UnrolledCell<T, P, N, G>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(_self) = &self.cur {
            let ret_val = self.cur;
            self.cur = _self.next.cell();
            ret_val
        } else {
            None
        }
    }
}

pub struct DrainingConsumingWrapper<
    T: Clone + 'static,
    P: PointerFamily,
    const N: u32,
    const G: u32,
>(DrainingConsumingIter<T, P, N, G>);

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> Iterator
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

pub struct ConsumingWrapper<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32>(
    ConsumingIter<T, P, N, G>,
);

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> Iterator
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

// struct OwnedNodeIterator<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32> {
//     list: UnrolledCell<T, P, N, G>,
// }

// impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> Iterator
//     for OwnedNodeIterator<T, P, N, G>
// {
//     type Item = T;

//     fn next(&mut self) -> Option<Self::Item> {
//         let value = self.list.car().cloned();
//         if self.list.index > 0 {
//             self.list.index -= 1;
//             value
//         } else {
//             None
//         }
//     }
// }

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> IntoIterator
    for UnrolledList<T, P, N, G>
{
    type Item = T;
    type IntoIter = ConsumingWrapper<T, P, N, G>;

    fn into_iter(self) -> Self::IntoIter {
        ConsumingWrapper(self.into_node_iter().flat_map(move |mut x| {
            // TODO: When doing this, we end up with a really funny
            // world where we have to _reallocate_ the whole vector. That
            // is not what we want. What we _actually_ want is to either
            //
            // 1. Take the target cell and elements
            // 2. Or - just clone each underlying element otherwise.
            //
            // Otherwise, we end up with a worst of both worlds, since
            // we need to reallocate the whole vec _just_ to get owned
            // references.
            /*
            let cell = P::make_mut(&mut x.0);

            match P::get_mut(&mut cell.elements) {
                Some(vec) => {
                    let elements = core::mem::take(vec);
                    MaybeCloned::Owned(elements.into_iter().take(x.index()).rev())
                }
                None => MaybeCloned::Cloned(OwnedNodeIterator {
                    list: cell.to_owned(),
                }), // None => cell.elements.iter().take(x.index()).rev(),
            }
            */

            let cell = P::make_mut(&mut x.0);

            let v = core::mem::take(cell.buffer_mut());

            let unique = v.into_unique();

            unique.into_iter().take(x.index()).rev()

            // cell.elements.into_unique()

            // MaybeCloned::Owned(v.into_unique().into_iter().cloned().take(x.index()).rev())

            // todo!()

            // let vec = P::make_mut(&mut cell.elements);

            // let elements = core::mem::take(vec);
            // elements.into_iter().take(x.index()).rev()
        }))
    }
}

// TODO have this also expose TryFold
pub struct IterWrapper<'a, T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32>(
    RefIter<'a, T, P, N, G>,
);

impl<'a, T: Clone, P: PointerFamily, const N: u32, const G: u32> Iterator
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

impl<'a, T: Clone, P: PointerFamily, const N: u32, const G: u32> IntoIterator
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

struct ExponentialChunks<I, const N: u32, const G: u32>
where
    I: Iterator,
{
    iter: I,
    size: usize,
    length: usize,
    running_sum: usize,
}

impl<I, const N: u32, const G: u32> ExponentialChunks<I, N, G>
where
    I: Iterator,
{
    fn new(iter: I, length: usize, mut size: usize) -> Self {
        let mut running_sum = size;

        while running_sum < length {
            size *= G as usize;
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

impl<I, const N: u32, const G: u32> Iterator for ExponentialChunks<I, N, G>
where
    I: Iterator,
{
    type Item = (usize, Vector<I::Item>);

    fn next(&mut self) -> Option<Self::Item> {
        let chunk_size = if self.length > self.running_sum {
            self.length - self.running_sum
        } else {
            self.size
        };

        let iter = self.iter.by_ref().take(chunk_size);

        let mut chunk = Vector::with_capacity(iter.size_hint().1.unwrap_or(0));

        for item in iter {
            chunk.push(item);
        }

        if chunk.is_empty() {
            return None;
        }

        let result = chunk;
        let size = self.size;

        self.size /= G as usize;
        self.length -= result.len();

        Some((size, result))
    }
}

fn from_vec<T: Clone, P: PointerFamily, const N: u32, const G: u32>(
    vec: Vec<T>,
) -> UnrolledList<T, P, N, G> {
    let length = vec.len();

    let mut pairs: SmallVec<[UnrolledList<_, _, N, G>; 16]> =
        ExponentialChunks::<_, N, G>::new(vec.into_iter(), length, N as usize)
            .map(|(size, x)| {
                let mut elements = x;
                elements.reverse();

                UnrolledList(P::new(UnrolledCell::new_chunk(
                    elements.len() as u32,
                    size as u32,
                    elements.into_shared_atomic(),
                    None,
                )))
            })
            .collect();

    let mut rev_iter = (0..pairs.len()).rev();
    rev_iter.next();

    for i in rev_iter {
        let prev = pairs.pop().unwrap();

        if let Some(UnrolledList(cell)) = pairs.get_mut(i) {
            P::get_mut::<UnrolledCell<T, P, N, G>>(cell)
                .expect("Only one owner allowed in construction")
                .next
                .set(Some(prev));
        } else {
            unreachable!()
        }
    }

    pairs.pop().unwrap_or_else(UnrolledList::new)
}

// and we'll implement FromIterator
// TODO specialize this for the into version?
impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> FromIterator<T>
    for UnrolledList<T, P, N, G>
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let reversed: Vec<_> = iter.into_iter().collect();
        from_vec(reversed)
    }
}

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> FromIterator<UnrolledList<T, P, N, G>>
    for UnrolledList<T, P, N, G>
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
                if cell.elements_len() + prev.0.elements_len() <= prev.0.size() as usize {
                    let left_inner = P::make_mut(cell);
                    let right_inner = P::make_mut(&mut prev.0);

                    // let left_vector = P::make_mut(&mut left_inner.elements);
                    // let right_vector = P::make_mut(&mut right_inner.elements);

                    // Drop the useless elements
                    if left_inner.index() < left_inner.elements_len() {
                        // left_vector.truncate(left_inner.index);

                        left_inner.elements_truncate(left_inner.index());
                    }

                    // TODO
                    if right_inner.index() < right_inner.elements_len() {
                        // right_vector.truncate(right_inner.index);

                        right_inner.elements_truncate(right_inner.index());
                    }

                    // Perform the actual move of the values
                    // right_vector.append(left_vector);

                    let mut taken = core::mem::take(left_inner.buffer_mut());
                    right_inner.buffer_mut().append(&mut taken);

                    // Swap the locations now after we've done the update
                    core::mem::swap(left_inner.buffer_mut(), right_inner.buffer_mut());

                    // Update this node to use correct capacity
                    let (left_size, right_size) = (left_inner.size(), right_inner.size());
                    left_inner.set_size(right_size);
                    right_inner.set_size(left_size);

                    // Adjust the indices accordingly
                    left_inner.set_index(left_inner.elements_len());
                    right_inner.set_index(0);

                    // Update this node to now point to the right nodes tail
                    core::mem::swap(&mut left_inner.next, &mut right_inner.next);
                } else {
                    P::make_mut(cell).next.set(Some(prev));
                }
            } else {
                unreachable!()
            }
        }

        nodes.pop().unwrap_or_else(Self::new)
    }
}

impl<'a, T: 'a + Clone, P: 'a + PointerFamily, const N: u32, const G: u32>
    FromIterator<&'a UnrolledList<T, P, N, G>> for UnrolledList<T, P, N, G>
{
    fn from_iter<I: IntoIterator<Item = &'a UnrolledList<T, P, N, G>>>(iter: I) -> Self {
        iter.into_iter().cloned().collect()
    }
}

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> From<Vec<T>>
    for UnrolledList<T, P, N, G>
{
    fn from(vec: Vec<T>) -> Self {
        from_vec(vec)
    }
}

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> From<&[T]>
    for UnrolledList<T, P, N, G>
{
    fn from(vec: &[T]) -> Self {
        from_vec(vec.to_vec())
    }
}

fn truncate<T: Clone>(value: &mut AtomicSharedVector<T>, index: usize) {
    for _ in 0..value.len() - index {
        value.pop();
    }
}

fn split_off<T: Clone>(value: &mut AtomicSharedVector<T>, index: usize) -> AtomicSharedVector<T> {
    let capacity = value.len() - index;
    let mut new = Vector::with_capacity(index);
    for i in index..value.len() {
        let value = value.get_mut(i).unwrap();
        new.push(unsafe { core::ptr::read(value) })
    }
    for _ in 0..capacity {
        core::mem::forget(value.pop());
    }

    new.into_shared_atomic()
}

#[cfg(test)]
mod tests {

    use crate::shared::RcPointer;

    type RcList<T> = UnrolledList<T, RcPointer, 256, 1>;

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

    type RcList<T> = UnrolledList<T, RcPointer, 256, 1>;
    type SmallRcList<T> = UnrolledList<T, RcPointer, 4, 2>;

    #[test]
    fn check_size() {
        println!(
            "{}",
            core::mem::size_of::<UnrolledCell<usize, RcPointer, 256, 256>>()
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
            println!("{:?}", node.elements_len());
        }

        // Should have 4 nodes at this point
        assert_eq!(left.node_iter().count(), 4);

        // 300 should be at 300
        assert_eq!(*left.get(300).unwrap(), 300);
        left.assert_list_invariants();
    }

    #[test]
    fn empty_node_appending_coalescing_works() {
        // 0/4: []
        let left: SmallRcList<i32> = SmallRcList::new();

        // 4/4: [2, 3, 4, 5]
        // 2/8: [0, 1]
        let right: SmallRcList<_> = (0..6).collect::<SmallRcList<_>>().reverse();

        assert_eq!(right.node_iter().count(), 2);

        // 6/8: [0, 1, 2, 3, 4, 5]
        let left = left.append(right);

        assert_eq!(left.node_iter().count(), 1);
        assert!(!left.is_empty());
        assert_eq!(*left.get(5).unwrap(), 0);
        left.assert_invariants();
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
    type RcList<T> = UnrolledList<T, RcPointer, 256, 1>;

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

    // Dropping cells one at a time with `cdr_mut` should release the buffers
    #[test]
    fn cdr_mut_releases_chunk_buffers() {
        type Tracked = alloc::rc::Rc<()>;
        let tracked = alloc::rc::Rc::new(());
        let mut list: UnrolledList<Tracked, RcPointer, 4, 2> =
            (0..64).map(|_| tracked.clone()).collect();
        assert_eq!(alloc::rc::Rc::strong_count(&tracked), 65);

        while list.cdr_mut().is_some() {}
        drop(list);

        assert_eq!(alloc::rc::Rc::strong_count(&tracked), 1);
    }

    #[test]
    fn dropping_a_whole_list_releases_chunk_buffers() {
        type Tracked = alloc::rc::Rc<()>;
        let tracked = alloc::rc::Rc::new(());
        let list: UnrolledList<Tracked, RcPointer, 4, 2> =
            (0..64).map(|_| tracked.clone()).collect();
        drop(list);
        assert_eq!(alloc::rc::Rc::strong_count(&tracked), 1);
    }
}

#[cfg(test)]
#[cfg(target_pointer_width = "64")]
mod split_layout {
    use super::*;

    #[derive(Clone)]
    #[repr(C)]
    struct V16(u64, u64);

    #[test]
    fn cell_is_two_arms_wide() {
        assert_eq!(size_of::<ChunkHead<V16>>(), 16, "chunk arm");
        assert_eq!(size_of::<CellStorage<V16>>(), 16, "union of the arms");
        assert_eq!(
            size_of::<UnrolledCell<V16, crate::shared::ArcPointer, 1, 2>>(),
            24,
            "payload: head + link"
        );
        assert_eq!(size_of::<Link<V16, crate::shared::ArcPointer, 1, 2>>(), 8);
    }
}
