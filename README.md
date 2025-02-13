# init4 bin base

This crate bundles common functionality for binaries produced by the [`init4`]
project. It provides:

- environment parsing utilities
- a standard [`tracing`] setup with [`otlp`] support
- a standard server for prometheus [`metrics`]
- standard environment variables to configure these features

This crate is intended to be used as a base for all binaries produced by the
`init4` project. It is not intended for outside consumption.

```rust
use init4_bin_base::init4;

fn main() {
    init4();
    // your code here
}

```

Build the crate docs with `cargo doc --open` to learn more.

[`init4`]: https://init4.technology
[`tracing`]: https://docs.rs/tracing/latest/tracing/
[`otlp`]: https://docs.rs/tracing-opentelemetry/latest/tracing_opentelemetry/
[`metrics`]: https://docs.rs/metrics/latest/metrics/
