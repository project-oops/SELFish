//! What a title says about itself.
//!
//! Two formats, one per hardware generation, both read and written: `PARAM.SFO` is a binary
//! key-value table inside every package; `param.json` is what Prospero-generation titles carry
//! in their directory. They are one crate because they answer the same questions (name, id,
//! category) and a caller wants whichever is present.
//!
//! Which keys a title carries, and which are required, varies by generation and category and
//! is left to the consumer; [`Sfo`] reads and writes any key.

#![forbid(unsafe_code)]

pub mod param;
pub mod sfo;
mod table;

pub use param::{Param, category};
pub use sfo::Sfo;
