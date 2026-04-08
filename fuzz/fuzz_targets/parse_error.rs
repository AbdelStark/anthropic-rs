//! Fuzz target for `anthropic::__fuzz::parse_error`.
//!
//! `parse_error` runs on the raw response body of any non-success HTTP
//! reply the client receives. That body is attacker-controllable — any
//! middlebox, proxy, or compromised upstream can shape it — so the parser
//! must:
//!
//! 1. Never panic, regardless of input bytes.
//! 2. Always return an `AnthropicError` (either a parsed `Api` payload or
//!    an `UnexpectedResponse` fallback).
//!
//! The fuzz input is structured as `[status_byte_hi, status_byte_lo, ...body]`
//! so the harness also exercises every possible HTTP status code. Short
//! inputs default to status 0 so that `cargo fuzz tmin` does not have to
//! carry two bytes before it can reach the parser.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let (status, body): (u16, &[u8]) = match data {
        [a, b, rest @ ..] => (u16::from_be_bytes([*a, *b]), rest),
        _ => (0, data),
    };

    // Calling the real entry point — a panic here is an immediate fuzz
    // finding. We intentionally `drop` the result: the contract is "never
    // panic, always return an error"; we do not assert on which variant
    // appears because both are valid.
    let _ = anthropic::__fuzz::parse_error(status, body);
});
