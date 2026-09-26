//! The generation split and ABI constants.
//!
//! The bottom of the spine: every other crate here depends on this one and it depends on
//! nothing. It holds the facts that decide how every other format is read or written, chiefly
//! which hardware generation a file is for.

#![forbid(unsafe_code)]

pub mod generation;

pub use generation::Generation;
