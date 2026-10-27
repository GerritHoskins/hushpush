//! Privacy-preserving primitives for LeakLens.

pub mod classify;
pub mod detect;
pub mod extract;
pub mod redact;
pub mod report;
pub mod source;

mod exit_code;
mod sensitive;

pub use exit_code::ExitCode;
pub use sensitive::Sensitive;
