# Repository Guidelines

## Project Structure & Module Organization

This repository is a small Rust terminal user-interface (TUI) application for
the Lance Lakehouse Format. Application code currently lives in
`src/main.rs`; as the project grows, keep additional Rust modules under
`src/` and place integration tests under `tests/`. `Cargo.toml` defines the
package and dependencies, `Cargo.lock` records resolved versions, and
`rust-toolchain.toml` pins Rust `1.97.0` with `rustfmt`, Clippy, and
Rust Analyzer. There are no separate asset or fixture directories yet.

## Build, Test, and Development Commands

Run commands from the repository root:

- `cargo run` builds and launches the viewer locally.
- `cargo check` performs a fast compile-time validation without producing a
  final executable.
- `cargo build` creates a debug build; use `cargo build --release` for an
  optimized build.
- `cargo test` runs unit and integration tests. It is currently expected to
  report no defined tests until coverage is added.
- `cargo fmt --all -- --check` verifies formatting; run `cargo fmt --all` to
  apply it.
- `cargo clippy --all-targets --all-features -- -D warnings` checks common
  Rust issues and treats warnings as failures.

## Coding Style & Naming Conventions

Use standard `rustfmt` formatting with four-space indentation and no manual
alignment. Follow Rust naming conventions: `snake_case` for functions,
variables, and modules; `UpperCamelCase` for types and traits; and `SCREAMING_SNAKE_CASE`
for constants. Prefer small, focused modules and explicit error handling over
panics in runtime paths. Keep dependency changes in `Cargo.toml` and commit
the corresponding `Cargo.lock` updates.

## Testing Guidelines

Use Rust’s built-in test framework (`#[test]`). Keep unit tests near the code
they exercise and integration tests in `tests/`, naming test files and
functions after the behavior under test (for example,
`tests/lance_open.rs` and `opens_valid_dataset`). Add regression tests for
new parsing, data-access, or UI-state behavior and run `cargo test` locally.

## Commit & Pull Request Guidelines

Existing commits use short, imperative descriptions such as `project setup`.
Keep commits focused and use a concise subject describing the change (for
example, `add dataset browser`). Pull requests should explain the user-visible
or architectural change, mention validation commands run, link a related
issue when one exists, and include terminal screenshots or a short recording
for visible TUI changes.

## Security & Configuration Tips

Do not commit datasets, credentials, generated binaries, or machine-specific
configuration. Review Lance input paths and any filesystem access carefully,
especially when adding commands that open or modify user data.
