//! Native dqlrs window.
//!
//! The window calls [`host::Host`] on the same thread. `dql-cli` supplies the
//! live engine. Tests supply [`host::MemoryHost`].

pub mod host;
pub mod rows;
pub mod tokens;

mod app;

pub use app::{run_native, Studio};
