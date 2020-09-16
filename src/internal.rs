use crossbeam_epoch::Guard;
use std::sync::atomic::{AtomicI64, Ordering};

pub(crate) struct XarcCount {
    count: AtomicI64,
}

impl XarcCount {
    fn new() -> XarcCount {
        XarcCount {
            count: AtomicI64::new(1),
        }
    }

    pub(crate) fn decrement(&self) -> i64 {
        self.count.fetch_sub(1, Ordering::Relaxed)
    }

    pub(crate) fn try_increment(&self) -> Result<i64, i64> {
        let mut count = self.count.load(Ordering::Relaxed);
        while count > 0 {
            match self.count.compare_exchange_weak(count, count + 1, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(c) => return Ok(c),
                Err(c) => count = c,
            }
        }
        Err(count)
    }

    fn unsafe_increment(&self) -> i64 {
        self.count.fetch_add(1, Ordering::Relaxed)
    }
}

pub(crate) struct XarcData<T: Send + Sync> {
    pub(crate) count: XarcCount,
    pub(crate) value: T,
}

impl<T: Send + Sync> XarcData<T> {
    pub(crate) fn new(value: T) -> Self {
        XarcData {
            count: XarcCount::new(),
            value,
        }
    }
}

pub(crate) fn decrement<T: Send + Sync>(ptr: *mut XarcData<T>, guard: &Guard) {
    unsafe {
        if !ptr.is_null() && (*ptr).count.decrement() == 1 {
            let boxed = Box::from_raw(ptr);
            guard.defer_unchecked(move || {
                drop(boxed);
            });
        }
    }
}

pub(crate) fn unguarded_decrement<T: Send + Sync>(ptr: *mut XarcData<T>) {
    unsafe {
        if !ptr.is_null() && (*ptr).count.decrement() == 1 {
            panic!("Unguarded XarcCount decrement to 0!")
        }
    }
}

pub(crate) fn try_increment<T: Send + Sync>(ptr: *mut XarcData<T>, _guard: &Guard) -> Result<(), ()> {
    unsafe {
        if ptr.is_null() || (*ptr).count.try_increment().is_ok() {
            Ok(())
        }
        else {
            Err(())
        }
    }
}

pub(crate) fn unguarded_increment<T: Send + Sync>(ptr: *mut XarcData<T>) {
    unsafe {
        if !ptr.is_null() && (*ptr).count.unsafe_increment() < 1 {
            panic!("Unguarded XarcCount increment from 0!");
        }
    }
}
