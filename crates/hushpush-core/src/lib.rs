//! Privacy-preserving primitives for hushpush.

pub mod allow;
pub mod classify;
pub mod config;
pub mod detect;
pub mod extract;
pub mod patterns;
pub mod redact;
pub mod report;
pub mod source;

mod exit_code;
mod sensitive;

pub use exit_code::ExitCode;
pub use sensitive::Sensitive;

pub mod ocr;
