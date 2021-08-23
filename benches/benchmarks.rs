use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lists::list::{ArcLinkedList, RcLinkedList};
use lists::unrolled::{ArcList, List};

use im_rc::Vector;

macro_rules! iteration {
    ($(($func_name:ident, $type:ty)),* $(,)?) => {
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

iteration! {
    (unrolled_rc_iteration, List<_>),
    (unrolled_arc_iteration, ArcList<_>),
    (linked_list_rc_iteration, RcLinkedList<_>),
    (linked_list_arc_iteration, ArcLinkedList<_>),
    (immutable_vector, Vector<_>),
    (vec_iteration, Vec<_>)
}

construction! {
    (unrolled_rc_construction, List<_>),
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
    immutable_vector,
    vec_iteration,
    // Construction
    unrolled_rc_construction,
    linked_list_rc_construction,
    immutable_vector_construction,
    vec_construction
);

criterion_main!(benches);
