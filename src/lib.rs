#![doc = include_str!("../README.md")]
#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;

#[doc(hidden)]
pub mod __private {
    pub use alloc::vec;
}

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
        $crate::__private::vec![$(
            $x,
        ) *].into_iter().collect::<$crate::list::List<_>>()
    }};

    ( $($x:expr ,)+ $(,)? ) => {{
        $crate::__private::vec![$($x)*].into_iter().collect::<$crate::list::List<_>>()
    }};
}

/// Construct a [`VList`](crate::list::VList) from a sequence of elements
#[macro_export]
macro_rules! vlist {
    () => { $crate::list::VList::new() };

    ( $($x:expr),+ $(,)? ) => {{
        $crate::__private::vec![$(
            $x,
        ) *].into_iter().collect::<$crate::list::VList<_>>()
    }};

    ( $($x:expr ,)+ $(,)? ) => {{
        $crate::__private::vec![$($x)*].into_iter().collect::<$crate::list::VList<_>>()
    }};
}

/// Construct a [`SharedList`](crate::list::SharedList) from a sequence of elements
#[cfg(target_has_atomic = "ptr")]
#[macro_export]
macro_rules! shared_list {
    () => { $crate::list::SharedList::new() };

    ( $($x:expr),+ $(,)? ) => {{
        $crate::__private::vec![$(
            $x,
        ) *].into_iter().collect::<$crate::list::SharedList<_>>()
    }};

    ( $($x:expr ,)+ $(,)? ) => {{
        $crate::__private::vec![$($x)*].into_iter().collect::<$crate::list::SharedList<_>>()
    }};
}

/// Construct a [`SharedList`](crate::list::SharedList) from a sequence of elements
#[cfg(target_has_atomic = "ptr")]
#[macro_export]
macro_rules! shared_vlist {
    () => { $crate::list::SharedVList::new() };

    ( $($x:expr),+ $(,)? ) => {{
        $crate::__private::vec![$(
            $x,
        ) *].into_iter().collect::<$crate::list::SharedVList<_>>()
    }};

    ( $($x:expr ,)+ $(,)? ) => {{
        $crate::__private::vec![$($x)*].into_iter().collect::<$crate::list::SharedVList<_>>()
    }};
}
