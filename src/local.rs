use super::{internal::*, shared::*};
use crossbeam_epoch::{Guard, pin};
use std::{hash::*, ptr, sync::atomic::Ordering};

/// `XarcLocal` is a thread-local smart pointer.
#[derive(Debug, Eq)]
pub struct XarcLocal<T: Send + Sync> {
    pub(crate) ptr: *mut XarcData<T>,
}

impl<T: Send + Sync> XarcLocal<T> {
    /// Initialize the smart pointer with `value`.
    pub fn new(value: T) -> Self {
        XarcLocal {
            ptr: Box::into_raw(Box::new(XarcData::new(value))),
        }
    }

    /// Initialize the smart pointer with null.
    pub fn null() -> Self {
        XarcLocal {
            ptr: ptr::null_mut(),
        }
    }

    pub(crate) fn init(ptr: *mut XarcData<T>) -> Self {
        XarcLocal {
            ptr,
        }
    }

    pub(crate) fn try_from(ptr: *mut XarcData<T>, guard: &Guard) -> Result<Self, ()> {
        try_increment(ptr, guard)?;
        Ok(XarcLocal::init(ptr))
    }

    /// Reset the smart pointer to null.
    pub fn reset(&mut self) {
        decrement(self.ptr, &pin());
        self.ptr = ptr::null_mut();
    }

    /// Check if the smart pointer is null.
    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    /// Dereference the pointer only if it is not null.
    /// None will be returned if it is null.
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
}

impl<T: Send + Sync> Default for XarcLocal<T> {
    fn default() -> Self {
        Self::init(ptr::null_mut())
    }
}

impl<T: Send + Sync> From<&Xarc<T>> for XarcLocal<T> {
    fn from(shared: &Xarc<T>) -> Self {
        loop {
            if let Ok(local) = shared.try_load(Ordering::Acquire) {
                return local;
            }
        }
    }
}

impl<T: Send + Sync> Hash for XarcLocal<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        ptr::hash(self.ptr, state);
    }
}

impl<T: Send + Sync> PartialEq for XarcLocal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr
    }
}
