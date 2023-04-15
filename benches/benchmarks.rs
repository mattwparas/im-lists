use criterion::{black_box, criterion_group, criterion_main, Criterion};
use im_lists::list::{GenericList, List, SharedList};

use im_rc::Vector;
use std::collections::LinkedList;

macro_rules! iteration {
    ($group:expr, size = $number:expr, $(($func_name:ident, $type:ty)),* $(,)?) => {
        $(
            let list = (0..$number).into_iter().collect::<$type>();
            $group.bench_function(stringify!($func_name), |b| {
                b.iter(|| black_box((&list).into_iter().sum::<usize>()))
            });
        )*
    };
}

pub fn iteration_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("iteration");

    iteration!(
        group,
        size = 100000usize,
        (unrolled_rc_iteration, List<_>),
        (unrolled_arc_iteration, SharedList<_>),
        (
            vlist_rc_iteration,
            im_lists::list::GenericList<_, im_lists::shared::RcPointer, 4, 2>
        ),
        (immutable_vector_iteration, Vector<_>),
        (vec_iteration, Vec<_>),
        (linked_list_iteration, LinkedList<_>)
    );

    group.bench_function("unrolled-cdr-iteration", |b| {
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

macro_rules! construction {
    ($group:expr, size = $number:expr, $(($func_name:ident, $type:ty)),* $(,)?) => {
        $(
            $group.bench_function(stringify!($func_name), |b| {
                b.iter(|| black_box((0..$number).into_iter().collect::<$type>()))
            });

        )*
    }
}

pub fn construction_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("construction");

    construction!(
        group,
        size = 100000usize,
        (unrolled_rc_construction, List<_>),
        (unrolled_arc_construction, SharedList<_>),
        (
            vlist_rc_construction,
            im_lists::list::GenericList<_, im_lists::shared::RcPointer, 4, 2>
        ),
        (immutable_vector_construction, Vector<_>),
        (vec_construction, Vec<_>),
        (linked_list_construction, LinkedList<_>)
    );

    group.bench_function("cons-unrolled-list", |b| {
        b.iter(|| {
            let iter = (0..100000usize).into_iter().rev();
            let last = List::new();
            black_box(iter.fold(last, |accum, next| List::cons(next, accum)))
        })
    });
}

pub fn push_front_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("push_front");

    group.bench_function("push_front_list", |b| {
        b.iter(|| {
            let mut list = List::new();
            for i in 0..10000 {
                list.cons_mut(i);
            }
        })
    });

    group.bench_function("push_front_vlist", |b| {
        b.iter(|| {
            let mut list: im_lists::list::GenericList<_, im_lists::shared::RcPointer, 4, 2> =
                GenericList::new();
            for i in 0..10000 {
                list.cons_mut(i);
            }
        })
    });

    group.bench_function("push_front_vec", |b| {
        b.iter(|| {
            let mut vec = Vec::new();
            for i in 0..10000 {
                vec.insert(0, i);
            }
        })
    });

    group.bench_function("push_front_linked_list", |b| {
        b.iter(|| {
            let mut vec = LinkedList::new();
            for i in 0..10000 {
                vec.push_front(i);
            }
        })
    });
}

criterion_group!(
    benches,
    iteration_bench,
    construction_bench,
    push_front_bench
);

criterion_main!(benches);
