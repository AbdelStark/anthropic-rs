# Changelog

All notable changes to `anthropic-rs` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
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
