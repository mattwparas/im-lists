// Code coverage doesn't pick up doc tests, duplicate these down here
// just to make sure no obvious regressions happen.
#[cfg(test)]
macro_rules! public_api_tests {
    ($mod_name:tt, $type:tt, $list_macro:tt) => {
        use crate::$list_macro;

        #[test]
        fn strong_count() {
            let list: $type<usize> = $type::new();
            assert_eq!(list.strong_count(), 1);
        }

        #[test]
        fn cell_count() {
            let mut list: $type<usize> = (0..256).into_iter().collect();
            assert_eq!(list.cell_count(), 1);

            list.push_front(100);
            list.push_front(200);

            assert_eq!(list.cell_count(), 2);
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
            let list = $list_macro![1, 2, 3, 4, 5];
            let cdr = list.cdr().unwrap();
            assert_eq!(cdr, $list_macro![2, 3, 4, 5]);
            let list = $list_macro![5];
            let cdr = list.cdr();
            assert!(cdr.is_none());
        }

        #[test]
        fn cdr_mut() {
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
        fn append_mut() {
            let mut left = $list_macro![1usize, 2, 3];
            let right = $list_macro![4usize, 5, 6];
            left.append_mut(right);
            assert_eq!(left, $list_macro![1, 2, 3, 4, 5, 6])
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
            let mut list = $list_macro![1usize, 2, 3];
            let vec = vec![4, 5, 6];
            list.extend(vec);
            assert_eq!(list, $list_macro![1, 2, 3, 4, 5, 6])
        }

        #[test]
        fn sort() {
            let mut list = $list_macro![5, 4, 3, 2, 1];
            list.sort();
            assert_eq!(list, $list_macro![1, 2, 3, 4, 5]);
        }

        #[test]
        fn sort_by() {
            let mut list = $list_macro![5, 4, 3, 2, 1];
            list.sort_by(Ord::cmp);
            assert_eq!(list, $list_macro![1, 2, 3, 4, 5]);
        }
    };
}

macro_rules! impl_iter {
    () => {
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
    };
}

macro_rules! impl_traits {
    ($list:tt, $rc_type:tt) => {
        impl<T: Clone> Default for $list<T> {
            fn default() -> Self {
                Self::new()
            }
        }

        impl<T: Clone> Extend<T> for $list<T> {
            fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
                self.append_mut(iter.into_iter().collect())
            }
        }

        // and we'll implement FromIterator
        impl<T: Clone> FromIterator<T> for $list<T> {
            fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
                $list(iter.into_iter().collect())
            }
        }

        impl<T: Clone> FromIterator<$list<T>> for $list<T> {
            fn from_iter<I: IntoIterator<Item = $list<T>>>(iter: I) -> Self {
                $list(iter.into_iter().map(|x| x.0).collect())
            }
        }

        impl<T: Clone> From<Vec<T>> for $list<T> {
            fn from(vec: Vec<T>) -> Self {
                $list(vec.into_iter().collect())
            }
        }

        impl<T: Clone + std::fmt::Debug> std::fmt::Debug for $list<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_list().entries(self).finish()
            }
        }

        pub struct IterRef<'a, T: Clone>(IterWrapper<'a, T, $rc_type, $rc_type>);

        impl<'a, T: Clone> Iterator for IterRef<'a, T> {
            type Item = &'a T;

            impl_iter!();
        }

        impl<'a, T: Clone> IntoIterator for &'a $list<T> {
            type Item = &'a T;
            type IntoIter = IterRef<'a, T>;

            #[inline(always)]
            fn into_iter(self) -> Self::IntoIter {
                IterRef((&self.0).into_iter())
            }
        }

        pub struct Iter<T: Clone>(ConsumingWrapper<T, $rc_type, $rc_type>);

        impl<T: Clone> Iterator for Iter<T> {
            type Item = T;

            impl_iter!();
        }

        impl<T: Clone> IntoIterator for $list<T> {
            type Item = T;
            type IntoIter = Iter<T>;

            #[inline(always)]
            fn into_iter(self) -> Self::IntoIter {
                Iter(self.0.into_iter())
            }
        }
    };
}
