# xarc
 `xarc` provides atomically swappable atomically refcounted smart pointers
as a safer building block for lockfree algorithms than raw atomic pointers.

`Xarc` is comparable to `Arc` but with the additional ability to atomically
be swapped into and out of `AtomicXarc`.
`Xarc` is dereferenceable but cannot have its contents atomically swapped.
`AtomicXarc` can have its contents atomically swapped but is not dereferenceable.

Here's a fairly minimal example.
```
use core::sync::atomic::Ordering;
use xarc::{AtomicXarc, Xarc};

let atomic = AtomicXarc::new(42);

let current = atomic.load(Ordering::Acquire);
let loaded = atomic.compare_exchange(&current, &Xarc::null(), Ordering::AcqRel, Ordering::Acquire).unwrap();
assert_eq!(*loaded.maybe_deref().unwrap(), 42);
```
