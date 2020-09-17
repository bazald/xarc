//! # xarc
//! 
//! `xarc` implements `Xarc` and `XarcLocal` where
//! `Xarc` is an atomic smart pointer and
//! `XarcLocal` is the corresponding thread-local dereferenceable pointer.

mod internal;
mod atomic;
mod pointer;

pub use atomic::XarcAtomic;
pub use pointer::Xarc;
