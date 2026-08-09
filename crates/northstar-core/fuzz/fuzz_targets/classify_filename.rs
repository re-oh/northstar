//! Fuzzes `ClassifiedFilename::classify` against arbitrary strings. Must
//! never panic; malformed input comes back as a typed `ClassifyError`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use northstar_core::ClassifiedFilename;

fuzz_target!(|data: &str| {
    let _ = ClassifiedFilename::classify(data);
});
