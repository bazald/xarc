use rayon::iter::*;
use std::{cell::UnsafeCell, mem, sync::atomic::Ordering, time::SystemTime};
use xarc::{XarcAtomic, Xarc};

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

struct Node<T: Send> {
    value: XarcAtomic<UnsafeCell<Option<T>>>,
    next: XarcAtomic<Node<T>>,
}

impl<T: Send> Node<T> {
    fn null() -> Self {
        Self {
            value: XarcAtomic::null(),
            next: XarcAtomic::null(),
        }
    }
}

struct Queue<T: Send> {
    head: XarcAtomic<Node<T>>,
    tail: XarcAtomic<Node<T>>,
}

impl<T: Send> Queue<T> {
    pub fn new() -> Self {
        let node = Xarc::new(Node::null());
        Self {
            head: XarcAtomic::from(&node),
            tail: XarcAtomic::from(&node),
        }
    }

    pub fn push(&self, value: T) {
        let value = Xarc::new(UnsafeCell::new(Some(value)));
        let mut new_tail = Xarc::new(Node::null());
        let mut current_tail = self.tail.load(Ordering::Relaxed);
        loop {
            match current_tail.maybe_deref().unwrap().value.compare_exchange(&Xarc::null(), &value, Ordering::Release, Ordering::Relaxed) {
                Ok(_) => {
                    let _ = self.try_update_tail(&current_tail, &new_tail);
                    break;
                },
                Err(_) => match self.try_update_tail(&current_tail, &new_tail) {
                    Ok(current_tail_next) => {
                        current_tail = current_tail_next;
                        new_tail = Xarc::new(Node::null());
                    },
                    Err(current_tail_next) => current_tail = current_tail_next,
                },
            }
        }
    }

    #[must_use]
    pub fn try_pop(&self) -> Option<T> {
        let mut current_head = self.head.load(Ordering::Relaxed);
        loop {
            let current_head_deref = current_head.maybe_deref().unwrap();
            let value = current_head_deref.value.load(Ordering::Relaxed);
            if value.is_null() {
                return None;
            }
            let mut next = current_head_deref.next.load(Ordering::Relaxed);
            if next.is_null() {
                next = Xarc::new(Node::null());
                if self.try_update_tail(&current_head, &next).is_err() {
                    current_head = self.head.load(Ordering::Relaxed);
                    continue;
                }
            }
            match self.head.compare_exchange(&current_head, &next, Ordering::Release, Ordering::Relaxed) {
                Ok(_) => unsafe {
                    return mem::take(&mut *current_head.maybe_deref().unwrap().value.load(Ordering::Acquire).maybe_deref().unwrap().get())
                },
                Err(head) => current_head = head,
            };
        }
    }

    fn try_update_tail(&self, current_tail: &Xarc<Node<T>>, new_tail: &Xarc<Node<T>>) -> Result<Xarc<Node<T>>, Xarc<Node<T>>> {
        current_tail.maybe_deref().unwrap().next.compare_exchange(&Xarc::null(), new_tail, Ordering::Relaxed, Ordering::Relaxed)
            .map(|_| self.tail.compare_and_swap(current_tail, new_tail, Ordering::Relaxed))
            .map_err(|current_tail_next| self.tail.compare_and_swap(current_tail, &current_tail_next, Ordering::Relaxed))
    }

    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Relaxed) == self.tail.load(Ordering::Relaxed)
    }
}

fn main() {
    let block_size = 512;
    let num_blocks = 512;
    let mut ranges: Vec<(i64, i64)> = Vec::new();
    for i in 0..num_blocks {
        ranges.push((i * block_size, (i + 1) * block_size));
    }

    let queue = Queue::new();

    let t0 = SystemTime::now();
    ranges.par_iter().for_each(|(begin, end)| {
        for i in *begin..*end {
            queue.push(i);
        }
    });
    let t1 = SystemTime::now();
    ranges.par_iter().for_each(|(begin, end)| {
        for _ in *begin..*end {
            let _ = queue.try_pop().unwrap();
        }
    });
    let t2 = SystemTime::now();

    assert_eq!(queue.is_empty(), true);

    println!("Push Time: {} µs\r\nPop Time: {} µs",
      t1.duration_since(t0).unwrap().as_micros(),
      t2.duration_since(t1).unwrap().as_micros());
}
