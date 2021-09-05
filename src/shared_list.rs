use crate::{shared::ArcConstructor, unrolled::UnrolledList};

pub struct SharedList<T: Clone>(UnrolledList<T, ArcConstructor, ArcConstructor>);
