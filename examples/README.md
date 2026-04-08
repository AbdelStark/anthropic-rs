# Examples

You can find a list of working Rust projects using anthropic-rs in this workspace.

To run an example, use the following command:

```bash
cd <example_name>
cargo run
```

For example, to run the `basic-messages` example:

```bash
cd basic-messages
cargo run
```

## List of examples

- [basic-messages](basic-messages): A basic example of the Messages API.
- [streaming-messages](streaming-messages): An example of streaming Messages
  responses, using `StreamAccumulator` to rebuild the final `MessagesResponse`.
- [tool-loop](tool-loop): End-to-end tool-use demo driven by
  `run_tool_loop`. The helper handles the call/execute/reply cycle so the
  example only needs to implement the tool logic.
