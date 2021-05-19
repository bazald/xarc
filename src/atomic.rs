use super::{internal::*, pointer::*};
use alloc::boxed::Box;
use core::{ptr, sync::atomic::{AtomicPtr, Ordering}};
use crossbeam_epoch::pin;
use crossbeam_utils::{Backoff, CachePadded};

/// `AtomicXarc` provides atomic storage for `Xarc` atomically refcounted smart pointers.
/// 
/// # Examples
/// 
/// Here is some typical usage of `AtomicXarc`.
/// ```
/// use core::sync::atomic::Ordering;
/// use xarc::{AtomicXarc, Xarc};
/// 
/// let atomic = AtomicXarc::new(42);
/// let same = atomic.load(Ordering::Acquire);
/// let different = Xarc::new(42);
/// 
/// assert_eq!(*atomic.load(Ordering::Acquire).maybe_deref().unwrap(), 42);
/// assert!(atomic.compare_exchange(&different, &Xarc::null(), Ordering::AcqRel, Ordering::Acquire)
///         .is_err());
/// assert_eq!(*atomic.compare_exchange(&same, &Xarc::null(), Ordering::AcqRel, Ordering::Acquire)
///             .unwrap().maybe_deref().unwrap(), 42);
/// ```
#[derive(Debug)]
pub struct AtomicXarc<T: Send> {
    pub(crate) ptr: CachePadded<AtomicPtr<XarcData<T>>>,
}

impl<T: Send> AtomicXarc<T> {
    /// Initialize the atomic smart pointer with `value`.
    #[must_use]
    pub fn new(value: T) -> Self {
        AtomicXarc {
            ptr: CachePadded::new(AtomicPtr::new(Box::into_raw(Box::new(XarcData::new(value))))),
        }
    }

    /// Initialize the atomic smart pointer with null.
    #[must_use]
    pub fn null() -> Self {
        AtomicXarc {
            ptr: CachePadded::new(AtomicPtr::new(ptr::null_mut())),
        }
    }

    #[must_use]
    pub(crate) fn init(ptr: *mut XarcData<T>) -> Self {
        AtomicXarc {
            ptr: CachePadded::new(AtomicPtr::new(ptr)),
        }
    }

    /// As an atomic operation, swap the contents of `self` with `new` if `self == current`.
    /// Returns the previous value of `self`.
    /// If the value does not equal `current` the operation failed.
    #[must_use]
    pub fn compare_and_swap(&self, current: &Xarc<T>, new: &Xarc<T>, success: Ordering, failure: Ordering) -> Xarc<T> {
        match self.compare_exchange(current, new, success, failure) {
            Ok(ptr) => ptr,
            Err(ptr) => ptr,
        }
    }

    /// As an atomic operation, swap the contents of `self` with `new` if `self == current`.
    /// Returns the previous value of `self` in a Result indicating whether the operation succeeded or failed.
    pub fn compare_exchange(&self, current: &Xarc<T>, new: &Xarc<T>, success: Ordering, failure: Ordering) -> Result<Xarc<T>, Xarc<T>> {
        let guard = pin();
        unguarded_increment(new.ptr);
        match self.ptr.compare_exchange(current.ptr, new.ptr, success, failure) {
            Ok(ptr) => {
                Ok(Xarc::init(ptr))
            },
            Err(ptr) => {
                decrement(new.ptr, &guard);
                Err(self.increment_or_reload(ptr, failure))
            },
        }
    }

    /// As an atomic operation, swap the contents of `self` with `new` if `self == current` but with spurious failure of the comparison allowed.
    /// Returns the previous value of `self` in a Result indicating whether the operation succeeded or failed.
    /// Allowing spurious failure is a performance optimization that is reasonable when no additional loops are required for correctness.
    pub fn compare_exchange_weak(&self, current: &Xarc<T>, new: &Xarc<T>, success: Ordering, failure: Ordering) -> Result<Xarc<T>, Xarc<T>> {
        let guard = pin();
        unguarded_increment(new.ptr);
        match self.ptr.compare_exchange_weak(current.ptr, new.ptr, success, failure) {
            Ok(ptr) => {
                Ok(Xarc::init(ptr))
            },
            Err(ptr) => {
                decrement(new.ptr, &guard);
                Err(self.increment_or_reload(ptr, failure))
            },
        }
    }

    /// Load the value into an `Xarc`.
    /// The internal atomic operation is repeated as needed until successful.
    #[must_use]
    pub fn load(&self, order: Ordering) -> Xarc<T> {
        let guard = pin();
        let backoff = Backoff::new();
        loop {
            if let Ok(pointer) = Xarc::try_from(self.ptr.load(order), &guard) {
                return pointer;
            }
            else {
                backoff.spin();
            }
        }
    }

    /// Attempt to load the value into an `Xarc`.
    /// It can fail if, after the pointer has been loaded but before it is used, it is swapped out in another thread and destroyed.
    #[allow(clippy::result_unit_err)]
    pub fn try_load(&self, order: Ordering) -> Result<Xarc<T>, ()> {
        let guard = pin();
        Xarc::try_from(self.ptr.load(order), &guard)
    }

    /// As an atomic operation, swap the contents of `self` with `new`.
    /// Returns the previous value of `self`.
    #[must_use]
    pub fn swap(&self, new: &Xarc<T>, order: Ordering) -> Xarc<T> {
        unguarded_increment(new.ptr);
        Xarc::init(self.ptr.swap(new.ptr, order))
    }

    #[must_use]
    fn increment_or_reload(&self, ptr: *mut XarcData<T>, order: Ordering) -> Xarc<T> {
        let guard = pin();
        if try_increment(ptr, &guard).is_ok() {
            Xarc::init(ptr)
        }
        else {
            self.load(order)
        }
    }
}

impl<T: Send> Drop for AtomicXarc<T> {
    fn drop(&mut self) {
        let ptr = self.ptr.load(Ordering::Relaxed);
        decrement(ptr, &pin());
    }
}

impl<T: Send> From<&Xarc<T>> for AtomicXarc<T> {
    #[must_use]
    fn from(pointer: &Xarc<T>) -> Self {
        unguarded_increment(pointer.ptr);
        AtomicXarc::init(pointer.ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xarc_simple_st_test() {
        let shared = AtomicXarc::new(42);
        let local = shared.try_load(Ordering::Acquire).unwrap();
        drop(shared);
        assert_eq!(*local.maybe_deref().unwrap(), 42);
    }

}
