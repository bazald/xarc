use super::{internal::*, pointer::*};
use crossbeam_epoch::{Guard, pin};
use std::{ptr, sync::atomic::{AtomicPtr, Ordering}};

/// `Xarc` is an atomic smart pointer.
#[derive(Debug)]
pub struct XarcAtomic<T: Send> {
    pub(crate) ptr: AtomicPtr<XarcData<T>>,
}

impl<T: Send> XarcAtomic<T> {
    /// Initialize the atomic smart pointer with `value`.
    #[must_use]
    pub fn new(value: T) -> Self {
        XarcAtomic {
            ptr: AtomicPtr::new(Box::into_raw(Box::new(XarcData::new(value)))),
        }
    }

    /// Initialize the atomic smart pointer with null.
    #[must_use]
    pub fn null() -> Self {
        XarcAtomic {
            ptr: AtomicPtr::new(ptr::null_mut()),
        }
    }

    #[must_use]
    pub(crate) fn init(ptr: *mut XarcData<T>) -> Self {
        XarcAtomic {
            ptr: AtomicPtr::new(ptr),
        }
    }

    /// As an atomic operation, if `self == current` swap the contents of `self` with `new`.
    /// Returns the previous value of `self`.
    /// If the value does not equal `current` the operation failed.
    #[must_use]
    pub fn compare_and_swap(&self, current: &Xarc<T>, new: &Xarc<T>, order: Ordering) -> Xarc<T> {
        unguarded_increment(new.ptr);
        let guard = pin();
        let ptr = self.ptr.compare_and_swap(current.ptr, new.ptr, order);
        if ptr == current.ptr {
            Xarc::init(ptr)
        }
        else {
            unguarded_decrement(new.ptr);
            self.increment_or_reload(ptr, &guard)
        }
    }

    /// As an atomic operation, if `self == current` swap the contents of `self` with `new`.
    /// Returns the previous value of `self` in a Result indicating whether the operation succeeded or failed.
    pub fn compare_exchange(&self, current: &Xarc<T>, new: &Xarc<T>, success: Ordering, failure: Ordering) -> Result<Xarc<T>, Xarc<T>> {
        unguarded_increment(new.ptr);
        let guard = pin();
        match self.ptr.compare_exchange(current.ptr, new.ptr, success, failure) {
            Ok(ptr) => {
                Ok(Xarc::init(ptr))
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
    pub fn compare_exchange_weak(&self, current: &Xarc<T>, new: &Xarc<T>, success: Ordering, failure: Ordering) -> Result<Xarc<T>, Xarc<T>> {
        unguarded_increment(new.ptr);
        let guard = pin();
        match self.ptr.compare_exchange_weak(current.ptr, new.ptr, success, failure) {
            Ok(ptr) => {
                Ok(Xarc::init(ptr))
            },
            Err(ptr) => {
                unguarded_decrement(new.ptr);
                Err(self.increment_or_reload(ptr, &guard))
            },
        }
    }

    /// Load the value into an `XarcLocal`.
    /// The internal atomic operation is repeated as needed until successful.
    #[must_use]
    pub fn load(&self, order: Ordering) -> Xarc<T> {
        let guard = pin();
        loop {
            if let Ok(pointer) = Xarc::<T>::try_from(self.ptr.load(order), &guard) {
                return pointer;
            }
        }
    }

    /// Attempt to load the value into an `XarcLocal`.
    /// It can fail if, after the pointer has been loaded but before it is used, it is swapped out in another thread and destroyed.
    pub fn try_load(&self, order: Ordering) -> Result<Xarc<T>, ()> {
        let guard = pin();
        Xarc::<T>::try_from(self.ptr.load(order), &guard)
    }

    #[must_use]
    fn increment_or_reload(&self, ptr: *mut XarcData<T>, guard: &Guard) -> Xarc<T> {
        if try_increment(ptr, guard).is_ok() {
            Xarc::init(ptr)
        }
        else {
            Xarc::from(self)
        }
    }
}

impl<T: Send> Drop for XarcAtomic<T> {
    fn drop(&mut self) {
        let ptr = self.ptr.load(Ordering::Relaxed);
        decrement(ptr, &pin());
    }
}

impl<T: Send> From<&Xarc<T>> for XarcAtomic<T> {
    #[must_use]
    fn from(pointer: &Xarc<T>) -> Self {
        unguarded_increment(pointer.ptr);
        XarcAtomic::init(pointer.ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xarc_simple_st_test() {
        let shared = XarcAtomic::new(42);
        let local = shared.try_load(Ordering::Acquire).unwrap();
        drop(shared);
        assert_eq!(*local.maybe_deref().unwrap(), 42);
    }

}
