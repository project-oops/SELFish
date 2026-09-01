//! The generation split and ABI constants.
//!
//! The bottom of the spine: everything else here depends on this and it depends on nothing.
//! What lives here is the small set of facts that decide how every other format is read or
//! written - chiefly *which console* a file is for, which is four bytes of difference and
//! the whole of the difference at this layer.

#![forbid(unsafe_code)]

pub mod generation;

pub use generation::Generation;
