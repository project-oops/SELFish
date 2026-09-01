//! What a title says about itself.
//!
//! Two formats for one job, one per console generation. `PARAM.SFO` is a binary key-value
//! table that appears inside every package; `param.json` is what the current generation
//! writes into a title directory. Both are read and both are written.
//!
//! # Why they are one crate
//!
//! They answer the same questions - what is this title called, what is its id, what category
//! is it - and anything that builds or inspects a title wants whichever one is present rather
//! than a choice of dependency. Splitting them would mean a caller depending on both and
//! writing the "try one, then the other" itself, three times over.
//!
//! # What is not here
//!
//! **The keys.** Which keys a title carries, and which are required, is convention that
//! varies by generation, category, and the submission tooling of the day. A format crate that
//! asserted a required-key list would report the first title that omits one as malformed.
//! `sfo::Sfo` will read and write any key; naming the ones a particular consumer needs is
//! that consumer's business.

#![forbid(unsafe_code)]

pub mod param;
pub mod sfo;
mod table;

pub use param::Param;
pub use sfo::Sfo;
