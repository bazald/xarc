//! # xarc
//! 
//! `xarc` implements `Xarc` and `XarcLocal` where
//! `Xarc` is an atomic smart pointer and
//! `XarcLocal` is the corresponding thread-local dereferenceable pointer.

mod internal;
mod shared;
mod local;

pub use shared::Xarc;
pub use local::XarcLocal;
