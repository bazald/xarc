use super::{internal::*};
use alloc::boxed::Box;
use core::{hash::*, ptr};
use crossbeam_epoch::{Guard, pin};

/// `Xarc` is a derefenceable atomically refcounted smart pointer.
/// `Xarc` is roughly equivalent to `Arc` but is compatible with `AtomicXarc`.
/// 
/// # Examples
/// 
/// Here is some typical usage of `Xarc`.
/// ```
/// use xarc::Xarc;
/// 
/// let xarc = Xarc::new(42);
/// let same = xarc.clone();
/// let different = Xarc::new(42);
/// 
/// // Null checks
/// assert!(!xarc.is_null());
/// assert_ne!(xarc, Xarc::null());
/// 
/// // Pointer comparisons
/// assert_eq!(xarc, same);
/// assert_ne!(xarc, different);
/// 
/// // Value comparisons
/// assert_eq!(xarc.maybe_deref().unwrap(), different.maybe_deref().unwrap());
/// assert_eq!(Xarc::<i64>::null().maybe_deref(), None);
/// ```
/// 
/// When implementing a container you often need structures with an immutable part,
/// such as a pointer to another part of the structure, and a separate value that
/// you can `take` to return a value as you remove it. `UnsafeCell` comes to the rescue.
/// ```
/// use core::{cell::UnsafeCell, mem};
/// use xarc::Xarc;
/// 
/// struct Example {
///     immutable: i64,
///     takeable: UnsafeCell<i64>,
/// }
/// 
/// let mut xarc = Xarc::new(Example {immutable: 0, takeable: UnsafeCell::new(42)});
/// 
/// let value = unsafe {
///     // You had better know what you're doing at this point. 🙂
///     core::mem::take(&mut *xarc.maybe_deref().unwrap().takeable.get())
/// };
/// 
/// assert_eq!(value, 42);
/// ```

#[derive(Debug, Eq)]
pub struct Xarc<T: Send> {
    pub(crate) ptr: *mut XarcData<T>,
}

impl<T: Send> Xarc<T> {
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
        let guard = pin();
        decrement(self.ptr, &guard);
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
    /// 
    /// # Safety
    /// - This should be called only if you're absolutely,
    /// 100% certain that nobody else could possibly have access to this data
    /// or if you *really* know what you're doing.
    #[must_use]
    pub unsafe fn unguarded_maybe_deref_mut(&mut self) -> Option<&mut T> {
        if !self.ptr.is_null() {
            Some(&mut (*self.ptr).value)
        }
        else {
            None
        }
    }
}

impl<T: Send> Clone for Xarc<T> {
    fn clone(&self) -> Self {
        unguarded_increment(self.ptr);
        Xarc::init(self.ptr)
    }
}

impl<T: Send> Drop for Xarc<T> {
    fn drop(&mut self) {
        decrement(self.ptr, &pin());
    }
}

impl<T: Send> Hash for Xarc<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        ptr::hash(self.ptr, state);
    }
}

impl<T: Send> PartialEq for Xarc<T> {
    #[must_use]
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr
    }
}

unsafe impl<T: Send> Send for Xarc<T> {}
unsafe impl<T: Send> Sync for Xarc<T> {}
