use criterion::{black_box, criterion_group, criterion_main, Criterion};
use im_lists::list::{ArcLinkedList, RcLinkedList};
use im_lists::unrolled::{ArcList, RcList};

use im_rc::Vector;

macro_rules! iteration {
    (size = $number:expr, $(($func_name:ident, $type:ty)),* $(,)?) => {
        $(
            fn $func_name(c: &mut Criterion) {
                let list = (0..$number).into_iter().collect::<$type>();

                c.bench_function(stringify!($func_name), |b| {
                    b.iter(|| black_box((&list).into_iter().sum::<usize>()))
                });
            }
        )*
    };
}

macro_rules! construction {
    (size = $number:expr, $(($func_name:ident, $type:ty)),* $(,)?) => {
        $(
            fn $func_name(c: &mut Criterion) {
                c.bench_function(stringify!($func_name), |b| {
                    b.iter(|| black_box((0..$number).into_iter().collect::<$type>()))
                });
            }
        )*
    }
}

fn cons_up_list(c: &mut Criterion) {
    c.bench_function("cons-unrolled-list", |b| {
        b.iter(|| {
            let iter = (0..10000usize).into_iter().rev();
            let last = RcList::new();
            black_box(iter.fold(last, |accum, next| RcList::cons(next, accum)))
        })
    });
}

fn cdr_iteration(c: &mut Criterion) {
    c.bench_function("unrolled-cdr-iteration", |b| {
        b.iter(|| {
            black_box({
                let mut list: Option<RcList<_>> = Some((0..10000usize).into_iter().collect());

                while let Some(car) = list.as_ref().map(|x| x.car()).flatten() {
                    black_box(car);
                    list = list.unwrap().cdr();
                }
            });
        })
    });
}

fn unrolled_test_iter(c: &mut Criterion) {
    let list = (0..100000usize).into_iter().collect::<RcList<_>>();
    c.bench_function("unrolled-test-iter", |b| {
        b.iter(|| {
            black_box(list.test_iter().sum::<usize>());
        })
    });
}

iteration! {
    size = 100000usize,
    (unrolled_rc_iteration, RcList<_>),
    (unrolled_arc_iteration, ArcList<_>),
    (linked_list_rc_iteration, RcLinkedList<_>),
    (linked_list_arc_iteration, ArcLinkedList<_>),
    (immutable_vector_iteration, Vector<_>),
    (vec_iteration, Vec<_>)
}

construction! {
    size = 100000usize,
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
    cdr_iteration,
    linked_list_rc_iteration,
    linked_list_arc_iteration,
    immutable_vector_iteration,
    vec_iteration,
    // Construction
    unrolled_rc_construction,
    linked_list_rc_construction,
    immutable_vector_construction,
    vec_construction,
    cons_up_list,
    unrolled_test_iter,
);

criterion_main!(benches);
