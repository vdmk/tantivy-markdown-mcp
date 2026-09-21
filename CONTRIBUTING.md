# Contributing

Before opening a pull request, format, lint, and test your changes.

## Development setup

Install stable Rust with Cargo. Linux and macOS are the intended platforms;
Windows support has not been validated.

Run the same checks used by CI from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

The integration tests use temporary knowledge bases and storage directories.
Live filesystem watcher updates are not yet covered by an integration test.

## Pre-commit hook

To run formatting, Clippy, and the test suite automatically before each commit,
enable the repository hook once after cloning:

```sh
git config core.hooksPath .githooks
```

## Coverage

To build and report LLVM source coverage, run:

```sh
scripts/coverage.sh
```

The integration tests launch the MCP server as a subprocess, so a normal test
run does not measure coverage inside the server process. This script builds an
instrumented server binary and points the test subprocesses at it before
merging their coverage data. It requires the LLVM coverage tools provided by
the installed Rust toolchain.

## Before publishing

To publish a version, update `package.version` in `Cargo.toml` and push the
change to `main`. After CI passes, GitHub Actions creates the matching `v` tag
if it does not already exist.

Run the release checks:

```sh
scripts/pre-publish.sh
```

The script scans committed Git history with Gitleaks and checks Rust
dependencies with `cargo audit`. Install both tools before running it. Run it
again after the final commit so the secret scan includes the release history.
