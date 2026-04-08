# CLAUDE.md

Operational notes for Claude Code (and other AI agents) working on this
repository. Read this in full before making changes.

## Project identity

`anthropic-rs` is a typed, async Rust client for Anthropic's Messages API.
It is a **library crate** intended to be consumed via crates.io. There is no
binary and no long-running service. The user is a Rust developer building an
application that talks to `https://api.anthropic.com`.

## Workspace layout

This is a Cargo workspace with **two** workspaces:

```
/                           ← top-level workspace (publishes the SDK)
├── Cargo.toml              ← workspace = ["anthropic"]
├── anthropic/              ← the published `anthropic` crate
│   ├── Cargo.toml
│   ├── README.md           ← also surfaces on crates.io / docs.rs
│   ├── src/
│   │   ├── lib.rs          ← public re-exports
│   │   ├── client.rs       ← Client / ClientBuilder / retries / streaming transport
│   │   ├── error.rs        ← AnthropicError + ApiError payload
│   │   ├── types.rs        ← Messages API request / response / content blocks
│   │   ├── stream.rs       ← StreamAccumulator + collect helpers
│   │   ├── tool_loop.rs    ← run_tool_loop agentic helper
│   │   ├── batches.rs      ← Message Batches API
│   │   ├── count_tokens.rs ← count_tokens endpoint
│   │   └── models.rs       ← list_models / get_model
│   └── tests/              ← wiremock-backed integration tests
└── examples/               ← SECOND workspace, never built by `cargo test` at root
    ├── Cargo.toml          ← workspace = ["basic-messages", "streaming-messages", "tool-loop"]
    ├── basic-messages/
    ├── streaming-messages/
    └── tool-loop/
```

The `examples/` directory is **a separate workspace** so the SDK can be
published without dragging the example crates along. To build / run an example
you must `cd examples` first or use `--manifest-path examples/<name>/Cargo.toml`.

## Tech stack

- **Language**: Rust 2021 edition. No MSRV is currently pinned.
- **Async runtime**: `tokio` (multi-thread, macros).
- **HTTP**: `reqwest` 0.12 with `json` + `stream` features.
- **SSE**: `reqwest-eventsource` 0.6.
- **TLS**: `rustls` (default) or `native-tls` via Cargo features.
- **Retries**: `backoff` 0.4 (`ExponentialBackoff` for 429s, honoring `Retry-After`).
- **Errors**: `thiserror`.
- **Tests**: `wiremock` 0.6 + `dotenvy` (dev only).

## Build / test / lint commands

These commands match what CI runs (`.github/workflows/ci.yml`). They must
all be green before a PR can merge:

```bash
# Format check (uses rustfmt.toml at the repo root)
cargo fmt --all -- --check

# Lint — warnings are errors, run on every target with all features
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Tests — unit + integration + doctests
cargo test --workspace --all-features

# Docs build — no broken intra-doc links
cargo doc --workspace --no-deps --all-features
```

The example workspace is built separately:

```bash
(cd examples && cargo build)
```

## Conventions

### Module / API design

- Public types live in `types.rs` and the per-feature modules
  (`batches.rs`, `count_tokens.rs`, `models.rs`).
- Every request type has a builder (`MessagesRequestBuilder`,
  `CountTokensRequestBuilder`) that validates inputs locally before they
  reach the network. **Add validation to the builder, not to `Client`.**
- HTTP transport, retries, and header construction live in `client.rs` and
  must stay there. Per-endpoint methods on `Client` should be thin wrappers
  that call into `post` / `get` / `delete` / `post_stream`.
- Use the existing `AnthropicError` variants. Avoid stuffing transport
  errors into `AnthropicError::InvalidRequest`; that variant is for local
  validation failures only.
- Public functions/methods must have rustdoc. Failing to document a
  public item is treated as a bug.

### Error handling

- Errors are typed and propagate via `Result<_, AnthropicError>`. No
  `unwrap` / `expect` outside tests.
