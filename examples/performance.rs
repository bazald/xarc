use crossbeam_epoch::pin;
use rayon::iter::*;
use std::{sync::{Arc, atomic::Ordering}, time::SystemTime};
use xarc::{AtomicXarc, Xarc};

fn main() {
    xarc_st_performance_test();
    xarc_mt_performance_test();
}

fn xarc_st_performance_test() {
    println!("xarc_st_performance_test:");

    let t0 = SystemTime::now();
    for _ in 1..500000 {
        let _ = Arc::new(42);
    }
    let t1 = SystemTime::now();
    for _ in 1..500000 {
        let _ = AtomicXarc::new(42);
    }
    let t2 = SystemTime::now();

    println!("Arc Time: {} µs\r\nXarc Time: {} µs",
      t1.duration_since(t0).unwrap().as_micros(),
      t2.duration_since(t1).unwrap().as_micros());
}

fn xarc_mt_performance_test() {
    println!("xarc_mt_performance_test:");

    let shared = AtomicXarc::new(42);

    let mut values: Vec<i64> = Vec::new();
    for i in 0..500000 {
        values.push(i);
    }

    let t0 = SystemTime::now();
    values.iter().for_each(|x| {
        let _guard = pin();
        let mut current = shared.load(Ordering::Acquire);
        let new = Xarc::new(*x);
        loop {
            match shared.compare_exchange_weak(&current, &new, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => break,
                Err(previous) => current = previous,
            }
        }
    });
    let t1 = SystemTime::now();
    values.par_iter().for_each(|x| {
        let _guard = pin();
        let mut current = shared.load(Ordering::Acquire);
        let new = Xarc::new(*x);
        loop {
            match shared.compare_exchange_weak(&current, &new, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => break,
                Err(previous) => current = previous,
            }
        }
    });
    let t2 = SystemTime::now();

    println!("Sequential Time: {} µs\r\nParallel Time: {} µs",
      t1.duration_since(t0).unwrap().as_micros(),
      t2.duration_since(t1).unwrap().as_micros());
}
