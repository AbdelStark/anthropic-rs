# `anthropic-fuzz`

`cargo fuzz` harnesses for the two parsers in this crate that run on
untrusted bytes pulled off the network:

- `parse_error` — decodes the body of a non-success HTTP response.
- `parse_results_jsonl` — decodes the JSON-Lines payload returned by the
  Message Batches results endpoint.

Both parsers are called from the transport critical path in
`anthropic::client::execute_bytes` and therefore have a hard contract that
they never panic on arbitrary input. The targets here enforce that
contract.

## Running locally

`cargo fuzz` requires nightly Rust. From the repository root:

```bash
cd fuzz
cargo +nightly fuzz run parse_error
cargo +nightly fuzz run parse_results_jsonl
```

List available targets:

```bash
cargo +nightly fuzz list
```

Minimize a crash input:

```bash
cargo +nightly fuzz tmin parse_error path/to/crash
```

## CI

A short fuzz smoke test runs on the `Fuzz` GitHub Actions workflow (see
`.github/workflows/fuzz.yml`). It builds every target and runs each for a
few seconds to catch regressions that cause immediate panics — it is not
a substitute for a sustained fuzzing campaign on dedicated hardware.

## Layout

This sub-crate is **intentionally excluded** from the top-level Cargo
workspace so that a plain `cargo test` at the repo root never tries to
resolve `libfuzzer-sys` (which requires nightly). Run every command
above from inside `fuzz/`.
