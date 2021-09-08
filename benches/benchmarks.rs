use criterion::{black_box, criterion_group, criterion_main, Criterion};
use im_lists::{list::List, shared_list::SharedList};

use im_rc::Vector;
use std::collections::LinkedList;

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
            let iter = (0..100000usize).into_iter().rev();
            let last = List::new();
            black_box(iter.fold(last, |accum, next| List::cons(next, accum)))
        })
    });
}

fn cdr_iteration(c: &mut Criterion) {
    c.bench_function("unrolled-cdr-iteration", |b| {
        b.iter(|| {
            black_box({
                let mut list: Option<List<_>> = Some((0..100000usize).into_iter().collect());

                while let Some(car) = list.as_ref().map(|x| x.car()).flatten() {
                    black_box(car);
                    list = list.unwrap().cdr();
                }
            });
        })
    });
}

iteration! {
    size = 100000usize,
    (unrolled_rc_iteration, List<_>),
    (unrolled_arc_iteration, SharedList<_>),
    (immutable_vector_iteration, Vector<_>),
    (vec_iteration, Vec<_>),
    (linked_list_iteration, LinkedList<_>)
}

construction! {
    size = 100000usize,
    (unrolled_rc_construction, List<_>),
    (immutable_vector_construction, Vector<_>),
    (vec_construction, Vec<_>)
}

criterion_group!(
    benches,
    // Iteration
    unrolled_rc_iteration,
    unrolled_arc_iteration,
    cdr_iteration,
    immutable_vector_iteration,
    vec_iteration,
    linked_list_iteration,
    // Construction
    unrolled_rc_construction,
    immutable_vector_construction,
    vec_construction,
    cons_up_list,
);

criterion_main!(benches);
