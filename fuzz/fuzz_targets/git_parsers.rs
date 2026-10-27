#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| hushpush_core::fuzzing::git_parsers(data));
