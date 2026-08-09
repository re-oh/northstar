//! Fuzzes `ContainerReader::parse` against arbitrary bytes. It must never
//! panic — every malformed/adversarial input should come back as a typed
//! `ContainerError`. See `docs/errors.md`'s "explicit errors over panics
//! for anything touching untrusted input" rule; this is what enforces it
//! for the container codec specifically.

#![no_main]

use libfuzzer_sys::fuzz_target;
use northstar_core::ContainerReader;

fuzz_target!(|data: &[u8]| {
    let _ = ContainerReader::parse(data);
});
