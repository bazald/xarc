use super::{internal::*, local::*};
use crossbeam_epoch::{Guard, pin};
use std::{ptr, sync::atomic::{AtomicPtr, Ordering}};

/// `Xarc` is an atomic smart pointer.
#[derive(Debug)]
pub struct Xarc<T: Send + Sync> {
    pub(crate) ptr: AtomicPtr<XarcData<T>>,
}

impl<T: Send + Sync> Xarc<T> {
    /// Initialize the atomic smart pointer with `value`.
    pub fn new(value: T) -> Self {
        Xarc {
            ptr: AtomicPtr::new(Box::into_raw(Box::new(XarcData::new(value)))),
        }
    }

    /// Initialize the atomic smart pointer with null.
    pub fn null() -> Self {
        Xarc {
            ptr: AtomicPtr::new(ptr::null_mut()),
        }
    }

    /// As an atomic operation, if `self == current` swap the contents of `self` with `new`.
    /// Returns the previous value of `self`.
    /// If the value does not equal `current` the operation failed.
    pub fn compare_and_swap(&self, current: &XarcLocal<T>, new: &XarcLocal<T>, order: Ordering) -> XarcLocal<T> {
        unguarded_increment(new.ptr);
        let guard = pin();
        let ptr = self.ptr.compare_and_swap(current.ptr, new.ptr, order);
        if ptr == current.ptr {
            XarcLocal::init(ptr)
        }
        else {
            unguarded_decrement(new.ptr);
            self.increment_or_reload(ptr, &guard)
        }
    }

    /// As an atomic operation, if `self == current` swap the contents of `self` with `new`.
    /// Returns the previous value of `self` in a Result indicating whether the operation succeeded or failed.
    pub fn compare_exchange(&self, current: &XarcLocal<T>, new: &XarcLocal<T>, success: Ordering, failure: Ordering) -> Result<XarcLocal<T>, XarcLocal<T>> {
        unguarded_increment(new.ptr);
        let guard = pin();
        match self.ptr.compare_exchange(current.ptr, new.ptr, success, failure) {
            Ok(ptr) => {
                Ok(XarcLocal::init(ptr))
            },
            Err(ptr) => {
                unguarded_decrement(new.ptr);
                Err(self.increment_or_reload(ptr, &guard))
            },
        }
    }

    /// As an atomic operation, if `self == current` swap the contents of `self` with `new`.
    /// Returns the previous value of `self` in a Result indicating whether the operation succeeded or failed.
    /// Failure does not necessarily imply that `self != current`.
    /// This is typically called within a loop.
    pub fn compare_exchange_weak(&self, current: &XarcLocal<T>, new: &XarcLocal<T>, success: Ordering, failure: Ordering) -> Result<XarcLocal<T>, XarcLocal<T>> {
        unguarded_increment(new.ptr);
        let guard = pin();
        match self.ptr.compare_exchange_weak(current.ptr, new.ptr, success, failure) {
            Ok(ptr) => {
                Ok(XarcLocal::init(ptr))
            },
            Err(ptr) => {
                unguarded_decrement(new.ptr);
                Err(self.increment_or_reload(ptr, &guard))
            },
        }
    }

    /// Attempt to load the value into an `XarcLocal`.
    /// It can fail if, after the pointer has been loaded but before it is used, it is swapped out in another thread and destroyed.
    /// If success is required, use `XarcLocal::from` instead.
    pub fn try_load(&self, order: Ordering) -> Result<XarcLocal<T>, ()> {
        let guard = pin();
        XarcLocal::<T>::try_from(self.ptr.load(order), &guard)
    }

    fn increment_or_reload(&self, ptr: *mut XarcData<T>, guard: &Guard) -> XarcLocal<T> {
        if try_increment(ptr, guard).is_ok() {
            XarcLocal::init(ptr)
        }
        else {
            XarcLocal::from(self)
        }
    }
}

impl<T: Send + Sync> Default for Xarc<T> {
    fn default() -> Self {
        Xarc {
            ptr: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

impl<T: Send + Sync> Drop for Xarc<T> {
    fn drop(&mut self) {
        let ptr = self.ptr.load(Ordering::Relaxed);
        decrement(ptr, &pin());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, time::SystemTime};

    #[test]
    fn xarc_simple_st_test() {
        let shared = Xarc::new(42);
        let local = shared.try_load(Ordering::Acquire).unwrap();
        drop(shared);
        assert_eq!(*local.maybe_deref().unwrap(), 42);
    }

    #[test]
    fn xarc_st_performance_test() {
        let t0 = SystemTime::now();
        for _ in 1..500000 {
            let _ = Arc::new(42);
        }
        let t1 = SystemTime::now();
        for _ in 1..500000 {
            let _ = Xarc::new(42);
        }
        let t2 = SystemTime::now();
    
        println!("Arc Time: {} µs\r\nXarc Time: {} µs",
          t1.duration_since(t0).unwrap().as_micros(),
          t2.duration_since(t1).unwrap().as_micros());
    }

    use rayon::iter::*;

    #[test]
    fn xarc_mt_performance_test() {
        let shared = Xarc::new(42);

        let mut values: Vec<i64> = Vec::new();
        for i in 0..500000 {
            values.push(i);
        }

        let t0 = SystemTime::now();
        values.iter().for_each(|x| {
            let local = XarcLocal::new(*x);
            shared.compare_and_swap(&local, &local, Ordering::AcqRel);
        });
        let t1 = SystemTime::now();
        values.par_iter().for_each(|x| {
            let local = XarcLocal::new(*x);
            shared.compare_and_swap(&local, &local, Ordering::AcqRel);
        });
        let t2 = SystemTime::now();

        println!("Sequential Time: {} µs\r\nParallel Time: {} µs",
          t1.duration_since(t0).unwrap().as_micros(),
          t2.duration_since(t1).unwrap().as_micros());
    }
}
