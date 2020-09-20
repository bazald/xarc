//! # xarc
//! 
//! `xarc` provides atomically swappable atomically refcounted smart pointers
//! as a safer building block for lockfree algorithms than raw atomic pointers.
//! 
//! `Xarc` is comparable to `Arc` but with the additional ability to atomically
//! be swapped into and out of `AtomicXarc`.
//! `Xarc` is dereferenceable but cannot have its contents atomically swapped.
//! `AtomicXarc` can have its contents atomically swapped but is not dereferenceable.

#![crate_name = "xarc"]

#![no_std]
extern crate alloc;

mod internal;
mod atomic;
mod pointer;

pub use atomic::AtomicXarc;
pub use pointer::Xarc;
