extern crate xarc;
use rayon::iter::*;
use std::{sync::atomic::Ordering, time::SystemTime};
use xarc::{XarcAtomic, Xarc};

struct Node<T: Send> {
    value: T,
    next: Xarc<Node<T>>,
}

impl<T: Send> Node<T> {
    fn new(value: T, next: Xarc<Node<T>>) -> Self {
        Self {
            value,
            next,
        }
    }

    fn replace_next(&mut self, next: Xarc<Node<T>>) {
        self.next = next;
    }
}

struct Stack<T: Send> {
    node: XarcAtomic<Node<T>>,
}

impl<T: Send> Stack<T> {
    fn new() -> Self {
        Self {
            node: XarcAtomic::null(),
        }
    }

    fn push(&self, value: T) {
        let mut new = Xarc::new(Node::new(value, Xarc::<Node<T>>::from(&self.node)));
        loop {
            match self.node.compare_exchange_weak(&new.maybe_deref().unwrap().next, &new, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_previous) => return,
                Err(current) => unsafe {
                    new.unguarded_maybe_deref_mut().unwrap().replace_next(current)
                },
            }
        }
    }

    #[must_use]
    fn try_pop(&self) -> Option<T> {
        let mut current = self.node.load(Ordering::Acquire);
        loop {
            if current.is_null() {
                return None
            }
            match self.node.compare_exchange_weak(&current, &current.maybe_deref().unwrap().next, Ordering::Acquire, Ordering::Relaxed) {
                Ok(_) => break,
                Err(pointer) => current = pointer,
            }
        }
        match current.try_take() {
            Ok(node) => Some(node.value),
            Err(_) => None,
        }
    }

    fn is_empty(&self) -> bool {
        self.node.load(Ordering::Acquire).is_null()
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
            let _ = stack.try_pop();
        }
    });
    let t2 = SystemTime::now();

    assert_eq!(stack.is_empty(), true);

    println!("Sequential Time: {} µs\r\nParallel Time: {} µs",
      t1.duration_since(t0).unwrap().as_micros(),
      t2.duration_since(t1).unwrap().as_micros());
}
