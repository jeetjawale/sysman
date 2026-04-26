# Contributing to Sysman

First off, thank you for considering contributing to `sysman`! It's people like you who make the open-source community such a great place to learn, inspire, and create.

## How Can I Contribute?

### Reporting Bugs
- Use the GitHub Issue Tracker.
- Describe the bug and include steps to reproduce.
- Include your OS and environment details (output of `sysman system` is helpful!).

### Suggesting Enhancements
- Open a GitHub Issue with the "enhancement" label.
- Explain why the feature would be useful.

### Pull Requests
1. **Fork the repo** and create your branch from `main`.
2. **Install dependencies**: Ensure you have Rust and the necessary system tools (`lm-sensors`, etc.) installed.
3. **Format your code**: Run `cargo fmt` before committing.
4. **Lint your code**: Run `cargo clippy` and ensure there are no warnings.
5. **Write tests**: If you're adding a new collector or logic, please add unit tests where possible.
6. **Submit the PR**: Provide a clear description of the changes.

## Development Workflow

### Standard Checks
Before submitting a PR, please run these commands:

```bash
cargo fmt --all -- --check
cargo clippy -- -D warnings
cargo test
```

### Style Guidelines
- Follow standard Rust naming conventions.
- Keep the TUI focused and high-performance.
- Avoid adding heavy dependencies unless necessary.

## Community
By participating in this project, you agree to abide by the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md).

Happy coding!
