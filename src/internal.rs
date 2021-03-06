use alloc::boxed::Box;
use core::sync::atomic::{AtomicUsize, Ordering};
use crossbeam_epoch::Guard;
use crossbeam_utils::CachePadded;

pub(crate) struct XarcCount {
    count: CachePadded<AtomicUsize>,
}

impl XarcCount {
    #[must_use]
    fn new() -> XarcCount {
        XarcCount {
            count: CachePadded::new(AtomicUsize::new(1)),
        }
    }

    #[must_use]
    pub(crate) fn decrement(&self) -> usize {
        self.count.fetch_sub(1, Ordering::Relaxed)
    }

    pub(crate) fn try_increment(&self) -> Result<usize, usize> {
        let mut count = self.count.load(Ordering::Relaxed);
        while count > 0 {
            match self.count.compare_exchange_weak(count, count + 1, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(c) => return Ok(c),
                Err(c) => count = c,
            }
        }
        Err(count)
    }

    #[must_use]
    fn unsafe_increment(&self) -> usize {
        self.count.fetch_add(1, Ordering::Relaxed)
    }
}

pub(crate) struct XarcData<T: Send> {
    pub(crate) count: XarcCount,
    pub(crate) value: T,
}

impl<T: Send> XarcData<T> {
    #[must_use]
    pub(crate) fn new(value: T) -> Self {
        XarcData {
            count: XarcCount::new(),
            value,
        }
    }
}

pub(crate) fn decrement<T: Send>(ptr: *mut XarcData<T>, guard: &Guard) {
    unsafe {
        if !ptr.is_null() && (*ptr).count.decrement() == 1 {
            let boxed = Box::from_raw(ptr);
            guard.defer_unchecked(move || {
                drop(boxed);
            });
        }
    }
}

pub(crate) fn try_increment<T: Send>(ptr: *mut XarcData<T>, _guard: &Guard) -> Result<(), ()> {
    unsafe {
        if ptr.is_null() || (*ptr).count.try_increment().is_ok() {
            Ok(())
        }
        else {
            Err(())
        }
    }
}

pub(crate) fn unguarded_increment<T: Send>(ptr: *mut XarcData<T>) {
    unsafe {
        if !ptr.is_null() && (*ptr).count.unsafe_increment() < 1 {
            panic!("Unguarded XarcCount increment from 0!");
        }
    }
}
