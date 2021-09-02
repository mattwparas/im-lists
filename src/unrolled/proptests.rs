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
#[derive(Debug, Clone)]
enum Action {
    Cons(usize),
    Cdr,
    Append(Vec<usize>),
}

impl Action {
    fn act_on_vector(self, mut vec: Vec<usize>) -> Vec<usize> {
        match self {
            Action::Cons(value) => {
                vec.insert(0, value);
                vec
            }
            Action::Cdr => {
                if vec.len() == 0 {
                    vec
                } else {
                    vec.remove(0);
                    vec
                }
            }
            Action::Append(mut right) => {
                vec.append(&mut right);
                vec
            }
        }
    }

    fn act_on_list(self, mut list: List<usize>) -> List<usize> {
        match self {
            Action::Cons(value) => {
                list.cons_mut(value);
                list
            }
            Action::Cdr => list.cdr().unwrap_or(List::new()),
            Action::Append(right) => list.extend(right),
        }
    }
}

fn crunch_actions_for_vec(mut initial: Vec<usize>, actions: Vec<Action>) -> Vec<usize> {
    for action in actions {
        initial = action.act_on_vector(initial);
    }
    initial
}

fn crunch_actions_for_list(mut initial: List<usize>, actions: Vec<Action>) -> List<usize> {
    for action in actions {
        initial = action.act_on_list(initial);
    }
    initial
}

fn action_strategy() -> impl Strategy<Value = Action> {
    prop_oneof![
        any::<usize>().prop_map(|x| Action::Cons(x)),
        Just(Action::Cdr),
        prop::collection::vec(0..100usize, 0..100)
            .prop_map(|x| Action::Append(x.into_iter().collect()))
    ]
}

fn actions_strategy() -> impl Strategy<Value = Vec<Action>> {
    prop::collection::vec(action_strategy(), 0..10)
}

fn vec_strategy() -> impl Strategy<Value = Vec<usize>> {
    prop::collection::vec(0..10000usize, 0..100)
}

proptest! {
    // The next line modifies the number of tests.
    #![proptest_config(ProptestConfig::with_cases(5))]
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

    #[test]
    #[ignore = "test is broken right now"]
    fn operations_in_order_match(vec in vec_strategy(), actions in actions_strategy()) {
        let initial_list: List<usize> = vec.clone().into_iter().collect();

        println!("{:?}", actions);

        let resulting_list = crunch_actions_for_list(initial_list, actions.clone());
        let resulting_vector = crunch_actions_for_vec(vec, actions);

        println!("list: {:?}", resulting_list);
        println!("vec: {:?}", resulting_vector);

        for (left, right) in resulting_list.into_iter().zip(resulting_vector.into_iter()) {
            assert_eq!(left, right)
        }
    }
}
