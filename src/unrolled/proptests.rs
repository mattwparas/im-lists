use proptest::prelude::*;

use super::*;
use crate::shared::RcConstructor;

// Define strategies here for property tests
type List<T> = UnrolledList<T, RcConstructor, RcConstructor>;

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
    Reverse,
    PushBack(usize),
    PopFront,
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
            Action::Reverse => {
                vec.reverse();
                vec
            }
            Action::PushBack(value) => {
                vec.push(value);
                vec
            }
            Action::PopFront => {
                if vec.len() == 0 {
                    vec
                } else {
                    vec.remove(0);
                    vec
                }
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
            Action::Append(right) => {
                list.extend(right);
                list
            }
            Action::Reverse => list.reverse(),
            Action::PushBack(value) => {
                list.push_back(value);
                list
            }
            Action::PopFront => {
                // println!("Before pop: {:?}", list);
                // println!("list elements: {:?}", list.elements());
                list.pop_front();
                // println!("After pop: {:?}", list);
                // println!("list elements: {:?}", list.elements());
                list
            }
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
        any::<usize>().prop_map(Action::Cons),
        Just(Action::Cdr),
        prop::collection::vec(0..100usize, 0..100)
            .prop_map(|x| Action::Append(x.into_iter().collect())),
        Just(Action::Reverse),
        any::<usize>().prop_map(Action::PushBack),
        Just(Action::PopFront)
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
            assert_eq!(i, *list.get(i).unwrap());
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
        assert_eq!(list.last(), vec.last());
    }

    // #[test]
    // fn iterators_equal(vec in vec_strategy()) {
    //     let list: List<usize> = vec.into();

    //     assert!(Iterator::eq(list.iter(), list.test_iter()));

    // }

    #[test]
    fn into_iter_equal(vec in vec_strategy()) {
        let list: List<usize> = vec.clone().into();
        assert!(Iterator::eq((&list).into_iter(), (&vec).into_iter()))
    }

    #[test]
    fn reverse_twice_results_in_same(list in list_strategy_from_iterator()) {
        let initial = list.clone();
        let resulting = list.reverse().reverse();
        assert!(Iterator::eq(initial.into_iter(), resulting.into_iter()));
    }

    #[test]
    fn iterators_equivalent(list in list_strategy_from_iterator()) {
        assert!(Iterator::eq(list.iter(), (&list).into_iter()));
    }

    #[test]
    fn simple_sorting(mut vec in vec_strategy()) {
        let mut list: List<_> = vec.clone().into();
        list.sort();
        vec.sort();
        assert!(Iterator::eq(list.iter(), vec.iter()));
    }

    #[test]
    fn simple_take(vec in vec_strategy(), count in (0..256*3usize)) {
        let list: List<_> = vec.clone().into();

        assert!(Iterator::eq(list.take(count).iter(), vec.iter().take(count)))
    }

    #[test]
    fn tail(vec in vec_strategy(), count in (0..256*4usize)) {
        let list: List<_> = vec.clone().into();

        let tail = list.tail(count);

        if count > list.len() {
            assert!(tail.is_none())
        } else {
            assert!(tail.is_some());
            assert_eq!(tail.unwrap().len() + count, list.len());
        }

        // else {
        //     assert_eq!(tail.unwrap().len(), list.len() - count);
        // }
    }
}

