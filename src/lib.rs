//! `dv`: where did this command come from?
//!
//! The library is split along the pipeline a lookup follows:
//!
//! 1. [`search`] walks PATH and records what happened at every entry.
//! 2. [`chain`] follows each match's symlinks down to the real file.
//! 3. [`provenance`] asks each detector which package owns that file.
//! 4. [`lookup`] ties the three together into a [`lookup::NameResult`].
//! 5. [`render`] prints results as text or JSON.

#![forbid(unsafe_code)]

pub mod chain;
pub mod lookup;
pub mod provenance;
pub mod render;
pub mod search;
