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
        // println!("Action: {:?}, List: {:?}", self, list);
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

fn crunch_actions_for_vec(initial: Vec<usize>, actions: Vec<Action>) -> Vec<usize> {
    actions
        .into_iter()
        .fold(initial, |vec, action| action.act_on_vector(vec))
}

fn crunch_actions_for_list(initial: List<usize>, actions: Vec<Action>) -> List<usize> {
    actions
        .into_iter()
        .fold(initial, |list, action| action.act_on_list(list))
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
    // #![proptest_config(ProptestConfig::with_cases(5))]
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
    fn operations_in_order_match(vec in vec_strategy(), actions in actions_strategy()) {
        random_test_runner(vec, actions);
    }

    #[test]
    fn iteration_using_cdr(vec in vec_strategy()) {
        let mut list: List<usize> = vec.clone().into();
        while let Some(cdr) = list.cdr_mut() {
            cdr.car().expect("Missing value from car");
        }
    }

    #[test]
    fn len_decreases_by_one_with_cdr(vec in vec_strategy()) {
        cdr_returns_smaller_vec(vec);
    }

    #[test]
    fn reverse_matches_expected(vec in vec_strategy()) {
        let mut list: List<usize> = vec.clone().into();
        list = list.reverse();

        let mut vec = vec;
        vec.reverse();

        assert!(Iterator::eq(list.into_iter(), vec.into_iter()));
    }
}

fn random_test_runner(vec: Vec<usize>, actions: Vec<Action>) {
    let initial_list: List<usize> = vec.clone().into_iter().collect();

    let resulting_list = crunch_actions_for_list(initial_list, actions.clone());
    let resulting_vector = crunch_actions_for_vec(vec, actions);

    resulting_list.assert_invariants();
    assert!(Iterator::eq(
        resulting_list.into_iter(),
        resulting_vector.into_iter()
    ));
}

fn cdr_returns_smaller_vec(vec: Vec<usize>) {
    let mut list: List<usize> = vec.clone().into();
    let mut length = list.len();
    while let Some(cdr) = list.cdr_mut() {
        length -= 1;
        assert_eq!(length, cdr.len())
    }
}

#[test]
fn gets_smaller_with_cdr() {
    let vec = vec![1, 2, 3, 4, 5];

    cdr_returns_smaller_vec(vec)
}

#[test]
fn subsequent_cdrs_failing() {
    use Action::*;

    let vec = vec![
        1397, 4198, 9496, 9048, 3133, 3069, 381, 5056, 3597, 9667, 8192, 4648, 5778, 6622, 7350,
        1781, 3544, 8277, 5832, 4265, 4455, 3792, 3066, 4106, 718, 4975, 4972, 6811, 3644, 4008,
        790, 5699, 6137, 8578, 3636, 7932, 3058, 6147, 2421, 1666, 221, 9354, 2043, 5094, 5878,
        8554, 3760, 4492, 6504, 9340, 3160, 2592, 1369, 8728, 4235, 7024, 4173, 4190, 3499, 9509,
        7194, 8764, 9606, 5895, 894, 9372, 7560, 7405, 5994, 3055, 5472, 1020, 6708, 465, 4485,
    ];

    let actions = vec![
        Cdr,
        Cdr,
        Cdr,
        Cdr,
        Cdr,
        Cdr,
        Cons(9264784016065117665),
        Cons(2977697179080415033),
    ];

    random_test_runner(vec, actions);
}

#[test]
fn cdr_to_append() {
    use Action::*;

    let vec = vec![2033, 9558, 2726, 6383, 5557, 8720, 2270, 9933];

    let actions = vec![
        Cdr,
        Append(vec![93, 14, 88, 6, 70, 45, 71, 22, 65]),
        Cdr,
        Cons(13552607039695591312),
        Cons(11950321023595395400),
    ];

    random_test_runner(vec, actions);
}