- API errors deserialized from response bodies become
  `AnthropicError::Api(ApiError)`. Non-JSON failure bodies become
  `AnthropicError::UnexpectedResponse { status, body }`.
- Stream transport errors are `AnthropicError::EventSource(...)`.
- Local validation errors are `AnthropicError::InvalidRequest(String)`.

### Testing

- Unit tests live in `#[cfg(test)] mod tests` blocks at the bottom of each
  source file. Integration tests live in `anthropic/tests/`.
- Network is **always** mocked via `wiremock`. Tests must not hit the real
  Anthropic API. There is no test in this repo that requires
  `ANTHROPIC_API_KEY` to be set.
- Every fix for a reported bug must come with a test that would have
  caught it.
- New public API surface must come with at least one happy-path test and
  one failure-path test.

### Style

- `rustfmt.toml` at the repo root sets `max_width = 120` and
  `use_small_heuristics = "Max"`. Run `cargo fmt --all` before committing.
- Prefer `impl Into<String>` for owned-string parameters in builder /
  constructor signatures.
- Avoid `pub use` of nested re-exports unless the symbol is part of the
  primary onboarding surface (already in `lib.rs`).

### Commits

- Conventional-commit style is preferred (see `git log`):
  `feat(types): ...`, `fix(client): ...`, `docs: ...`, `chore: ...`.
- Keep commits focused. Mixed-purpose commits make `git blame` painful.

## Critical constraints

- **Never commit secrets.** `.env` is in `.gitignore`. Tests must use
  `test-key` (or similar dummy values) and a `wiremock::MockServer` URL.
- **Never log the API key.** `Client::Debug` already redacts it; do not
  override or weaken that. New code that touches `Client` must keep the
  redaction.
- **Never make a real API call from a test.** Use `wiremock`.
- **Do not change `MessagesRequest` field shape** without updating both
  the request and response serde tests in `types.rs`. The wire format
  must match Anthropic's documented schema exactly.
- **Do not introduce a synchronous (blocking) public API.** This crate is
  async-only.
- **Do not add a dependency** without a clear, demonstrable need. Every
  dependency is a liability and a supply-chain risk.

## Gotchas (things that have tripped people up)

- **`examples/` is a separate workspace.** `cargo test` from the repo root
  will NOT touch the example crates, and `cargo build` from the root will
  not build them either. CI builds them via the example workspace.
- **`Client::messages` rejects `stream=true`** with `InvalidRequest`. Use
  `Client::messages_stream` for streaming.
- **`reqwest::Request::try_clone()` returns `None` for streaming bodies.**
  When that happens we cannot retry — `execute_bytes` falls back to a
  single attempt. None of the current endpoints stream a request body, so
  this should never trigger in practice.
- **`backoff` retries indefinitely until it hits its `max_elapsed_time`**
  (default 15 minutes). Tests that exercise the retry path must use
  `up_to_n_times(N)` on the `MockServer` to bound the loop.
- **`Retry-After` is honored in seconds only.** HTTP-date format is
  intentionally not parsed (it's exotic for `429`s and adds a date-parsing
  dependency). See `parse_retry_after` in `client.rs`.
- **`StreamAccumulator` requires a `message_start` event before any
  delta.** Out-of-order events return `AnthropicError::InvalidRequest`.

## Current state

- Version: `0.1.0` (workspace crate `anthropic`).
- Status: Beta. Public API surface covers Messages, count_tokens, Models,
  Message Batches, streaming, and a tool-use loop helper.
- CI: fmt + clippy + tests + docs on every PR.
- Known limitations: no `ANTHROPIC_AUTH_TOKEN` / OAuth support; no
  request-level retry policy override (only the client-wide
  `ExponentialBackoff`); HTTP-date format for `Retry-After` is ignored.

If you're about to make a change that contradicts anything above, stop and
flag it before proceeding.
