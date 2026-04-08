# Changelog

All notable changes to `anthropic-rs` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Optional `tracing` Cargo feature. When enabled, every HTTP call on the
  transport critical path emits an `anthropic.http` span with `method`,
  `path`, `status`, `attempts`, and `duration_ms` fields, plus a
  per-attempt `debug!` event carrying the attempt number, response status,
  and attempt duration. The dependency and every instrumentation point
  compile out entirely when the feature is off.
- Per-call retry policy override via the new `RetryPolicy` type and the
  `MessagesRequestBuilder::backoff` / `no_retries` / `retry_policy`
  builder methods (plus the equivalents on `CountTokensRequestBuilder`
  and `CreateBatchRequest`). Interactive paths can opt out of retries
  with `.no_retries()`; background workers can stretch the retry budget
  with `.backoff(ExponentialBackoff { .. })` — all without rebuilding
  the `Client`.
- Pinned MSRV of 1.82 via `package.rust-version` in `anthropic/Cargo.toml`
  and a matching `MSRV` job in `.github/workflows/ci.yml` that reads the
  version from `Cargo.toml` so it stays in sync automatically.
- Supply-chain CI: a new `.github/workflows/supply-chain.yml` runs
  `cargo audit` and `cargo deny` (checking advisories, bans, licenses, and
  sources) on every PR, every push to main, and on a daily schedule.
  Policy is configured in a committed `deny.toml`.
- Fuzz targets for `parse_error` and `parse_results_jsonl` under `fuzz/`,
  wired to a `Fuzz` GitHub Actions workflow that smoke-runs each target
  on nightly. Both parsers sit on the transport critical path and consume
  attacker-controllable bytes, so the harness enforces the
  "never panic on arbitrary input" contract. A small regression corpus
  is baked into the library's `__fuzz` test module so the same invariants
  are checked in regular CI runs.
- `Client` now derives `Clone` and implements `Debug` with the API key
  redacted, so clients can be safely shared across handlers and printed in
  diagnostic logs without leaking credentials.
- `MessagesRequestBuilder` validates `temperature` and `top_p` against
  `[0.0, 1.0]` locally before sending the request.
- `ClientBuilder` rejects empty `api_key`, `api_base`, and `api_version`
  values up front (previously they would surface as opaque API errors).
- `Client::from_env` returns `AnthropicError::InvalidRequest` if
  `ANTHROPIC_TIMEOUT_SECS` cannot be parsed as a positive integer
  (previously the value was silently ignored).
- 429 retries now honor the `Retry-After` response header (integer seconds
  form).
- `CLAUDE.md` agent context file documenting workspace layout, conventions,
  build commands, and gotchas.
- `CHANGELOG.md` (this file).

### Changed
- Streaming transport errors are now surfaced as
  `AnthropicError::EventSource` instead of being string-wrapped into
  `AnthropicError::InvalidRequest`. Callers can now match on the typed
  variant.
- `Client::messages` / `messages_stream` / `count_tokens` / etc. share a
  single `execute_bytes` helper, removing the duplicated retry loop that
  previously lived in `execute` and `execute_raw`.
- README clarifies that `run_tool_loop` executes tool callbacks
  sequentially (the previous wording incorrectly claimed parallel
  execution).
- `.env.example` rewritten to match the variables actually read by
  `Client::from_env` (`ANTHROPIC_API_KEY`, `ANTHROPIC_API_BASE`,
  `ANTHROPIC_API_VERSION`, `ANTHROPIC_BETA`, `ANTHROPIC_TIMEOUT_SECS`).
- Crate `homepage` / `repository` URLs in `Cargo.toml` updated to the
  current `AbdelStark/anthropic-rs` repository.

### Removed
- `anthropic::stream::empty_response` — unused outside its own test and
  trivially constructible by users who actually need an empty
  `MessagesResponse`.

## [0.1.0]

Initial release of the modernized SDK. See git history for details.
