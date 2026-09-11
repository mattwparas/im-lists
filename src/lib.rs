#![doc = include_str!("../README.md")]

pub mod handler;
pub mod list;
pub mod shared;
mod shared_vector;
pub(crate) mod unrolled;

/// Construct a [`List`](crate::list::List) from a sequence of elements
#[macro_export]
macro_rules! list {
    () => { $crate::list::List::new() };

    ( $($x:expr),+ $(,)? ) => {{
        vec![$(
            $x,
        ) *].into_iter().collect::<$crate::list::List<_>>()
    }};

    ( $($x:expr ,)+ $(,)? ) => {{
        vec![$($x)*].into_iter().collect::<$crate::list::List<_>>()
    }};
}

/// Construct a [`VList`](crate::list::VList) from a sequence of elements
#[macro_export]
macro_rules! vlist {
    () => { $crate::list::VList::new() };

    ( $($x:expr),+ $(,)? ) => {{
        vec![$(
            $x,
        ) *].into_iter().collect::<$crate::list::VList<_>>()
    }};

    ( $($x:expr ,)+ $(,)? ) => {{
        vec![$($x)*].into_iter().collect::<$crate::list::VList<_>>()
    }};
}

/// Construct a [`SharedList`](crate::list::SharedList) from a sequence of elements
#[macro_export]
macro_rules! shared_list {
    () => { $crate::list::SharedList::new() };

    ( $($x:expr),+ $(,)? ) => {{
        vec![$(
            $x,
        ) *].into_iter().collect::<$crate::list::SharedList<_>>()
    }};

    ( $($x:expr ,)+ $(,)? ) => {{
        vec![$($x)*].into_iter().collect::<$crate::list::SharedList<_>>()
    }};
}

/// Construct a [`SharedList`](crate::list::SharedList) from a sequence of elements
#[macro_export]
macro_rules! shared_vlist {
    () => { $crate::list::SharedVList::new() };

    ( $($x:expr),+ $(,)? ) => {{
        vec![$(
            $x,
        ) *].into_iter().collect::<$crate::list::SharedVList<_>>()
    }};

    ( $($x:expr ,)+ $(,)? ) => {{
        vec![$($x)*].into_iter().collect::<$crate::list::SharedVList<_>>()
    }};
}

#[cfg(test)]
mod user_bit_contract {
    use crate::shared::{ArcPointer, PointerFamily, RcPointer};

    #[test]
    fn families_without_a_header_report_false() {
        let p = RcPointer::new(7u64);
        assert!(!RcPointer::user_bit(&p));
        assert!(!RcPointer::set_user_bit(&p, true), "no place to put it");
        assert!(!RcPointer::user_bit(&p), "still false");

        let a = ArcPointer::new(7u64);
        assert!(!ArcPointer::user_bit(&a));
        assert!(!ArcPointer::set_user_bit(&a, true));
        assert!(!ArcPointer::user_bit(&a));
    }
}

