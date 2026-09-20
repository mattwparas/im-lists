#[cfg(test)]
mod proptests;

use crate::shared_vector::{AtomicSharedVector, Vector};
use smallvec::SmallVec;

use crate::shared::PointerFamily;

use std::cmp::Ordering;
use std::iter::{FlatMap, FromIterator, Rev};
use std::marker::PhantomData;

type ConsumingIter<T, P, const N: u32, const G: u32> = FlatMap<
    NodeIter<T, P, N, G>,
    // Rev<std::iter::Take<std::vec::IntoIter<T>>>,
    MaybeCloned<T, N, G>,
    fn(UnrolledList<T, P, N, G>) -> MaybeCloned<T, N, G>, // Rev<std::iter::Take<std::vec::IntoIter<T>>>,
>;

type MaybeCloned<T, const N: u32, const G: u32> =
    Rev<std::iter::Take<crate::shared_vector::IntoIter<T>>>;

type RefIter<'a, T, P, const N: u32, const G: u32> = FlatMap<
    NodeIterRef<'a, T, P, N, G>,
    Rev<std::slice::Iter<'a, T>>,
    fn(&'a UnrolledCell<T, P, N, G>) -> Rev<std::slice::Iter<'a, T>>,
>;

type DrainingConsumingIter<T, P, const N: u32, const G: u32> = FlatMap<
    DrainingNodeIter<T, P, N, G>,
    CellDrain<T>,
    fn(UnrolledList<T, P, N, G>) -> CellDrain<T>,
>;

/// One cell's worth of drained values. A cell holding its element inline hands
/// it straight back - going out to a buffer just to iterate one value was
/// allocating per cons on the consuming-iteration path.
pub enum CellDrain<T> {
    One(Option<T>),
    Chunk(Rev<std::iter::Take<crate::shared_vector::IntoIter<T>>>),
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
        // P::ptr_eq(&self.0.elements, &other.0.elements) && self.0.index == other.0.index()
        self.0.slot_ptr_eq(&other.0) && self.0.index() == other.0.index()
    }

    pub fn as_ptr_usize(&self) -> usize {
        P::as_ptr(&self.0) as usize
    }

    pub fn elements_as_ptr_usize(&self) -> usize {
        // P::as_ptr(&self.0.elements) as usize
        self.0.slot_as_ptr() as usize
    }

    pub fn next_ptr_as_usize(&self) -> Option<usize> {
        self.0.next.addr()
    }

    pub fn current_node_iter(&self) -> impl Iterator<Item = &T> {
        self.0.slot_iter().take(self.index() as _).rev()
    }

    pub fn draining_iterator(self) -> Option<DrainingConsumingWrapper<T, P, N, G>> {
        Some(DrainingConsumingWrapper(
            self.into_draining_node_iter().flat_map(|x| {
                let index = x.index();

                P::try_unwrap(x.0)
                    .map(|mut cell| {
                        if let Some(value) = cell.take_inline() {
                            CellDrain::One((index >= 1).then_some(value))
                        } else if cell.slot_is_unique() {
                            CellDrain::Chunk(
                                std::mem::take(cell.storage_mut())
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
            }),
        ))
    }

    // #[cfg(test)]
    pub fn cell_count(&self) -> usize {
        self.node_iter().count()
    }

    // This is actually like O(n / 64) which is actually quite nice
    // Saves us some time
    pub fn len(&self) -> usize {
        self.node_iter().map(|node| node.index() as usize).sum()
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

            if inner.index() < inner.slot_len() {
                // elements_mut.truncate(inner.index());
                let idx = inner.index() as usize;
                inner.slot_truncate(idx);
            }

            // elements_mut.reverse();
            inner.slot_reverse();
            inner.next.set(None);
        }

        for mut right in node_iter {
            let cell = P::make_mut(&mut right.0);
            // let elements_mut = P::make_mut(&mut cell.elements);

            if cell.index() < cell.slot_len() {
                // elements_mut.truncate(cell.index());

                {
                    let idx = cell.index() as usize;
                    cell.slot_truncate(idx)
                }
            }

            // elements_mut.reverse();
            cell.slot_reverse();
            cell.next.set(Some(left));
            left = right;
        }

        left
    }

    pub fn last(&self) -> Option<&T> {
        self.node_iter().last().and_then(|x| x.slot_as_slice().first())
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

                inner.slot_ensure_unique();

                // Grab the end of the vector, this will be the new backing
                // let remaining = elements_mut.split_off(inner.index() - count);

                let at = inner.index() as usize - count;
                let remaining = split_off(inner.storage_mut(), at);

                inner.set_index(count as _);

                // *elements_mut = remaining;

                *inner.storage_mut() = remaining;

                nodes.push(node);
                break;
            } else {
                // Note: We might want to truncate the remaining
                // elements of the vector.
                count -= node.0.index();
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
        let index = self.0.index() as usize;
        let mut elements_len = self.elements().len();
        let size = self.size();

        // This is saying: If we are pointing to a cell where the offset
        // has been moved to the right but the underlying data has not
        // yet been truncated, we should attempt to eagerly do so, otherwise
        // we should fall back to the existing implementation.
        let already_unique: Option<&mut UnrolledCell<T, P, N, G>> = if index < elements_len {
            // The cursor is behind LastUsed, so per the paper this is a cons
            // onto the tail of a longer list and belongs on the new-block path.
            // The one exception is a suffix nobody else can observe: over a
            // uniquely owned buffer, truncating is pure reclamation - no copy,
            // and no tail whose identity anyone could ask about.
            if frontier_cons_enabled() && !self.0.slot_is_unique() {
                self.slow_path_new_node(value);
                return;
            }
            // Only when we actually own the cell. `make_mut` on a shared one
            // copies the whole chunk just to truncate a copy nobody else can
            // see, which is how a cons onto shared structure ends up O(chunk).
            // Shared falls through to the paths below, which already answer it
            // by allocating a new node.
            match P::get_mut(&mut self.0) {
                Some(cell) => {
                    cell.slot_truncate(index);
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
            // Always initialize a vec with half the capacity of the previous one.
            // Never zero: an empty buffer reallocates on the first push, and
            // `grow_amortized` has a floor of 8, so a one-element cell would
            // otherwise carry a buffer sized for eight.
            let cap = (self.size() as usize / 4).max(1);
            let size = self.size();
            let grown = size * cons_growth(UnrolledCell::<T, P, N, G>::GROWTH_RATE);

            let old_self = std::mem::replace(self, UnrolledList::new());

            let cell = if inline_cells_enabled() {
                UnrolledCell::new_inline(value, Some(old_self))
            } else {
                occ_note_chunk(cap);
                let mut vec = Vector::with_capacity(cap);
                vec.push(value);
                occ_note_element();
                UnrolledCell::new_chunk(1, grown, vec.into_shared_atomic(), Some(old_self))
            };

            *self = UnrolledList(P::new(cell));
        } else {
            // If we can, save the value. We can shrink this down a bit
            if let Some(inner) = already_unique {
                match inner.slot_try_push(value) {
                    Ok(_) => {
                        inner.set_index(inner.index() + 1);
                    }
                    Err(v) => {
                        self.slow_path_new_node(v);
                    }
                }
            } else {
                match P::get_mut(&mut self.0) {
                    Some(inner) => match inner.slot_try_push(value) {
                        Ok(_) => {
                            inner.set_index(inner.index() + 1);
                        }
                        Err(v) => {
                            self.slow_path_new_node(v);
                        }
                    },
                    None => {
                        self.slow_path_new_node(value);
                    }
                }
            }
        }
    }

    fn slow_path_new_node(&mut self, value: T) {
        // Only grow if its actually larger enough
        let filled = self.elements().len() > self.size() as usize / 4;

        let size = self.size();

        let size = if filled {
            size.saturating_mul(cons_growth(UnrolledCell::<T, P, N, G>::GROWTH_RATE))
        } else {
            size
        };

        let old_self = std::mem::replace(self, UnrolledList::new());

        let next = if old_self.is_empty() {
            None
        } else {
            Some(old_self)
        };

        let cell = if inline_cells_enabled() {
            UnrolledCell::new_inline(value, next)
        } else {
            occ_note_chunk(N as usize);
            let mut vec = Vector::with_capacity(N as _);
            vec.push(value);
            occ_note_element();
            UnrolledCell::new_chunk(1, size, vec.into_shared_atomic(), next)
        };

        *self = UnrolledList(P::new(cell));
    }

    // Should be O(1) always
    // Should also not have to clone
    pub fn cdr(&self) -> Option<UnrolledList<T, P, N, G>> {
        self.0.cdr()
    }

    // Just pop off the internal value and move the index up
    pub fn pop_front(&mut self) -> Option<T> {
        let cell = P::make_mut(&mut self.0);
        // let elements = P::make_mut(&mut cell.elements);
        // let ret = elements.pop();
        let ret = cell.slot_pop();

        if ret.is_some() {
            cell.set_index(cell.index() - 1);
        }

        // If after we've popped, its empty, move the pointer to the
        // next one (if there is one)
        if cell.index() == 0 {
            if let Some(mut next) = cell.next.take() {
                std::mem::swap(self, &mut next);
            }
        }

        ret
    }

    /// Raw pointer to the next cell, or `None` at the end of the chain. The
    /// link's tag bits are masked off.
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

    /// Position this list's cursor at `index` within its current cell.
    ///
    /// Generated code walks a list as a `(cell, index)` pair held in registers -
    /// stepping is then arithmetic rather than a new cell - and calls this to
    /// turn that pair back into a real list when the value escapes. Copying the
    /// cell here is the cost the register walk was avoiding, so it is paid once,
    /// on escape, rather than once per step.
    pub(crate) fn set_cursor(&mut self, index: usize) {
        if self.0.index() == index {
            return;
        }

        // An inline cell holds exactly one element and its cursor is implicitly
        // 1, so there is no other position to move to.
        debug_assert!(!self.0.next.is_inline(), "an inline cell has only one cursor position");

        let cell = P::make_mut(&mut self.0);
        cell.set_index(index);
    }

    /// Walk this list forward to the cell at `ptr` and set its cursor to
    /// `index`, the way repeated `cdr_mut` would: cells owned uniquely are
    /// stepped past without allocating, and a unique target cell is
    /// repositioned in place. `Err` hands the list back, partly walked, at
    /// the first shared cell - it still keeps the target reachable.
    pub(crate) fn reposition(mut self, ptr: *const UnrolledCell<T, P, N, G>, index: usize) -> Result<Self, Self> {
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
                // Ran off the end without meeting the cell: not a cell of
                // this list. Nothing left to hand back.
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
        self.0.slot_as_slice()
    }

    fn size(&self) -> u32 {
        self.0.cap()
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
            .flat_map(|x| x.slot_as_slice()[0..x.index()].iter().rev())
    }

    // Every node must have either CAPACITY elements, or be marked as full
    // Debateable whether I want them marked as full
    #[cfg(test)]
    pub fn assert_invariants(&self) {
        assert!(self
            .node_iter()
            .all(|node| node.slot_len() <= node.cap() as usize))
    }

    pub fn get(&self, mut index: usize) -> Option<&T> {
        if index < self.0.index() {
            self.0.slot_get(self.0.index() - index - 1)
        } else {
            let mut cur = self.0.next.cell();
            index -= self.0.index();

            while let Some(node) = cur {
                if index < node.index() {
                    let node_cap = node.index();
                    return node.slot_get(node_cap as usize - index - 1);
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
        self.0.slot_is_empty() || self.0.index() == 0
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
        // The union does not drop its own field, so pick the live one first.
        unsafe {
            if !self.next.is_inline() {
                // The buffer is what owns the allocation, and it is itself
                // `ManuallyDrop` inside `ChunkHead`, which has no `Drop` - so
                // dropping the head alone freed nothing and leaked every chunk
                // buffer dropped this way (a `cdr` walk over a fresh list leaked
                // the whole list). The draining paths `mem::take` the buffer
                // and leave an empty one behind, so it is always valid here.
                core::mem::ManuallyDrop::drop(&mut (*self.storage.chunk).buffer);
            } else if !self.next.is_moved() {
                core::mem::ManuallyDrop::drop(&mut self.storage.inline);
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

// TEMPORARY: chunk occupancy instrumentation.
pub static OCC_CHUNKS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
pub static OCC_CAPACITY: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
pub static OCC_ELEMENTS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

#[inline]
fn occ_note_chunk(cap: usize) {
    use core::sync::atomic::Ordering::Relaxed;
    if !occ_enabled() {
        return;
    }
    OCC_CAPACITY.fetch_add(cap as u64, Relaxed);
    let n = OCC_CHUNKS.fetch_add(1, Relaxed) + 1;
    if n % 20_000_000 == 0 {
        let cap = OCC_CAPACITY.load(Relaxed);
        let el = OCC_ELEMENTS.load(Relaxed);
        eprintln!(
            "occupancy: chunks={n} capacity_slots={cap} elements={el} fill={:.1}% avg_cap={:.1}",
            100.0 * el as f64 / cap.max(1) as f64,
            cap as f64 / n as f64
        );
    }
}

#[inline]
fn occ_note_element() {
    if occ_enabled() {
        OCC_ELEMENTS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    }
}



fn occ_enabled() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("IM_LISTS_OCCUPANCY").is_some())
}


/// The link to the next cell, carrying the storage discriminant in its low bit.
///
/// The flag has to live inside the cell: `P::try_unwrap` hands out owned cells
/// that have left their allocation, and their `Drop` still has to decide which
/// arm to drop. `next` is the only field with room for it, and it sits at the
/// same offset in both arms, so reading the flag never depends on knowing which
/// arm you are looking at.
#[repr(transparent)]
pub(crate) struct Link<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32> {
    raw: *const UnrolledCell<T, P, N, G>,
    // `raw` reaches `P` only through the cell, which contains a `Link` in turn,
    // so the parameter reads as purely recursive without this.
    _marker: core::marker::PhantomData<fn() -> P>,
}

/// Bit 0: this cell keeps its single element inline rather than in a chunk.
/// Bit 1: that element has already been moved out, so `Drop` leaves it alone.
/// Cells are at least word aligned, so both are ours.
/// Which arm of the union this cell is using, in the low bits of the link.
/// Cells are word aligned, so bits 0..3 are ours - exactly enough for three
/// variants plus the moved flag, with nothing spare.
pub(crate) const VARIANT_CHUNK: usize = 0;
pub(crate) const VARIANT_INLINE: usize = 1;
pub(crate) const VARIANT_MASK: usize = 3;

/// Set once an inline element has been moved out, so `Drop` leaves it alone.
const MOVED_TAG: usize = 4;
const TAG_MASK: usize = VARIANT_MASK | MOVED_TAG;

/// Kept for generated code, which only needs to tell inline from not.
pub(crate) const INLINE_TAG: usize = VARIANT_INLINE;

/// Kept for generated code, which masks these off `next` to get the pointer.
pub(crate) const LINK_TAG_MASK: usize = TAG_MASK;

impl<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32> Link<T, P, N, G> {
    #[inline(always)]
    fn pack(ptr: *const UnrolledCell<T, P, N, G>, variant: usize) -> Self {
        debug_assert!(
            ptr as usize & TAG_MASK == 0,
            "cell pointers are aligned, so the low bits are ours"
        );
        Self {
            raw: ((ptr as usize) | variant) as *const _,
            _marker: core::marker::PhantomData,
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
        self.raw as usize & VARIANT_MASK
    }

    #[inline(always)]
    pub(crate) fn is_inline(&self) -> bool {
        self.variant() == VARIANT_INLINE
    }

    #[inline(always)]
    #[inline(always)]
    pub(crate) fn set_variant(&mut self, variant: usize) {
        let bits = self.raw as usize;
        self.raw = ((bits & !VARIANT_MASK) | variant) as *const _;
    }

    #[inline(always)]
    fn ptr(&self) -> *const UnrolledCell<T, P, N, G> {
        ((self.raw as usize) & !TAG_MASK) as *const _
    }

    #[inline(always)]
    pub(crate) fn is_moved(&self) -> bool {
        self.raw as usize & MOVED_TAG != 0
    }

    #[inline(always)]
    pub(crate) fn set_moved(&mut self, on: bool) {
        let bits = self.raw as usize;
        self.raw = (if on { bits | MOVED_TAG } else { bits & !MOVED_TAG }) as *const _;
    }

    #[inline(always)]
    pub(crate) fn is_some(&self) -> bool {
        !self.ptr().is_null()
    }

    #[inline(always)]
    pub(crate) fn is_none(&self) -> bool {
        self.ptr().is_null()
    }

    /// The next cell, borrowed without touching any count.
    #[inline(always)]
    pub(crate) fn cell(&self) -> Option<&UnrolledCell<T, P, N, G>> {
        unsafe { self.ptr().as_ref() }
    }

    /// The link as a list, borrowed. For the few callers that want the pointer
    /// rather than the cell - a strong count, say.
    #[inline(always)]
    pub(crate) fn borrow(&self) -> Option<core::mem::ManuallyDrop<UnrolledList<T, P, N, G>>> {
        self.borrow_list()
    }

    /// The link as a list, for the few callers that need the pointer itself.
    /// Deliberately `ManuallyDrop` - this borrows, it does not take.
    #[inline(always)]
    fn borrow_list(&self) -> Option<core::mem::ManuallyDrop<UnrolledList<T, P, N, G>>> {
        let ptr = self.ptr();
        if ptr.is_null() {
            None
        } else {
            Some(core::mem::ManuallyDrop::new(UnrolledList(unsafe {
                P::from_raw(ptr)
            })))
        }
    }

    /// A second reference to the same next cell, flag included.
    #[inline(always)]
    pub(crate) fn clone_link(&self) -> Self {
        let mut cloned = Self::new(self.clone_list(), self.variant());
        cloned.set_moved(self.is_moved());
        cloned
    }

    #[inline(always)]
    pub(crate) fn clone_list(&self) -> Option<UnrolledList<T, P, N, G>> {
        self.borrow_list()
            .map(|list| UnrolledList(P::clone(&list.0)))
    }

    /// Take the link, leaving the flag where it is.
    #[inline(always)]
    pub(crate) fn take(&mut self) -> Option<UnrolledList<T, P, N, G>> {
        let ptr = self.ptr();
        self.raw = ((self.raw as usize) & TAG_MASK) as *const _;
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

    pub(crate) fn strong_count(&self) -> usize {
        self.borrow_list()
            .map(|list| P::strong_count(&list.0))
            .unwrap_or(0)
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

/// A counted handle to the cell, recovered from a bare reference.
///
/// Sound because every `UnrolledCell` this is called on lives at the start of a
/// `P` allocation - the node iterators only ever hand out cells reached through
/// a `Link`, which is exactly such a pointer.
#[inline(always)]
pub(crate) fn list_from_cell<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32>(
    cell: &UnrolledCell<T, P, N, G>,
) -> UnrolledList<T, P, N, G> {
    let borrowed = core::mem::ManuallyDrop::new(UnrolledList::<T, P, N, G>(unsafe {
        P::from_raw(cell as *const UnrolledCell<T, P, N, G>)
    }));
    UnrolledList(P::clone(&borrowed.0))
}

// A `Link` owns exactly one `P::Pointer`, so it is exactly as shareable as one.
// Spelled out because the raw pointer inside would otherwise make it neither.
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

/// A cell holding a single element stores it here rather than in a separately
/// allocated buffer: that buffer costs a 16 byte header plus the allocation
/// itself, and a one element chunk's `cdr` is just `next`, so nothing needs to
/// share it. Measured cheaper than boxing the element, which trades 8 bytes of
/// cell for a 16 byte allocation.
///
/// The discriminant is the high bit of `size`, which only ever holds small
/// capacities. `repr(C)` because generated code reads these cells by offset.
#[repr(C)]
pub(crate) union CellStorage<T: Clone + 'static> {
    pub(crate) inline: core::mem::ManuallyDrop<T>,
    pub(crate) chunk: core::mem::ManuallyDrop<ChunkHead<T>>,
}


/// Everything a chunked cell needs in the same 16 bytes a single inline element
/// occupies: its cursor, its capacity target, and the chunk itself.
#[repr(C)]
pub(crate) struct ChunkHead<T: Clone + 'static> {
    pub(crate) index: u32,
    pub(crate) size: u32,
    pub(crate) buffer: core::mem::ManuallyDrop<AtomicSharedVector<T>>,
}




/// Growth applied when a cons has to start a new chunk. `G` is Bagwell's
/// geometric schedule; 1 makes every chunk hold a single element, which is what
/// heavy structural sharing degenerates to anyway.
fn cons_growth(default: u32) -> u32 {
    static V: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        std::env::var("IM_LISTS_CONS_GROWTH")
            .ok()
            .and_then(|x| x.parse().ok())
            .filter(|x| *x >= 1)
    })
    .unwrap_or(default)
}

fn inline_cells_enabled() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("IM_LISTS_INLINE_CELL").as_deref() != Ok("false"))
}

// Bagwell, Fast Functional Lists section 2.1. `cons` compares the pointer
// offset with LastUsed: equal means this cell sits at the frontier of its block
// and the entry can be written in place; less than means "a cons is being
// applied to the tail of a longer list", where the paper allocates a new block
// whose link is that tail, so "the two lists now share a common tail, just as
// would have been the case with a Linked List implementation".
//
// Off restores the pre-existing behaviour (push into the chunk regardless of
// where the cursor sits), which copies the chunk and loses tail identity.
fn frontier_cons_enabled() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("IM_LISTS_FRONTIER_CONS").as_deref() != Ok("false"))
}

pub struct UnrolledCell<T: Clone + 'static, P: PointerFamily, const N: u32, const G: u32> {
    pub(crate) storage: CellStorage<T>,
    pub(crate) next: Link<T, P, N, G>,
}

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> Clone for UnrolledCell<T, P, N, G> {
    fn clone(&self) -> Self {
        let storage = unsafe {
            if self.next.is_inline() {
                CellStorage {
                    inline: core::mem::ManuallyDrop::new((*self.storage.inline).clone()),
                }
            } else {
                CellStorage {
                    chunk: core::mem::ManuallyDrop::new(ChunkHead {
                        index: self.storage.chunk.index,
                        size: self.storage.chunk.size,
                        buffer: core::mem::ManuallyDrop::new(
                            (*self.storage.chunk.buffer).clone(),
                        ),
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

impl<T: Clone + std::fmt::Debug, P: PointerFamily, const N: u32, const G: u32> std::fmt::Debug
    for UnrolledList<T, P, N, G>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

impl<T: Clone, P: PointerFamily, const N: u32, const G: u32> UnrolledCell<T, P, N, G> {
    const GROWTH_RATE: u32 = if G == 0 { 1 } else { G };

    /// The chunk's contents. Every read and every mutation goes through this
    /// surface, so a cell that stores its single element some other way only
    /// has to be taught about it here.
    #[inline(always)]
    pub(crate) fn cap(&self) -> u32 {
        if self.is_inline() {
            // Holds one and can take one more; the second element moves it out
            // to a chunk, which is where a real capacity target comes back.
            2
        } else {
            unsafe { self.storage.chunk.size }
        }
    }

    /// This cell's cursor. An inline cell holds exactly one element, so it is
    /// always at 1 - there is nowhere else for the cursor to be.
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
            // An inline cell's cursor is implicitly 1, so writing 1 back is a
            // no-op - `tail` does exactly that when it lands on a boundary.
            // Anything else means the caller thinks this cell is a chunk.
            debug_assert_eq!(index, 1, "an inline cell holds exactly one element");
            return;
        }
        unsafe { (*self.storage.chunk).index = index as u32 };
    }

    #[inline(always)]
    pub(crate) fn set_cap(&mut self, size: u32) {
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
    pub(crate) fn storage(&self) -> &AtomicSharedVector<T> {
        &self.chunk().buffer
    }

    /// Move an inline element back into a buffer. Every mutation goes through
    /// `storage_mut`, so inline cells only ever have to answer reads.
    #[cold]
    fn deinline(&mut self) {
        debug_assert!(self.next.is_inline() && !self.next.is_moved());
        let value = unsafe { core::mem::ManuallyDrop::take(&mut self.storage.inline) };
        // A cell deinlines because a second element is arriving, so it needs
        // room for exactly two. Going through `grow_amortized` here would take
        // the `grow_floor` of 4 and leave two dead slots on every such buffer -
        // which, at a hundred million of them, is gigabytes.
        let cap = 2;
        // The buffer only needs room for the two elements at hand, but the
        // cell's capacity *target* has to keep climbing - pinning it at 2 caps
        // every cell at two elements, which fragments a list into one small
        // buffer per pair and kills the geometric schedule outright. A new cell
        // is always the previous one grown, and the previous one is `next`.
        let target = self
            .next
            .cell()
            .map(|cell| cell.cap().saturating_mul(Self::GROWTH_RATE))
            .unwrap_or(N)
            .max(cap as u32);
        let mut buffer = AtomicSharedVector::with_capacity(cap);
        buffer.push(value);
        self.storage = CellStorage {
            chunk: core::mem::ManuallyDrop::new(ChunkHead {
                index: 1,
                size: target,
                buffer: core::mem::ManuallyDrop::new(buffer),
            }),
        };
        self.next.set_variant(VARIANT_CHUNK);
    }

    #[inline(always)]
    pub(crate) fn storage_mut(&mut self) -> &mut AtomicSharedVector<T> {
        if self.is_inline() {
            self.deinline();
        }
        unsafe { &mut (*self.storage.chunk).buffer }
    }

    /// Move the inline element out of a cell we own. Leaves the cell marked so
    /// `Drop` will not touch the moved value.
    fn take_inline(&mut self) -> Option<T> {
        if !self.next.is_inline() || self.next.is_moved() {
            return None;
        }
        self.next.set_moved(true);
        Some(unsafe { core::mem::ManuallyDrop::take(&mut self.storage.inline) })
    }

    #[inline(always)]
    fn inline_ref(&self) -> &T {
        debug_assert!(self.is_inline());
        unsafe { &self.storage.inline }
    }

    #[inline(always)]
    pub(crate) fn slot_len(&self) -> usize {
        if self.is_inline() {
            1
        } else {
            self.storage().len()
        }
    }

    #[inline(always)]
    pub(crate) fn slot_get(&self, i: usize) -> Option<&T> {
        if self.is_inline() {
            (i == 0).then(|| self.inline_ref())
        } else {
            self.storage().get(i)
        }
    }

    #[inline(always)]
    pub(crate) fn slot_is_empty(&self) -> bool {
        !self.is_inline() && self.storage().is_empty()
    }

    #[inline(always)]
    pub(crate) fn slot_iter(&self) -> core::slice::Iter<'_, T> {
        self.slot_as_slice().iter()
    }

    #[inline(always)]
    pub(crate) fn slot_as_slice(&self) -> &[T] {
        if self.is_inline() {
            core::slice::from_ref(self.inline_ref())
        } else {
            self.storage()
        }
    }

    #[inline(always)]
    pub(crate) fn slot_is_unique(&self) -> bool {
        // The element is owned outright; sharing of the cell is P's business.
        self.is_inline() || self.storage().is_unique()
    }

    #[inline(always)]
    pub(crate) fn slot_as_ptr(&self) -> *const T {
        if self.is_inline() {
            self.inline_ref() as *const T
        } else {
            self.storage().as_ptr()
        }
    }

    #[inline(always)]
    pub(crate) fn slot_try_push(&mut self, value: T) -> Result<(), T> {
        self.storage_mut().try_push(value)
    }

    #[inline(always)]
    pub(crate) fn slot_push(&mut self, value: T) {
        self.storage_mut().push(value)
    }


    #[inline(always)]
    pub(crate) fn slot_pop(&mut self) -> Option<T> {
        self.storage_mut().pop()
    }

    /// Drop everything past `len`. An inline cell holds a single element, so
    /// any `len >= 1` already satisfies this.
    pub(crate) fn slot_truncate(&mut self, len: usize) {
        if self.is_inline() && len >= 1 {
            return;
        }
        truncate(self.storage_mut(), len);
    }

    #[inline(always)]
    pub(crate) fn slot_reverse(&mut self) {
        if !self.is_inline() {
            self.storage_mut().reverse()
        }
    }

    #[inline(always)]
    pub(crate) fn slot_ensure_unique(&mut self) {
        if !self.is_inline() {
            self.storage_mut().ensure_unique()
        }
    }

    #[inline(always)]
    pub(crate) fn slot_ptr_eq(&self, other: &Self) -> bool {
        if self.is_inline() || other.is_inline() {
            return self.slot_as_ptr() == other.slot_as_ptr();
        }
        self.storage().ptr_eq(other.storage())
    }

    fn new() -> Self {
        Self::new_chunk(0, N, AtomicSharedVector::new(), None)
    }

    /// A chunked cell. Everything but the inline case goes through here, so the
    /// union is only ever built in one place.
    #[inline(always)]
    fn new_chunk(
        index: u32,
        size: u32,
        buffer: AtomicSharedVector<T>,
        next: Option<UnrolledList<T, P, N, G>>,
    ) -> Self {
        UnrolledCell {
            storage: CellStorage {
                chunk: core::mem::ManuallyDrop::new(ChunkHead {
                    index,
                    size,
                    buffer: core::mem::ManuallyDrop::new(buffer),
                }),
            },
            next: Link::new(next, VARIANT_CHUNK),
        }
    }

    /// A fresh one-element cell, holding its element inline. `size` stays the
    /// capacity this cell would have allocated, so the buffer is only deferred
    /// to the second element, not resized.
    #[inline(always)]
    fn new_inline(value: T, next: Option<UnrolledList<T, P, N, G>>) -> Self {
        UnrolledCell {
            storage: CellStorage {
                inline: core::mem::ManuallyDrop::new(value),
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
        self.slot_get(self.index() - 1)
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
        // index > 1, so this cell holds more than one element and is not inline.
        debug_assert!(!self.is_inline());
        let mut cell = Self::new_chunk(
            (self.index() - 1) as u32,
            self.cap(),
            self.storage().clone(),
            None,
        );
        cell.next = self.next.clone_link();
        cell
    }

    // TODO make this better
    fn cons_mut(&mut self, value: T) {
        // let reference = P::make_mut(&mut self.elements);
        self.slot_push(value);
        // reference.push(value);
        self.set_index(self.index() + 1);
    }

    // Spill over the values to a new node
    // otherwise, copy the node and spill over
    fn cons(value: T, mut cdr: UnrolledList<T, P, N, G>) -> UnrolledList<T, P, N, G> {
        let size = cdr.size();

        let shared = P::strong_count(&cdr.0) > 1;
        // Section 2.1 frontier test, as in `cons_mut`: a cursor behind LastUsed
        // over a buffer someone else still holds must link to the tail rather
        // than copy the chunk out from under it.
        let behind_frontier = frontier_cons_enabled()
            && (cdr.0.index() as usize) < cdr.elements().len()
            && !cdr.0.slot_is_unique();

        if shared || behind_frontier || cdr.elements().len() > size as usize - 1 {
            let grown = size * cons_growth(Self::GROWTH_RATE);
            // An empty cdr is not a link - hanging the empty cell off the chain
            // leaves a node with nothing in it, which `last` walks straight into.
            // `slow_path_new_node` has always guarded this; this branch did not,
            // and simply never saw an empty cdr until now.
            let next = if cdr.is_empty() { None } else { Some(cdr) };
            let cell = if inline_cells_enabled() {
                Self::new_inline(value, next)
            } else {
                UnrolledCell::new_chunk(1, grown, crate::arc_vector![value], next)
            };
            UnrolledList(P::new(cell))
        } else {
            let inner = P::make_mut(&mut cdr.0);
            // let elements = P::make_mut(&mut inner.elements);

            // Push first: on an inline cell that is what moves it out to a
            // chunk, and only a chunk has a cursor to bump.
            // Reclaim an owned suffix first, so the push lands at the cursor
            // rather than past it. (With the frontier test on, only a uniquely
            // owned buffer can reach here behind the frontier.)
            let idx = inner.index() as usize;
            if frontier_cons_enabled() && idx < inner.slot_as_slice().len() {
                inner.slot_truncate(idx);
            }
            inner.slot_push(value);
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

        if let Some(mut _self) = std::mem::take(&mut self.cur) {
            if let Some(next) = _self.0.next.borrow() {
                // If we can, drop these values!
                // if next.strong_count() == 1 && P::strong_count(&next.0.elements) == 1 {
                if P::strong_count(&next.0) == 1 && next.0.slot_is_unique() {
                    // self.cur = _self.0.next.clone_link();
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
        //         std::mem::swap(&mut self.cur, &mut value);
        //         return value;
        //     }

        //     // If we can, drop these values!
        //     if next.strong_count() == 1 && P::strong_count(&next.0.elements) == 1 {
        //         // self.cur = _self.0.next.clone_link();
        //         // self.cur = P::get_mut(&mut _self.0).and_then(|x| x.next.take());
        //         // todo!()

        //         let mut value = _self
        //             .as_mut()
        //             .and_then(|x| P::get_mut(&mut x.0).and_then(|x| x.next.take()));

        //         std::mem::swap(&mut self.cur, &mut value);

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
        if let Some(_self) = std::mem::take(&mut self.cur) {
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
//         if self.list.index() > 0 {
//             self.list.set_index(self.list.index() - 1);
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

            match P::get_mut(cell.storage_mut()) {
                Some(vec) => {
                    let elements = std::mem::take(vec);
                    MaybeCloned::Owned(elements.into_iter().take(x.index()).rev())
                }
                None => MaybeCloned::Cloned(OwnedNodeIterator {
                    list: cell.to_owned(),
                }), // None => cell.elements.iter().take(x.index()).rev(),
            }
            */

            let cell = P::make_mut(&mut x.0);

            let v = std::mem::take(cell.storage_mut());

            let unique = v.into_unique();

            unique.into_iter().take(x.index() as _).rev()

            // cell.elements.into_unique()

            // MaybeCloned::Owned(v.into_unique().into_iter().cloned().take(x.index()).rev())

            // todo!()

            // let vec = P::make_mut(&mut cell.elements);

            // let elements = std::mem::take(vec);
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
                .flat_map(|x| x.slot_as_slice()[0..x.index()].iter().rev()),
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
                if cell.slot_len() + prev.0.slot_len() <= prev.0.cap() as usize {
                    let left_inner = P::make_mut(cell);
                    let right_inner = P::make_mut(&mut prev.0);

                    // let left_vector = P::make_mut(&mut left_inner.elements);
                    // let right_vector = P::make_mut(&mut right_inner.elements);

                    // Drop the useless elements
                    if left_inner.index() < left_inner.slot_len() {
                        // left_vector.truncate(left_inner.index());

                        let li = left_inner.index() as usize;
                        left_inner.slot_truncate(li);
                    }

                    // TODO
                    if right_inner.index() < right_inner.slot_len() {
                        // right_vector.truncate(right_inner.index());

                        let ri = right_inner.index() as usize;
                        right_inner.slot_truncate(ri);
                    }

                    // Perform the actual move of the values
                    // right_vector.append(left_vector);

                    let taken = std::mem::take(left_inner.storage_mut());
                    right_inner.storage_mut().append(&mut { taken });

                    // Swap the locations now after we've done the update
                    std::mem::swap(left_inner.storage_mut(), right_inner.storage_mut());

                    // Update this node to use correct capacity
                    let (left_cap, right_cap) = (left_inner.cap(), right_inner.cap());
                    left_inner.set_cap(right_cap);
                    right_inner.set_cap(left_cap);

                    // Adjust the indices accordingly
                    left_inner.set_index(left_inner.slot_len());
                    right_inner.set_index(0);

                    // Update this node to now point to the right nodes tail
                    std::mem::swap(&mut left_inner.next, &mut right_inner.next);
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
        new.push(unsafe { std::ptr::read(value) })
    }
    for _ in 0..capacity {
        std::mem::forget(value.pop());
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
            println!("{:?}", node.slot_len());
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

    // A chunked cell dropped on its own - as `cdr_mut` does when it steps past
    // one - has to release its buffer and the elements in it. It used to drop
    // only the `ChunkHead`, whose buffer is `ManuallyDrop`, so every element
    // stayed alive and the buffer leaked: a Steel loop walking a fresh list
    // with `cdr` leaked the whole list each time.
    #[test]
    fn cdr_mut_releases_chunk_buffers() {
        let tracked = std::rc::Rc::new(());
        type Tracked = std::rc::Rc<()>;
        let mut list: UnrolledList<Tracked, RcPointer, 4, 2> =
            (0..64).map(|_| tracked.clone()).collect();
        assert_eq!(std::rc::Rc::strong_count(&tracked), 65);

        while list.cdr_mut().is_some() {}
        drop(list);

        assert_eq!(std::rc::Rc::strong_count(&tracked), 1);
    }

    #[test]
    fn dropping_a_whole_list_releases_chunk_buffers() {
        let tracked = std::rc::Rc::new(());
        type Tracked = std::rc::Rc<()>;
        let list: UnrolledList<Tracked, RcPointer, 4, 2> =
            (0..64).map(|_| tracked.clone()).collect();
        drop(list);
        assert_eq!(std::rc::Rc::strong_count(&tracked), 1);
    }
}



#[cfg(test)]
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


