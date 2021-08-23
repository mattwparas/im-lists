use std::{ops::Deref, rc::Rc, sync::Arc};

use std::fmt::Debug;

// pub trait DummyClone {
//     fn dummy_clone(&self) -> Self;
// }

// impl<T: Clone> DummyClone for T {
//     fn dummy_clone(&self) -> Self {
//         self.clone()
//     }
// }

#[derive(Clone)]
pub struct RcConstructor {}

impl<T> SmartPointerConstructor<T> for RcConstructor {
    type RC = Rc<T>;

    fn unwrap(ptr: &Self::RC) -> T
    where
        T: Clone,
    {
        (**ptr).clone()
    }

    fn fmt(ptr: &Option<Self::RC>, f: &mut std::fmt::Formatter<'_>) -> Box<dyn std::fmt::Debug>
    where
        T: Debug,
    {
        // f.debug_tuple("").field(ptr).finish()
        // unimplemented!()
        // Box::new(ptr)

        unimplemented!()
    }
}

// impl std::fmt::Debug for RcConstructor {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("Rc").finish()
//     }
// }

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

    fn fmt(ptr: &Option<Self::RC>, f: &mut std::fmt::Formatter<'_>) -> Box<dyn std::fmt::Debug>
    where
        T: Debug,
    {
        unimplemented!()
    }
}

// #[derive(Clone)]
// pub struct BoxConstructor {}

// impl<T: Clone> SmartPointerConstructor<T> for BoxConstructor {
//     type RC = Box<T>;

//     fn unwrap(ptr: &Self::RC) -> T
//     where
//         T: Clone,
//     {
//         (**ptr).clone()
//     }

//     fn fmt(ptr: &Option<Self::RC>, f: &mut std::fmt::Formatter<'_>) -> Box<dyn std::fmt::Debug>
//     where
//         T: Debug,
//     {
//         unimplemented!()
//     }
// }

// #[derive(Clone)]
// pub struct GcConstructor {}

// impl<T: Clone + Debug> RefCountedConstructor<T> for GcConstructor {
//     type RC = Gc<T>;

//     fn unwrap(ptr: &Self::RC) -> T
//     where
//         T: Clone,
//     {
//         todo!()
//     }
// }

// Definition and impls for RefCounted
pub trait SmartPointer: Clone {
    type Target;

    fn new(obj: Self::Target) -> Self;
    fn strong_count(this: &Self) -> usize;

    fn unwrap(&self) -> Self::Target;
    fn try_unwrap(this: Self) -> Option<Self::Target>;
    fn get_mut(&mut self) -> Option<&mut Self::Target>;
    fn make_mut(&mut self) -> &mut Self::Target;
    fn ptr_eq(this: &Self, other: &Self) -> bool;
    fn as_ptr(&self) -> *const Self::Target;
}

// Avoid creating infinite types by using an additional trait to provide some
// indirection
pub trait SmartPointerConstructor<T>: Clone {
    type RC: SmartPointer<Target = T> + Deref<Target = T>;

    fn unwrap(ptr: &Self::RC) -> T
    where
        T: Clone;

    fn fmt(ptr: &Option<Self::RC>, f: &mut std::fmt::Formatter<'_>) -> Box<dyn std::fmt::Debug>
    where
        T: Debug;
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

    fn get_mut(&mut self) -> Option<&mut Self::Target> {
        Rc::get_mut(self)
    }

    fn make_mut(&mut self) -> &mut Self::Target {
        // Rc::make_mut(self)
        todo!()
    }

    fn ptr_eq(this: &Self, other: &Self) -> bool {
        Rc::ptr_eq(this, other)
    }

    fn as_ptr(&self) -> *const Self::Target {
        Rc::as_ptr(self)
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

    fn get_mut(&mut self) -> Option<&mut Self::Target> {
        todo!()
    }

    fn make_mut(&mut self) -> &mut Self::Target {
        todo!()
    }

    fn ptr_eq(this: &Self, other: &Self) -> bool {
        todo!()
    }

    fn as_ptr(&self) -> *const Self::Target {
        todo!()
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

    fn get_mut(&mut self) -> Option<&mut Self::Target> {
        todo!()
    }

    fn make_mut(&mut self) -> &mut Self::Target {
        todo!()
    }

    fn ptr_eq(this: &Self, other: &Self) -> bool {
        todo!()
    }

    fn as_ptr(&self) -> *const Self::Target {
        todo!()
    }
}

// #[derive(Clone)]
// pub struct Gc<T>(Rc<T>);

// impl<T: Clone + Debug> RefCounted for Gc<T> {
//     type Target = T;

//     fn new(obj: Self::Target) -> Self {
//         todo!()
//     }

//     fn strong_count(this: &Self) -> usize {
//         todo!()
//     }

//     fn unwrap(&self) -> Self::Target {
//         todo!()
//     }

//     fn try_unwrap(this: Self) -> Option<Self::Target> {
//         todo!()
//     }

//     fn get_mut(&mut self) -> Option<&mut Self::Target> {
//         todo!()
//     }

//     fn make_mut(&mut self) -> &mut Self::Target {
//         todo!()
//     }

//     fn ptr_eq(this: &Self, other: &Self) -> bool {
//         todo!()
//     }

//     fn as_ptr(&self) -> *const Self::Target {
//         todo!()
//     }
// }

// impl<T> Deref for Gc<T> {
//     type Target = T;

//     fn deref(&self) -> &Self::Target {
//         todo!()
//     }
// }
