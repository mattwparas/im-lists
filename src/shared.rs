use std::{ops::Deref, rc::Rc, sync::Arc};

pub trait PointerFamily {
    type Pointer<T>: Deref<Target = T>;

    fn new<T>(value: T) -> Self::Pointer<T>;
    fn strong_count<T>(this: &Self::Pointer<T>) -> usize;
    fn try_unwrap<T>(this: Self::Pointer<T>) -> Option<T>;
    fn get_mut<T>(this: &mut Self::Pointer<T>) -> Option<&mut T>;
    fn ptr_eq<T>(this: &Self::Pointer<T>, other: &Self::Pointer<T>) -> bool;
    fn make_mut<T: Clone>(ptr: &mut Self::Pointer<T>) -> &mut T;
}

struct RcPointer;

impl PointerFamily for RcPointer {
    type Pointer<T> = Rc<T>;

    fn new<T>(value: T) -> Self::Pointer<T> {
        Rc::new(value)
    }

    fn strong_count<T>(this: &Self::Pointer<T>) -> usize {
        Rc::strong_count(this)
    }

    fn try_unwrap<T>(this: Self::Pointer<T>) -> Option<T> {
        Rc::try_unwrap(this).ok()
    }

    fn get_mut<T>(this: &mut Self::Pointer<T>) -> Option<&mut T> {
        Rc::get_mut(this)
    }

    fn ptr_eq<T>(this: &Self::Pointer<T>, other: &Self::Pointer<T>) -> bool {
        Rc::ptr_eq(this, other)
    }

    fn make_mut<T: Clone>(ptr: &mut Self::Pointer<T>) -> &mut T {
        Rc::make_mut(ptr)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RcConstructor {}

impl<T> SmartPointerConstructor<T> for RcConstructor {
    type RC = Rc<T>;

    fn make_mut(ptr: &mut Self::RC) -> &mut T
    where
        T: Clone,
    {
        Rc::make_mut(ptr)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ArcConstructor {}

impl<T> SmartPointerConstructor<T> for ArcConstructor {
    type RC = Arc<T>;

    fn make_mut(ptr: &mut Self::RC) -> &mut T
    where
        T: Clone,
    {
        Arc::make_mut(ptr)
    }
}

// Definition and impls for RefCounted
pub(crate) trait SmartPointer: Clone {
    type Target;

    fn new(obj: Self::Target) -> Self;
    fn strong_count(this: &Self) -> usize;
    fn try_unwrap(this: Self) -> Option<Self::Target>;
    fn get_mut(this: &mut Self) -> Option<&mut Self::Target>;
    fn ptr_eq(this: &Self, other: &Self) -> bool;
}

// Avoid creating infinite types by using an additional trait to provide some
// indirection
pub(crate) trait SmartPointerConstructor<T>: Clone {
    type RC: SmartPointer<Target = T> + Deref<Target = T>;

    fn make_mut(ptr: &mut Self::RC) -> &mut T
    where
        T: Clone;
}

impl<T> SmartPointer for Rc<T> {
    type Target = T;

    fn new(obj: T) -> Rc<T> {
        Rc::new(obj)
    }

    fn strong_count(this: &Rc<T>) -> usize {
        Rc::strong_count(this)
    }

    fn try_unwrap(this: Self) -> Option<Self::Target> {
        Rc::try_unwrap(this).ok()
    }

    fn get_mut(this: &mut Self) -> Option<&mut Self::Target> {
        Rc::get_mut(this)
    }

    fn ptr_eq(this: &Self, other: &Self) -> bool {
        Rc::ptr_eq(this, other)
    }
}

impl<T> SmartPointer for Arc<T> {
    type Target = T;

    fn new(obj: T) -> Arc<T> {
        Arc::new(obj)
    }

    fn strong_count(this: &Arc<T>) -> usize {
        Arc::strong_count(this)
    }

    fn try_unwrap(this: Self) -> Option<Self::Target> {
        Arc::try_unwrap(this).ok()
    }

    fn get_mut(this: &mut Self) -> Option<&mut Self::Target> {
        Arc::get_mut(this)
    }

    fn ptr_eq(this: &Self, other: &Self) -> bool {
        Arc::ptr_eq(this, other)
    }
}

#[cfg(test)]
mod shared_pointer_tests {
    use super::*;

    #[test]
    fn rc_ptr_eq() {
        let one = <RcConstructor as SmartPointerConstructor<usize>>::RC::new(10);

        let two = <RcConstructor as SmartPointerConstructor<usize>>::RC::clone(&one);

        assert!(<RcConstructor as SmartPointerConstructor<usize>>::RC::ptr_eq(&one, &two));
    }

    #[test]
    fn arc_ptr_eq() {
        let one = <ArcConstructor as SmartPointerConstructor<usize>>::RC::new(10);

        let two = <ArcConstructor as SmartPointerConstructor<usize>>::RC::clone(&one);

        assert!(<ArcConstructor as SmartPointerConstructor<usize>>::RC::ptr_eq(&one, &two));
    }

    #[test]
    fn arc_try_unwrap() {
        let one = <ArcConstructor as SmartPointerConstructor<usize>>::RC::new(10);

        assert!(<ArcConstructor as SmartPointerConstructor<usize>>::RC::try_unwrap(one).is_ok());
    }

    #[test]
    fn arc_get_mut() {
        let mut one = <ArcConstructor as SmartPointerConstructor<usize>>::RC::new(10);

        let mut_ref =
            <ArcConstructor as SmartPointerConstructor<usize>>::RC::get_mut(&mut one).unwrap();

        *mut_ref = 20;

        assert_eq!(one.as_ref(), &20);
    }
}
