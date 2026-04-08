//! Fuzz target for `anthropic::__fuzz::parse_results_jsonl`.
//!
//! `parse_results_jsonl` runs on the JSON-Lines results file downloaded
//! from `GET /v1/messages/batches/{id}/results`. The body is tens-of-MB
//! of line-delimited JSON, is never pre-validated, and (like `parse_error`)
//! could be tampered with by an adversarial proxy. The parser's contract is:
//!
//! 1. Never panic.
//! 2. Either return a `Vec<BatchResultItem>` of successfully-parsed lines
//!    or an `AnthropicError::InvalidRequest` naming the first bad line.
//!
//! The harness feeds raw bytes directly: the `__fuzz` wrapper handles
//! lossy UTF-8 conversion to mirror what the transport layer does on a
//! real response body.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = anthropic::__fuzz::parse_results_jsonl(data);
});
