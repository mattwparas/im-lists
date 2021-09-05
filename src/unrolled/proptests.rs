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
    actions.into_iter().fold(initial, |list, action| {
        let res = action.act_on_list(list);
        // println!("After: {:?}", res);
        res
    })
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
    prop::collection::vec(0..10000usize, 0..256 * 3)
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

    #[test]
    fn last_always_selects_last(vec in vec_strategy()) {
        let list: List<usize> = vec.clone().into();
        assert_eq!(list.last(), vec.last().cloned());
    }

    #[test]
    fn iterators_equal(vec in vec_strategy()) {
        let list: List<usize> = vec.into();

        assert!(Iterator::eq(list.iter(), list.test_iter()));

    }

    #[test]
    fn into_iter_equal(vec in vec_strategy()) {
        let list: List<usize> = vec.clone().into();
        assert!(Iterator::eq((&list).into_iter(), (&vec).into_iter()))
    }
}

fn random_test_runner(vec: Vec<usize>, actions: Vec<Action>) {
    let initial_list: List<usize> = vec.clone().into_iter().collect();

    let resulting_list = crunch_actions_for_list(initial_list, actions.clone());
    let resulting_vector = crunch_actions_for_vec(vec, actions);

    resulting_list.assert_invariants();

    // println!("List length: {}", resulting_list.len());
    // println!("vector length: {}", resulting_vector.len());

    // println!("List: {:?}", resulting_list);
    // println!("Vector: {:?}", resulting_vector);

    assert!(Iterator::eq(resulting_list.iter(), resulting_vector.iter()));
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

#[test]
fn larger_test_case() {
    use Action::*;

    let vec = vec![
        5241, 5959, 8782, 6667, 2789, 4359, 1850, 2295, 391, 6551, 5960, 4692, 2762, 1362, 5179,
        8717, 5127, 4583, 6431, 3896, 2204, 9361, 269, 2618, 8346, 8991, 3965, 5793, 2100, 9501,
        2143, 4702, 2449, 5619, 7991, 8464, 9809, 9079, 7563, 2694, 4346, 4906, 3391, 7043, 3464,
        5213, 155, 8666, 8444, 2004, 2191, 237, 7115, 7776, 4411, 1602, 2963, 7241, 7450, 7432,
        6051, 8776, 9341, 5027, 6385, 8240, 8682, 9240, 5725, 2219, 908, 7019, 4254, 7193, 2673,
        3003, 6895, 4208, 6727, 4425, 3121, 5763, 553, 974, 2738, 8273, 497, 2299, 761, 2173, 7772,
        1252, 8844, 812, 9828, 9930, 5596, 2638, 4767, 5024, 2037, 8878, 2956, 1855, 776, 5298,
        1168, 7133, 9854, 8055, 1971, 6933, 2727, 2036, 481, 9667, 5537, 6826, 802, 8033, 2200,
        9260, 8996, 7414, 5229, 5159, 8382, 428, 2153, 6885, 6536, 8937, 4935, 2163, 737, 5859,
        7431, 2533, 6117, 1165, 8078, 2966, 2837, 974, 1953, 8364, 4655, 9230, 1272, 2471, 1594,
        6561, 6144, 5483, 8858, 6634, 310, 8664, 1340, 9032, 3091, 2967, 8291, 8107, 7619, 715,
        5694, 4026, 5947, 1709, 8159, 6187, 58, 3078, 2825, 307, 5247, 5653, 3725, 2264, 1982,
        4940, 5171, 614, 4550, 1568, 9348, 2554, 682, 419, 8126, 6235, 8142, 8562, 804, 6462, 1124,
        853, 7040, 105, 8806, 8474, 275, 4038, 4792, 180, 6580, 1300, 7606, 296, 5806, 4093, 6757,
        9125, 4023, 592, 4325, 8455, 1921, 6201, 1066, 650, 7158, 7750, 3714, 3349, 8973, 9400,
        9514, 9330, 9128, 3437, 5254, 7512, 7205, 6547, 5293, 8543, 8527, 4529, 4192, 4789, 3668,
        7583, 7137, 8034, 3767, 6063, 2633, 3813, 3695, 9958, 8994, 1315, 7669, 3407,
    ];

    // TODO check if cdr is the problem here
    let actions = vec![
        Cdr,
        Append(vec![80, 12, 75, 0, 0, 36, 36, 66, 79, 19, 10, 57]),
        Cdr,
        Append(vec![]),
        Cons(3298191517146272390),
    ];

    random_test_runner(vec, actions);
}
