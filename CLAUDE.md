# init4-bin-base

## Commands

- `cargo +nightly fmt` - format
- `cargo clippy -p init4-bin-base --all-features --all-targets` - lint with features
- `cargo clippy -p init4-bin-base --no-default-features --all-targets` - lint without
- `cargo t -p init4-bin-base` - test specific crate

Pre-push: clippy (both feature sets) + fmt. Never use `cargo check/build`.
These checks apply before any push — new commits, rebases, cherry-picks, etc.

### Pre-push Checks (enforced by Claude hook)

A Claude hook in `.claude/settings.json` runs `.claude/hooks/pre-push.sh`
before every `git push`. The push is blocked if any check fails. The checks:

- `cargo +nightly fmt -- --check`
- `cargo clippy -p init4-bin-base --all-targets --all-features -- -D warnings`
- `cargo clippy -p init4-bin-base --all-targets --no-default-features -- -D warnings`
- `RUSTDOCFLAGS="-D warnings" cargo doc -p init4-bin-base --no-deps`

Clippy and doc warnings are hard failures.

## Style

- Functional combinators over imperative control flow
- `let else` for early returns, avoid nesting
- No glob imports; group imports from same crate
- Private by default, `pub(crate)` for internal, `pub` for API only
- `thiserror` for library errors, `eyre` for apps, never `anyhow`
- Builders for structs with >4 fields or multiple same-type fields
- Tests: fail fast with `unwrap()`, never return `Result`

## Semver and Dependency Bumps

This crate re-exports many dependencies in its public API. Bumping a
re-exported dependency's version is a **breaking change** if the new version
is semver-incompatible. When bumping dependencies, ensure our own version
bump signals the same level of compatibility.

Rules:
- If a re-exported dep gets a semver-incompatible bump, we MUST also bump
  our version incompatibly (i.e. bump minor while pre-1.0).
- If a re-exported dep gets a semver-compatible bump, our version bump need
  not signal incompatibility.
- Pre-1.0 crates treat minor bumps as breaking (e.g. 0.12 -> 0.13 is
  incompatible).

### Re-exported dependencies (public API surface)

Via `pub use` in `deps` module:
- `tracing`, `tracing-core`, `tracing-subscriber`, `tracing-opentelemetry`
- `opentelemetry`, `opentelemetry-otlp`, `opentelemetry-sdk`
- `metrics`

Via public struct fields, function signatures, or trait impls:
- `alloy` — struct fields, params, return types, trait impls
- `reqwest` — struct fields, function params
- `tokio` — watch channels, JoinHandle in public API
- `oauth2` — types in public API
- `axum` — Service/Layer trait impls
- `tower` — Service/Layer trait impls
- `url` — public struct fields
