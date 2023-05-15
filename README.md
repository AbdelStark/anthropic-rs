# anthropic-rs

Anthropic unofficial Rust SDK  🦀.

[![GitHub Workflow Status](https://github.com/abdelhamidbakhta/anthropic-rs/actions/workflows/test.yml/badge.svg)](https://github.com/abdelhamidbakhta/anthropic-rs/actions/workflows/test.yml)
[![Project license](https://img.shields.io/github/license/abdelhamidbakhta/anthropic-rs.svg?style=flat-square)](LICENSE)
[![Pull Requests welcome](https://img.shields.io/badge/PRs-welcome-ff69b4.svg?style=flat-square)](https://github.com/abdelhamidbakhta/anthropic-rs/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22)

`anthropic-rs` is an unofficial Rust library to interact with Anthropic REST API.

Features:

- [ ] Completion (`/v1/complete`)

## Table of Contents

- [anthropic-rs](#anthropic-rs)
  - [Table of Contents](#table-of-contents)
  - [Installation](#installation)
  - [Configuration](#configuration)
  - [Usage](#usage)
  - [Contributing](#contributing)
  - [License](#license)

## Installation

Add `anthropic-rs` to your `Cargo.toml` file.

Using `cargo add`:

```bash
cargo add anthropic
```

or manually:

```toml
[dependencies]
anthropic = "0.0.1"
```

## Configuration

anthropic-rs uses `dotenv` to automatically load environment variables from a `.env` file. You can also set these variables manually in your environment. Here is an example of the configuration variables used:

```bash
ANTHROPIC_API_KEY="..."
ANTHROPIC_DEFAULT_MODEL="claude-v1"
```

Replace the "..." with your actual tokens and preferences.

You can also set these variables manually when you crate a new `Client` instance, see more details in usage section.

## Usage

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize the logger.
    env_logger::init();

    // Load the environment variables from the .env file.
    dotenv().ok();

    // Build from configuration.
    let cfg = AnthropicConfig::new()?;
    let client = Client::try_from(cfg)?;

    // Send a completion request.
    client.complete().await?;

    Ok(())
}
```

You can find full working examples in the [examples](examples) directory.

## Contributing

Contributions to `anthropic-rs` are welcomed! Feel free to submit a pull request or create an issue.

## License

anthropic-rs is licensed under the [MIT License](LICENSE).
