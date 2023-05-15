# anthropic-rs

Anthropic unofficial Rust SDK  🦀.

[![GitHub Workflow Status](https://github.com/abdelhamidbakhta/anthropic-rs/actions/workflows/test.yml/badge.svg)](https://github.com/abdelhamidbakhta/anthropic-rs/actions/workflows/test.yml)
[![Project license](https://img.shields.io/github/license/abdelhamidbakhta/anthropic-rs.svg?style=flat-square)](LICENSE)
[![Pull Requests welcome](https://img.shields.io/badge/PRs-welcome-ff69b4.svg?style=flat-square)](https://github.com/abdelhamidbakhta/anthropic-rs/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22)

`anthropic-rs` is an unofficial Rust library to interact with Anthropic REST API.

## Usage

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load the environment variables from the .env file.
    dotenv().ok();

    // Build from configuration.
    let cfg = AnthropicConfig::new()?;
    let client = Client::try_from(cfg)?;

    let complete_request = CompleteRequestBuilder::default().prompt("How many toes do dogs have?").build()?;
    // Send a completion request.
    let complete_response = client.complete(complete_request).await?;

    println!("completion response: {complete_response:?}");
    Ok(())
}
```

You can find full working examples in the [examples](examples) directory.

## Configuration

anthropic-rs uses `dotenv` to automatically load environment variables from a `.env` file. You can also set these variables manually in your environment. Here is an example of the configuration variables used:

```bash
ANTHROPIC_API_KEY="..."
ANTHROPIC_DEFAULT_MODEL="claude-v1"
```

Replace the "..." with your actual tokens and preferences.

You can also set these variables manually when you crate a new `Client` instance, see more details in usage section.

## Features

- [ ] Completion (`/v1/complete`)

## Contributing

Contributions to `anthropic-rs` are welcomed! Feel free to submit a pull request or create an issue.

## License

anthropic-rs is licensed under the [MIT License](LICENSE).
