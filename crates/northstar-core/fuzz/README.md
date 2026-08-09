# northstar-core fuzz targets

Requires a nightly Rust toolchain (unlike the rest of the workspace — see
`../../../rust-toolchain.toml`) and `cargo-fuzz`:

```sh
cargo install cargo-fuzz
rustup toolchain install nightly
```

Run a target (from this `fuzz/` directory):

```sh
cargo +nightly fuzz run container_parse
cargo +nightly fuzz run classify_filename
```

`tests/fixtures/` one level up (`../tests/fixtures/valid` and
`../tests/fixtures/corrupted`) makes a reasonable starting seed corpus —
copy those files into `corpus/<target-name>/` before a long run so the
fuzzer starts from realistic-shaped inputs rather than empty/random bytes.

Both targets assert only one thing: **no panic**. Malformed input coming
back as a `ContainerError`/`ClassifyError` is success, not a finding — see
`docs/errors.md`.