fn random_test_runner(vec: Vec<usize>, actions: Vec<Action>) {
    let initial_list: List<usize> = vec.clone().into_iter().collect();

    let resulting_list = crunch_actions_for_list(initial_list, actions.clone());
    let resulting_vector = crunch_actions_for_vec(vec, actions);

    resulting_list.assert_invariants();

    println!("List length: {}", resulting_list.len());
    println!("vector length: {}", resulting_vector.len());

    println!("List: {:?}", resulting_list);
    println!("Vector: {:?}", resulting_vector);

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

#[test]
fn test_case_with_reverse() {
    use Action::*;

    let vec = vec![
        6803, 1646, 9508, 8285, 2735, 9999, 7491, 9889, 3410, 8917, 823, 8026, 4030, 2800, 1092,
        238, 3414, 5438, 3859, 6898, 6636, 8270, 2689, 7622, 2849, 1403, 697, 3861, 9509, 9331,
        1160, 9766, 6783, 7506, 8055, 562, 8124, 9754, 7966, 6350, 6354, 6939, 9280, 6717, 7703,
        9289, 7774, 4160, 8299, 2551, 4406, 5278, 5462, 6521, 7916, 8075, 9940, 9804, 5581, 3572,
        5733, 7182, 1459, 9577, 8108, 3933, 1424, 9431, 5153, 8307, 5486, 6987, 1427, 5438, 8506,
        9183, 1532, 7186, 6021, 6350, 8066, 6040, 7190, 4495, 1745, 5535, 2105, 9267, 610, 4760,
        9037, 8830, 2173, 7579, 9237, 5980, 476, 9173, 2576, 8964, 9847, 5212, 7637, 8171, 7403,
        7291, 7724, 9818, 3352, 8958, 8556, 2707, 8697, 1695, 320, 5534, 9604, 7370, 1077, 2513,
        9056, 6272, 7621, 701, 9513, 2661, 4310, 691, 5337, 3357, 161, 5986, 5550, 8982, 5750, 363,
        8852, 1856, 2877, 7406, 2234, 280, 5160, 3672, 8811, 3661, 2692, 4903, 6303, 3356, 7714,
        8614, 864, 5582, 8107, 7335, 2047, 9146, 8148, 254, 1390, 4709, 5655, 9642, 6787, 6203,
        8547, 4899, 8662, 2554, 3399, 1712, 5455, 1703, 3646, 2602, 6056, 9469, 5517, 9271, 3188,
        8016, 7788, 4818, 4862, 7667, 5866, 2237, 4016, 3114, 3911, 6899, 6058, 502, 535, 4262,
        4675, 7042, 2729, 4854, 6445, 3577, 2263, 9762, 8867, 8065, 4685, 7628, 4034, 1677, 7480,
        8222, 4143, 3675, 3689, 3690, 4958, 5870, 7154, 2138, 29, 9705, 7990, 3670, 1636, 5239,
        6980, 1955, 6077, 2494, 7947, 3791, 4421, 6403, 1453, 4169, 9909, 9386, 7777, 2151, 8281,
        6904, 34, 2932, 8407, 7935, 2146, 6370, 6445, 3963, 2975, 7720, 5573, 724, 2291, 1413,
        4486, 5385, 6459, 7022, 5766, 7050, 4069, 7605, 1708, 4220, 3467, 3606, 8183, 3876, 2758,
        4795, 5467, 7926, 9967, 614, 512, 1113, 2879, 6005, 644, 754, 5746, 7889, 1549, 907, 6259,
        7509, 3999, 6474, 2053, 3508, 979, 476, 9665, 5182, 7520, 9172, 5559, 30, 2142, 1398, 226,
        7644, 8757, 1051, 1061, 5810, 3207, 47, 1147, 9156, 4610, 7109, 5333, 9018, 5718, 49, 9549,
        2121, 1732, 8652, 6877, 4704, 1610, 5913, 4559, 218, 9302, 932, 9883, 3854, 6161, 2255,
        8709, 453, 2745, 2156, 9936, 3123, 3612, 161, 6517, 1735, 4383, 2897, 5412, 9962, 4593,
        5999, 5799, 67, 6031, 921, 6726, 6782, 4127, 9291, 210, 8535, 2542, 2728, 3427, 224, 2034,
        210, 7564, 4263, 6544, 6291, 8720, 8763, 1415, 3374, 4329, 7436, 5722, 4916, 7507, 3463,
        3509, 6654, 5083, 2325, 8297, 2546, 7514, 3747, 4749, 9887, 1525, 1408, 9382, 2345, 6286,
        2975, 4084, 4352, 9353, 390, 6739, 4218, 5265, 65, 6293, 17, 2600, 1807, 9740, 6423, 9250,
        4958, 6252, 2155, 8439, 9670, 266, 2808, 9685, 4368, 2861, 4182, 4063, 6311, 742, 4431,
        7277, 6325, 2015, 3635, 2847, 3367, 8479, 3939, 958, 2935, 1450, 96, 4011, 1408, 5900,
        3175, 3847, 4627, 1326, 6685, 930, 8157, 1392, 7252, 3261, 751, 7929, 4518, 2577, 9362,
        8418, 6215, 3586, 7014, 36, 5409, 4687, 543, 7099, 5349, 9635, 5977, 934, 8350, 4558, 376,
        2593, 1164, 8361, 4592, 4597, 8550, 9473, 5846, 5978, 158, 9917, 8495, 1564, 2193, 9053,
        7211, 9380, 4299, 4076, 6121, 8378, 7766, 1983, 939, 3857, 6264, 8952, 2182, 60, 8922,
        1889, 5806, 4166, 5994, 1333, 7012, 5757, 3177, 6511, 6853, 3624, 4931,
    ];

    let actions = vec![
        Cdr,
        Cdr,
        Reverse,
        Cons(5983617890402927536),
        Cdr,
        Cdr,
        Append(vec![32, 37, 58, 81, 66, 58, 60, 6, 84, 0, 32]),
        Reverse,
    ];

    random_test_runner(vec, actions);
}

#[test]
fn large_tail_case() {
    let vec: Vec<usize> = vec![
        0, 0, 0, 0, 0, 47, 4360, 4958, 6341, 4310, 4228, 2128, 85, 1605, 9685, 3525, 2069, 2331,
        2279, 7768, 3316, 466, 6335, 9894, 7640, 1624, 3685, 6075, 263, 8435, 2859, 746, 2492, 746,
        2305, 4277, 2050, 7727, 27, 2182, 1182, 9996, 221, 4864, 2183, 5627, 8086, 2910, 6291,
        5206, 3913, 4194, 385, 9089, 3995, 3030, 5334, 7860, 2384, 3217, 6091, 3917, 8994, 5251,
        3828, 9557, 1904, 3914, 3799, 883, 3420, 4271, 520, 3482, 6190, 8199, 3158, 9626, 6497,
        3494, 499, 6116, 7045, 149, 9741, 9742, 7002, 9608, 7536, 824, 1456, 4640, 5974, 4866,
        8051, 9347, 9778, 6555, 5874, 1328, 360, 6765, 3589, 9707, 4038, 867, 4251, 5877, 5520,
        9355, 307, 3190, 4057, 5600, 2944, 959, 1130, 3027, 9969, 9918, 5409, 4172, 3441, 7903,
        4423, 6296, 1695, 2743, 6697, 3656, 3973, 4909, 554, 4910, 7916, 2251, 8920, 5986, 1197,
        8377, 5705, 5242, 2403, 6445, 1828, 728, 8326, 9630, 3318, 8792, 7409, 4510, 2753, 8352,
        6250, 4398, 8179, 9168, 554, 7539, 2774, 8221, 7091, 335, 1033, 7551, 9377, 1040, 7723,
        6241, 5469, 1283, 9057, 6555, 177, 365, 8384, 2042, 7290, 1689, 2373, 7744, 344, 6275, 61,
        6319, 8869, 5038, 2647, 7241, 6156, 1225, 3682, 6872, 6716, 3496, 5656, 46, 9396, 898, 499,
        992, 3350, 4941, 4974, 8187, 9456, 2545, 7049, 1905, 7419, 3933, 6435, 2208, 2822, 1091,
        1311, 3225, 8417, 8913, 7044, 4779, 7852, 5619, 6664, 2264, 1926, 6721, 2568, 2819, 2403,
        8487, 7006, 3464, 69, 4798, 7875, 8848, 9359, 495, 4217, 4866, 2839, 9458, 7841, 3805,
        7224, 3351, 9545, 5855, 1015, 5084, 8326, 9322, 4543, 7000, 3754, 6867, 6662, 8362, 8971,
        1682, 4834, 3143, 4732, 3357, 7331, 6269, 1842, 6344, 9221, 6823, 4774, 9703, 3607, 620,
        250, 9143, 7990, 8660, 8049, 5975, 1380, 2439, 5706, 4182, 3243, 7224, 9326, 8502, 8164,
        4979, 6285, 912, 5830, 6122, 7140, 6918, 7969, 1192, 2370, 1192, 9427, 3802, 693, 1466,
        1047, 9069, 6204, 818, 5139, 1393, 8971, 9839, 4936, 2820, 3965, 275, 3383, 9490, 2517,
        3665, 2848, 6207, 3697, 130, 820, 3652, 8642, 3313, 3834, 7442, 5425, 4142, 7107, 386,
        8650, 1081, 1801, 6890, 9279, 9538, 6116, 7932, 8385, 1083, 9099, 2641, 73, 1843, 2151,
        1039, 6298, 6015, 566, 5107, 1234, 9353, 3115, 1473, 9000, 6018, 5554, 812, 5951, 6703,
        8770, 7255, 2419, 8405, 8774, 2979, 7683, 3819, 9292, 8390, 3663, 202, 5944, 5680, 8890,
        2539, 7483, 558, 803, 6613, 5865, 7399, 3188, 9855, 4904, 527, 2355, 1469, 9738, 2602,
        3833, 9088, 5995, 6636, 8309, 5880, 4123, 3637, 7656, 3363, 3072, 382, 8373, 2576, 5352,
        827, 1141, 1081, 5399, 2630, 7645, 6042, 8481, 6286, 8816, 261, 3318, 5968, 5572, 8024,
        462, 9884, 1401, 4232, 7643, 5984, 3808, 3891, 9182, 1362, 9552, 4670, 755, 5596, 3456,
        5773, 3060, 8310, 7577, 34, 4328, 2095, 246, 4956, 9673, 6922, 1995, 107, 9856, 5030, 9585,
        1647, 7573, 7782, 4410, 8604, 3532, 749, 3088, 5480, 8572, 5335, 834, 252, 847, 4086, 5874,
        3449, 8496, 758, 7604, 6190, 1539,
    ];

    let count = 479usize;

    println!("Vec length: {}", vec.len());

    let list: List<_> = vec.clone().into();

    let tail = list.tail(count);

    if count > list.len() {
        assert!(tail.is_none())
    } else {
        assert!(tail.is_some());
        assert_eq!(tail.unwrap().len() + count, list.len());
    }
}

#[test]
fn test_case_with_pop_front() {
    use Action::*;

    let vec = vec![
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 18, 5736,
        4565, 2115, 8698, 7837, 4028, 1829, 1260, 1371, 337, 2812, 6058, 6692, 5494, 435, 6074,
        6006, 3648, 323, 1917, 921, 9537, 5695, 8082, 7948, 7588, 4575, 8334, 1386, 3132, 4254,
        1637, 9078, 7801, 3, 5774, 988, 9699, 1343, 1700, 2436, 256, 5538, 526, 2349, 5732, 1483,
        3540, 9386, 5981, 3972, 885, 5065, 4510, 7626, 8059, 1450, 3713, 4218, 1787, 8124, 546,
        9435, 7970, 9694, 6981, 9713, 8629, 3395, 4192, 1297, 2281, 3699, 8094, 814, 5180, 3752,
        1158, 9464, 1950, 9671, 1031, 2548, 5690, 5462, 392, 4761, 8679, 5836, 3684, 9613, 7873,
        6647, 8638, 1952, 4545, 4515, 7764, 2022, 7638, 1398, 1595, 9997, 6210, 3909, 3254, 9825,
        4210, 6406, 9982, 8625, 694, 75, 9261, 2974, 3955, 7796, 1086, 7294, 1399, 7215, 478, 4562,
        1237, 8461, 8932, 7867, 1022, 4035, 6190, 9648, 1802, 1853, 8532, 9865, 6603, 2447, 6647,
        6214, 2746, 560, 4637, 9917, 5030, 3867, 8388, 3738, 71, 3485, 7719, 455, 9631, 619, 4232,
        9239, 2367, 3019, 2845, 951, 8251, 662, 6283, 287, 1239, 6415, 1589, 1228, 1009, 9402,
        9089, 9627, 9026, 3848, 5218, 99, 2973, 2979, 6044, 4504, 1501, 2485, 8806, 2506, 9361,
        310, 5821, 5707, 5531, 1242, 2989, 6187, 98, 9691, 4996, 2520, 3107, 1740, 9222, 3853,
        2778, 405, 6474, 8801, 5049, 6924, 420, 8859, 3063, 2047, 3974, 9679, 9534, 7674, 1245,
        627, 9019, 4195, 9803, 1430, 6503, 9069, 3865, 7755, 2696, 7588, 2226, 1875, 7604,
    ];

    // println!("{}", vec.len());

    // println!(
    //     "{}",
    //     vec![
    //         40, 50, 59, 7, 50, 97, 62, 59, 18, 98, 79, 63, 8, 18, 6, 23, 31, 7, 36, 94, 29, 94, 21,
    //         4, 6, 67, 19, 10, 60, 61, 55, 30, 47, 4, 18, 40, 33, 27, 57, 19, 80, 92, 72, 6, 76, 93,
    //         81, 30, 5, 48, 85, 61, 98, 46, 12, 97, 70, 61, 50, 37, 30, 10, 75, 14,
    //     ]
    //     .len()
    // );

    let actions = vec![
        Append(vec![
            40, 50, 59, 7, 50, 97, 62, 59, 18, 98, 79, 63, 8, 18, 6, 23, 31, 7, 36, 94, 29, 94, 21,
            4, 6, 67, 19, 10, 60, 61, 55, 30, 47, 4, 18, 40, 33, 27, 57, 19, 80, 92, 72, 6, 76, 93,
            81, 30, 5, 48, 85, 61, 98, 46, 12, 97, 70, 61, 50, 37, 30, 10, 75, 14,
        ]),
        Cdr,
        Cons(6970920282707533934),
        Cons(17163059031184983451),
        Cdr,
        PushBack(4160322910230717775),
        PopFront,
        PushBack(320814026882087868),
        Cdr,
    ];

    random_test_runner(vec, actions);
}
