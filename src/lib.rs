#![doc = include_str!("../README.md")]

#[macro_use]
pub(crate) mod macros;
pub mod list;
pub(crate) mod shared;
pub mod shared_list;
pub(crate) mod unrolled;

#[macro_export]
macro_rules! list {
    () => { $crate::list::List::new() };

    ( $($x:expr),* ) => {{
        vec![$(
            $x,
        ) *].into_iter().collect::<$crate::list::List<_>>()
    }};

    ( $($x:expr ,)* ) => {{
        vec![$($x)*].into_iter().collect::<$crate::list::List<_>>()
    }};
}

#[macro_export]
macro_rules! shared_list {
    () => { $crate::shared_list::SharedList::new() };

    ( $($x:expr),* ) => {{
        vec![$(
            $x,
        ) *].into_iter().collect::<$crate::shared_list::SharedList<_>>()
    }};

    ( $($x:expr ,)* ) => {{
        vec![$($x)*].into_iter().collect::<$crate::shared_list::SharedList<_>>()
    }};
}
