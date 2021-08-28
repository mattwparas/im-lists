use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lists::list::{ArcLinkedList, RcLinkedList};
use lists::unrolled::{ArcList, RcList};

use im_rc::Vector;

macro_rules! iteration {
    (size = $number:expr, $(($func_name:ident, $type:ty)),* $(,)?) => {
        $(
            fn $func_name(c: &mut Criterion) {
                let list = (0..10000usize).into_iter().collect::<$type>();

                c.bench_function(stringify!($func_name), |b| {
                    b.iter(|| black_box((&list).into_iter().sum::<usize>()))
                });
            }
        )*
    };
}

macro_rules! construction {
    ($(($func_name:ident, $type:ty)),* $(,)?) => {
        $(
            fn $func_name(c: &mut Criterion) {
                c.bench_function(stringify!($func_name), |b| {
                    b.iter(|| black_box((0..10000usize).into_iter().collect::<$type>()))
                });
            }
        )*
    }
}

// fn cons_up_list(c: &mut Criterion) {
//     c.bench_function("cons-unrolled-list", |b| {
//         b.iter(|| {
//             let mut iter = (0..10000usize).into_iter().rev();
//             let last = List::cons_empty(iter.next().unwrap());
//             black_box(iter.fold(last, |accum, next| List::cons_raw(next, accum)))
//         })
//     });
// }

iteration! {
    size = 10000,
    (unrolled_rc_iteration, RcList<_>),
    (unrolled_arc_iteration, ArcList<_>),
    (linked_list_rc_iteration, RcLinkedList<_>),
    (linked_list_arc_iteration, ArcLinkedList<_>),
    (immutable_vector_iteration, Vector<_>),
    (vec_iteration, Vec<_>)
}

construction! {
    (unrolled_rc_construction, RcList<_>),
    (linked_list_rc_construction, RcLinkedList<_>),
    (immutable_vector_construction, Vector<_>),
    (vec_construction, Vec<_>)
}

criterion_group!(
    benches,
    // Iteration
    unrolled_rc_iteration,
    unrolled_arc_iteration,
    linked_list_rc_iteration,
    linked_list_arc_iteration,
    immutable_vector_iteration,
    vec_iteration,
    // Construction
    unrolled_rc_construction,
    linked_list_rc_construction,
    immutable_vector_construction,
    vec_construction,
    // cons_up_list
);

criterion_main!(benches);
