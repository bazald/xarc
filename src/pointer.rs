use super::{internal::*, atomic::*};
use crossbeam_epoch::{Guard, pin};
use std::{hash::*, mem, ptr, sync::atomic::Ordering};

/// `XarcLocal` is a thread-local smart pointer.
#[derive(Debug, Eq)]
pub struct Xarc<T: Default + Send + Sync> {
    pub(crate) ptr: *mut XarcData<T>,
}

impl<T: Default + Send + Sync> Xarc<T> {
    /// Initialize the smart pointer with `value`.
    #[must_use]
    pub fn new(value: T) -> Self {
        Xarc {
            ptr: Box::into_raw(Box::new(XarcData::new(value))),
        }
    }

    /// Initialize the smart pointer with null.
    #[must_use]
    pub fn null() -> Self {
        Xarc {
            ptr: ptr::null_mut(),
        }
    }

    #[must_use]
    pub(crate) fn init(ptr: *mut XarcData<T>) -> Self {
        Xarc {
            ptr,
        }
    }

    pub(crate) fn try_from(ptr: *mut XarcData<T>, guard: &Guard) -> Result<Self, ()> {
        try_increment(ptr, guard)?;
        Ok(Xarc::init(ptr))
    }

    /// Reset the smart pointer to null.
    pub fn reset(&mut self) {
        decrement(self.ptr, &pin());
        self.ptr = ptr::null_mut();
    }

    /// Check if the smart pointer is null.
    #[must_use]
    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    /// Dereference the pointer only if it is not null.
    /// None will be returned if it is null.
    #[must_use]
    pub fn maybe_deref(&self) -> Option<&T> {
        if !self.ptr.is_null() {
            unsafe {
                Some(&(*self.ptr).value)
            }
        }
        else {
            None
        }
    }

    /// Dereference the pointer only if it is not null.
    /// None will be returned if it is null.
    /// # Safety
    /// - This should be called only if you're absolutely, 100% certain that nobody else could possibly have access to this data.
    #[must_use]
    pub unsafe fn unguarded_maybe_deref_mut(&mut self) -> Option<&mut T> {
        if !self.ptr.is_null() {
            Some(&mut (*self.ptr).value)
        }
        else {
            None
        }
    }

    /// If the pointer is non-null and the last pointing to its data, extract the raw data. Otherwise return itself.
    pub fn try_take(self) -> Result<T, Self> {
        if !self.ptr.is_null() {
            unsafe {
                let data = &mut *self.ptr;
                if data.count.load() == 1 {
                    let value = mem::take(&mut data.value);
                    Ok(value)
                }
                else {
                    Err(self)
                }
            }
        }
        else {
            Err(self)
        }
    }
}

impl<T: Default + Send + Sync> Default for Xarc<T> {
    #[must_use]
    fn default() -> Self {
        Self::init(ptr::null_mut())
    }
}

impl<T: Default + Send + Sync> Drop for Xarc<T> {
    fn drop(&mut self) {
        decrement(self.ptr, &pin());
    }
}

impl<T: Default + Send + Sync> From<&XarcAtomic<T>> for Xarc<T> {
    #[must_use]
    fn from(shared: &XarcAtomic<T>) -> Self {
        shared.load(Ordering::Acquire)
    }
}

impl<T: Default + Send + Sync> Hash for Xarc<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        ptr::hash(self.ptr, state);
    }
}

impl<T: Default + Send + Sync> PartialEq for Xarc<T> {
    #[must_use]
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr
    }
}

unsafe impl<T: Default + Send + Sync> Send for Xarc<T> {}
unsafe impl<T: Default + Send + Sync> Sync for Xarc<T> {}
