#![doc = include_str!("../README.md")]
pub mod list;
pub mod shared;
pub mod shared_list;
pub mod unrolled;

#[macro_export]
macro_rules! list {
    () => { $crate::list::List::new() };

    ( $($x:expr),* ) => {{
        vec![$(
            $x,
        ) *].into_iter().into_iter().collect::<$crate::list::List<_>>()
    }};

    ( $($x:expr ,)* ) => {{
        vec![$($x)*].into_iter().collect::<$crate::list::List<_>>()
    }};
}
