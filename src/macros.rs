// Code coverage doesn't pick up doc tests, duplicate these down here
// just to make sure no obvious regressions happen.
macro_rules! public_api_tests {
    ($mod_name:tt, $type:tt, $list_macro:tt) => {
        use crate::$list_macro;

        #[test]
        fn strong_count() {
            let list: $type<usize> = $type::new();
            assert_eq!(list.strong_count(), 1);
        }

        #[test]
        fn len() {
            let list = $list_macro![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
            assert_eq!(list.len(), 10);
        }

        #[test]
        fn reverse() {
            let list = $list_macro![1, 2, 3, 4, 5].reverse();
            assert_eq!(list, $list_macro![5, 4, 3, 2, 1]);
        }

        #[test]
        fn last() {
            let list = $list_macro![1, 2, 3, 4, 5];
            assert_eq!(list.last(), Some(5));
        }

        #[test]
        fn car() {
            let list = $list_macro![1, 2, 3, 4, 5];
            let car = list.car();
            assert_eq!(car, Some(1));

            let list: $type<usize> = $list_macro![];
            let car = list.car();
            assert!(car.is_none());
        }

        #[test]
        fn cdr() {
            let mut list = $list_macro![1, 2, 3, 4, 5];
            list.cdr_mut().expect("This list has a tail");
            assert_eq!(list, $list_macro![2, 3, 4, 5]);

            let mut list = $list_macro![1, 2, 3];
            assert!(list.cdr_mut().is_some());
            assert_eq!(list, $list_macro![2, 3]);
            assert!(list.cdr_mut().is_some());
            assert_eq!(list, $list_macro![3]);
            assert!(list.cdr_mut().is_none());
            assert_eq!(list, $list_macro![]);
        }

        #[test]
        fn cons() {
            let list = $type::cons(
                1,
                $type::cons(2, $type::cons(3, $type::cons(4, $type::new()))),
            );
            assert_eq!(list, $list_macro![1, 2, 3, 4]);
        }

        #[test]
        fn cons_mut() {
            let mut list = $list_macro![];
            list.cons_mut(3);
            list.cons_mut(2);
            list.cons_mut(1);
            list.cons_mut(0);
            assert_eq!(list, $list_macro![0, 1, 2, 3])
        }

        #[test]
        fn push_front() {
            let mut list = $list_macro![];
            list.push_front(3);
            list.push_front(2);
            list.push_front(1);
            list.push_front(0);
            assert_eq!(list, $list_macro![0, 1, 2, 3])
        }

        #[test]
        fn iter() {
            assert_eq!($list_macro![1usize, 1, 1, 1, 1].iter().sum::<usize>(), 5);
        }

        #[test]
        fn get() {
            let list = $list_macro![1, 2, 3, 4, 5];
            assert_eq!(list.get(3), Some(4));
            assert!(list.get(1000).is_none());
        }

        #[test]
        fn append() {
            let left = $list_macro![1usize, 2, 3];
            let right = $list_macro![4usize, 5, 6];
            assert_eq!(left.append(right), $list_macro![1, 2, 3, 4, 5, 6])
        }

        #[test]
        fn is_empty() {
            let mut list = $type::new();
            assert!(list.is_empty());
            list.cons_mut("applesauce");
            assert!(!list.is_empty());
        }

        #[test]
        fn extend() {
            let list = $list_macro![1usize, 2, 3];
            let vec = vec![4, 5, 6];
            assert_eq!(list.extend(vec), $list_macro![1, 2, 3, 4, 5, 6])
        }
    };
}
