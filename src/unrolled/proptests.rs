use proptest::prelude::*;

use super::*;

// Define strategies here for property tests

type List<T> = RcList<T>;

// Defines an arbitrary list containing elements from -10000 to 10000
fn list_strategy_from_iterator() -> impl Strategy<Value = List<isize>> {
    prop::collection::vec(-10000..10000isize, 0..10000).prop_map(|x| x.into_iter().collect())
}

// Generate arbitrary sequence of manipulations to both a vector and a list
// Apply those manipulations in order, then check that the state of both is the same
// If the state of the resulting is the same AND the invariants of the list hold, we're good
enum Actions {
    Cons,
    Cdr,
    Append(Vec<usize>),
}

proptest! {
    #[test]
    fn append_resulting_length_equivalent(left in list_strategy_from_iterator(), right in list_strategy_from_iterator()) {
        let mut left = left;

        let left_length = left.len();
        let right_length = right.len();

        left.append_mut(right);

        left.assert_invariants();

        assert_eq!(left.len(), left_length + right_length);
    }

    #[test]
    fn append_non_mut_resulting_length_equivalent(left in list_strategy_from_iterator(), right in list_strategy_from_iterator()) {
        let mut left = left;
        let left_length = left.len();
        let right_length = right.len();

        left = left.append(right);

        left.assert_invariants();

        assert_eq!(left.len(), left_length + right_length);
    }

    #[test]
    fn list_creation_from_iterator_has_correct_number_of_values(size in 0..10000usize) {
        let list = (0..size).into_iter().collect::<List<_>>();
        assert_eq!(list.len(), size);
    }

    #[test]
    fn indexing_correctly_lines_up(size in 0..10000usize) {
        let list = (0..size).into_iter().collect::<List<_>>();
        for i in 0..list.len() {
            assert_eq!(i, list.get(i).unwrap());
        }
    }
}
