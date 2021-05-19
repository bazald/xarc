use crossbeam_epoch::pin;
use crossbeam_utils::Backoff;
use rayon::iter::*;
use std::{cell::UnsafeCell, mem, sync::atomic::Ordering, time::SystemTime};
use xarc::{AtomicXarc, Xarc};

#[cfg(not(target_os = "windows"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

struct Node<T: Send> {
    value: UnsafeCell<Option<T>>,
    next: Xarc<Node<T>>,
}

impl<T: Send> Node<T> {
    fn new(value: T, next: Xarc<Node<T>>) -> Self {
        Self {
            value: UnsafeCell::new(Some(value)),
            next,
        }
    }

    fn replace_next(&mut self, next: Xarc<Node<T>>) {
        self.next = next;
    }
}

struct Stack<T: Send> {
    node: AtomicXarc<Node<T>>,
}

impl<T: Send> Stack<T> {
    pub fn new() -> Self {
        Self {
            node: AtomicXarc::null(),
        }
    }

    pub fn push(&self, value: T) {
        let _guard = pin();
        let backoff = Backoff::new();
        let mut new = Xarc::new(Node::new(value, self.node.load(Ordering::Relaxed)));
        loop {
            match self.node.compare_exchange_weak(&new.maybe_deref().unwrap().next, &new, Ordering::Release, Ordering::Relaxed) {
                Ok(_previous) => return,
                Err(current) => {
                    unsafe {
                        new.unguarded_maybe_deref_mut().unwrap().replace_next(current);
                    }
                    backoff.spin();
                },
            }
        }
    }

    #[must_use]
    pub fn try_pop(&self) -> Option<T> {
        let _guard = pin();
        let backoff = Backoff::new();
        let mut current = self.node.load(Ordering::Relaxed);
        loop {
            if current.is_null() {
                return None
            }
            match self.node.compare_exchange_weak(&current, &current.maybe_deref().unwrap().next, Ordering::Acquire, Ordering::Relaxed) {
                Ok(_) => break,
                Err(pointer) => {
                    current = pointer;
                    backoff.spin();
                },
            }
        }
        unsafe {
            mem::take(&mut *current.maybe_deref().unwrap().value.get())
        }
    }

    pub fn is_empty(&self) -> bool {
        let _guard = pin();
        self.node.load(Ordering::Relaxed).is_null()
    }
}

fn main() {
    let block_size = 512;
    let num_blocks = 512;
    let mut ranges: Vec<(i64, i64)> = Vec::new();
    for i in 0..num_blocks {
        ranges.push((i * block_size, (i + 1) * block_size));
    }

    let stack = Stack::new();

    let t0 = SystemTime::now();
    ranges.par_iter().for_each(|(begin, end)| {
        for i in *begin..*end {
            stack.push(i);
        }
    });
    let t1 = SystemTime::now();
    ranges.par_iter().for_each(|(begin, end)| {
        for _ in *begin..*end {
            let _ = stack.try_pop().unwrap();
        }
    });
    let t2 = SystemTime::now();

    assert_eq!(stack.is_empty(), true);

    println!("Push Time: {} µs\r\nPop Time: {} µs",
      t1.duration_since(t0).unwrap().as_micros(),
      t2.duration_since(t1).unwrap().as_micros());
}
