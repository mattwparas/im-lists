use std::{ops::Deref, rc::Rc, sync::Arc};

#[derive(Clone, PartialEq)]
pub struct RcConstructor {}

impl<T> SmartPointerConstructor<T> for RcConstructor {
    type RC = Rc<T>;

    fn unwrap(ptr: &Self::RC) -> T
    where
        T: Clone,
    {
        (**ptr).clone()
    }

    fn make_mut(ptr: &mut Self::RC) -> &mut T
    where
        T: Clone,
    {
        Rc::make_mut(ptr)
    }
}

#[derive(Clone)]
pub struct ArcConstructor {}

impl<T> SmartPointerConstructor<T> for ArcConstructor {
    type RC = Arc<T>;

    fn unwrap(ptr: &Self::RC) -> T
    where
        T: Clone,
    {
        (**ptr).clone()
    }

    fn make_mut(ptr: &mut Self::RC) -> &mut T
    where
        T: Clone,
    {
        Arc::make_mut(ptr)
    }
}

// Definition and impls for RefCounted
pub trait SmartPointer: Clone {
    type Target;

    fn new(obj: Self::Target) -> Self;
    fn strong_count(this: &Self) -> usize;

    fn unwrap(&self) -> Self::Target;
    fn try_unwrap(this: Self) -> Option<Self::Target>;
    fn get_mut(this: &mut Self) -> Option<&mut Self::Target>;
    // fn make_mut(this: &mut Self) -> &mut Self::Target;
    fn ptr_eq(this: &Self, other: &Self) -> bool;
    fn as_ptr(&self) -> *const Self::Target;

    fn get_mut_unchecked(this: &mut Self) -> Option<&mut Self::Target>;
}

// Avoid creating infinite types by using an additional trait to provide some
// indirection
pub trait SmartPointerConstructor<T>: Clone {
    type RC: SmartPointer<Target = T> + Deref<Target = T>;

    fn unwrap(ptr: &Self::RC) -> T
    where
        T: Clone;

    fn make_mut(ptr: &mut Self::RC) -> &mut T
    where
        T: Clone;
}

// trait Clone

impl<T> SmartPointer for Rc<T> {
    type Target = T;

    fn new(obj: T) -> Rc<T> {
        Rc::new(obj)
    }

    fn strong_count(this: &Rc<T>) -> usize {
        Rc::strong_count(this)
    }

    fn unwrap(&self) -> Self::Target {
        // (**self).clone()
        todo!()

        // *Rc::make_mut(self)
    }

    fn try_unwrap(this: Self) -> Option<Self::Target> {
        Rc::try_unwrap(this).ok()
    }

    fn get_mut(this: &mut Self) -> Option<&mut Self::Target> {
        Rc::get_mut(this)
    }

    // fn make_mut(this: &mut Self) -> &mut Self::Target {
    //     // Rc::make_mut(self)
    //     todo!()
    // }

    fn ptr_eq(this: &Self, other: &Self) -> bool {
        Rc::ptr_eq(this, other)
    }

    fn as_ptr(&self) -> *const Self::Target {
        Rc::as_ptr(self)
    }

    fn get_mut_unchecked(this: &mut Self) -> Option<&mut Self::Target> {
        // todo!()

        // unsafe { &mut (Rc::as_ptr(self).value}

        unsafe { (Rc::as_ptr(this) as *mut T).as_mut() }

        // pub unsafe fn get_mut_unchecked(this: &mut Self) -> &mut T {
        //     // We are careful to *not* create a reference covering the "count" fields, as
        //     // this would conflict with accesses to the reference counts (e.g. by `Weak`).
        //     unsafe { &mut (*this.ptr.as_ptr()).value }
        // }
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

    fn unwrap(&self) -> Self::Target {
        todo!()
    }

    fn try_unwrap(this: Self) -> Option<Self::Target> {
        Arc::try_unwrap(this).ok()
    }

    fn get_mut(this: &mut Self) -> Option<&mut Self::Target> {
        Arc::get_mut(this)
    }

    // fn make_mut(this: &mut Self) -> &mut Self::Target {
    //     todo!()
    // }

    fn ptr_eq(this: &Self, other: &Self) -> bool {
        todo!()
    }

    fn as_ptr(&self) -> *const Self::Target {
        todo!()
    }

    fn get_mut_unchecked(this: &mut Self) -> Option<&mut Self::Target> {
        unsafe { (Arc::as_ptr(this) as *mut T).as_mut() }
    }
}

impl<T: Clone> SmartPointer for Box<T> {
    type Target = T;

    fn new(obj: Self::Target) -> Self {
        Box::new(obj)
    }

    fn strong_count(this: &Self) -> usize {
        1
    }

    fn unwrap(&self) -> Self::Target {
        // *self.clone()
        todo!()
    }

    fn try_unwrap(this: Self) -> Option<Self::Target> {
        todo!()
    }

    fn get_mut(this: &mut Self) -> Option<&mut Self::Target> {
        todo!()
    }

    // fn make_mut(this: &mut Self) -> &mut Self::Target {
    //     todo!()
    // }

    fn ptr_eq(this: &Self, other: &Self) -> bool {
        todo!()
    }

    fn as_ptr(&self) -> *const Self::Target {
        todo!()
    }

    fn get_mut_unchecked(this: &mut Self) -> Option<&mut Self::Target> {
        todo!()
    }
}
