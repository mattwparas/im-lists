// use std::{cmp::Ordering, hash::Hash, iter::FromIterator};

// use crate::{
//     list::GenericList,
//     shared::{ArcPointer, PointerFamily, RcPointer},
//     unrolled::{ConsumingWrapper, IterWrapper, UnrolledList},
// };

// pub struct GenericAssocList<
//     K: Clone + Hash + PartialEq,
//     V: Clone,
//     P: PointerFamily = RcPointer,
//     const N: usize = 256,
//     const G: usize = 1,
// >(GenericList<(K, V), P, N, G>);
