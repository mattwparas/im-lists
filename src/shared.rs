use std::{ops::Deref, rc::Rc, sync::Arc};

pub trait PointerFamily {
    type Pointer<T: ?Sized>: Deref<Target = T>;

    fn new<T>(value: T) -> Self::Pointer<T>;
    fn strong_count<T: ?Sized>(this: &Self::Pointer<T>) -> usize;
    fn try_unwrap<T>(this: Self::Pointer<T>) -> Option<T>;
    fn get_mut<T: ?Sized>(this: &mut Self::Pointer<T>) -> Option<&mut T>;
    fn ptr_eq<T: ?Sized>(this: &Self::Pointer<T>, other: &Self::Pointer<T>) -> bool;
    fn make_mut<T: Clone + ?Sized>(ptr: &mut Self::Pointer<T>) -> &mut T;
    fn clone<T: ?Sized>(ptr: &Self::Pointer<T>) -> Self::Pointer<T>;
    fn as_ptr<T: ?Sized>(this: &Self::Pointer<T>) -> *const T;
}

pub struct RcPointer;

impl PointerFamily for RcPointer {
    type Pointer<T: ?Sized> = Rc<T>;

    fn new<T>(value: T) -> Self::Pointer<T> {
        Rc::new(value)
    }

    fn strong_count<T: ?Sized>(this: &Self::Pointer<T>) -> usize {
        Rc::strong_count(this)
    }

    fn try_unwrap<T>(this: Self::Pointer<T>) -> Option<T> {
        Rc::try_unwrap(this).ok()
    }

    fn get_mut<T: ?Sized>(this: &mut Self::Pointer<T>) -> Option<&mut T> {
        Rc::get_mut(this)
    }

    fn ptr_eq<T: ?Sized>(this: &Self::Pointer<T>, other: &Self::Pointer<T>) -> bool {
        Rc::ptr_eq(this, other)
    }

    fn make_mut<T: Clone + ?Sized>(ptr: &mut Self::Pointer<T>) -> &mut T {
        Rc::make_mut(ptr)
    }

    fn clone<T: ?Sized>(ptr: &Self::Pointer<T>) -> Self::Pointer<T> {
        Rc::clone(ptr)
    }

    fn as_ptr<T: ?Sized>(this: &Self::Pointer<T>) -> *const T {
        Rc::as_ptr(this)
    }
}

pub struct ArcPointer;

impl PointerFamily for ArcPointer {
    type Pointer<T: ?Sized> = Arc<T>;

    fn new<T>(value: T) -> Self::Pointer<T> {
        Arc::new(value)
    }

    fn strong_count<T: ?Sized>(this: &Self::Pointer<T>) -> usize {
        Arc::strong_count(this)
    }

    fn try_unwrap<T>(this: Self::Pointer<T>) -> Option<T> {
        Arc::try_unwrap(this).ok()
    }

    fn get_mut<T: ?Sized>(this: &mut Self::Pointer<T>) -> Option<&mut T> {
        Arc::get_mut(this)
    }

    fn ptr_eq<T: ?Sized>(this: &Self::Pointer<T>, other: &Self::Pointer<T>) -> bool {
        Arc::ptr_eq(this, other)
    }

    fn make_mut<T: Clone + ?Sized>(ptr: &mut Self::Pointer<T>) -> &mut T {
        Arc::make_mut(ptr)
    }

    fn clone<T: ?Sized>(ptr: &Self::Pointer<T>) -> Self::Pointer<T> {
        Arc::clone(ptr)
    }

    fn as_ptr<T: ?Sized>(this: &Self::Pointer<T>) -> *const T {
        Arc::as_ptr(this)
    }
}
