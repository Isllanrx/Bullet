#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod domain;
mod runtime;

pub use domain::{historic, library, mods, overlay, party};
pub use runtime::{phase, selection, state, supervisor};

pub mod env;
pub mod error;
