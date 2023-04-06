#![doc = include_str!("../README.md")]

pub mod assoc;
pub mod list;
pub mod shared;
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
