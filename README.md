# init4 bin-base

[![Crates.io](https://img.shields.io/crates/v/init4-bin-base.svg)](https://crates.io/crates/init4-bin-base)
[![Documentation](https://docs.rs/init4-bin-base/badge.svg)](https://docs.rs/init4-bin-base)
[![CI](https://github.com/init4tech/bin-base/actions/workflows/rust.yml/badge.svg)](https://github.com/init4tech/bin-base/actions/workflows/rust.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

Common functionality for binaries produced by the [`init4`] project. This crate provides:

- Environment parsing utilities
- Standard [`tracing`] setup with [`otlp`] support
- Standard server for Prometheus [`metrics`]
- Standard environment variables to configure these features

> **Note:** This crate is intended as a base for all binaries in the init4 project. It is not intended for outside consumption.

## Installation

```toml
[dependencies]
init4-bin-base = "0.18"
```

## Quick Start

```rust
use init4_bin_base::init4;

fn main() {
    init4();
    // your code here
}
```

Build the crate docs with `cargo doc --open` for more details.

[`init4`]: https://init4.technology
[`tracing`]: https://docs.rs/tracing/latest/tracing/
[`otlp`]: https://docs.rs/tracing-opentelemetry/latest/tracing_opentelemetry/
[`metrics`]: https://docs.rs/metrics/latest/metrics/

---

# init4 Tracing Best Practices

## Carefully Consider Level

Event and span level should correspond to the significance of the event as follows:

| Level | Usage | Examples |
|-------|-------|----------|
| `TRACE` | Low-level, detailed debugging info. Use rarely. | HTTP request body, every network packet |
| `DEBUG` | Low-level lifecycle info useful for debugging. Use sparingly. | Single DB query result, single function call result |
| `INFO` | Normal operation lifecycle info. Default level for most events. | Request processing start, DB connection established |
| `WARN` | Potential problems that don't prevent operation. | Request took longer than expected, ignored parse error |
| `ERROR` | Problems that prevent correct operation. | DB connection failed, required file not found |

By default, the OTLP exporter captures `DEBUG` and higher. Configure with `OTEL_LEVEL` env var.
The log formatter logs at `INFO` level. Configure with `RUST_LOG` env var.

```rust
// ❌ Avoid
warn!("Connected to database");

// ✅ Instead
info!("Connected to database");
```

## Import from bin-base

Re-export all necessary crates from `init4-bin-base` rather than adding them to your `Cargo.toml`:

```rust
// ❌ Avoid
use tracing::info;

// ✅ Instead
use init4_bin_base::deps::tracing::info;
```

---

# Spans

Spans represent the duration of a unit of work. They should be:

- **Time-limited** — at most a few seconds
- **Work-associated** — tied to a specific action
- **Informative** — have useful data, not over-verbose

## Inheritance

Spans inherit the currently-entered span as their parent. Avoid spurious span relationships:

```rust
// ❌ Avoid — accidental parent-child relationship
let span = info_span!("outer_function").entered();
let my_closure = || {
    let span = info_span!("accidental_child").entered();
    // do some work
};
do_work(closure);

// ✅ Instead — closure span created before outer span
let my_closure = || {
    let span = info_span!("not_a_child").entered();
    // do some work
};
let span = info_span!("outer_function").entered();
do_work(closure);
```

## Avoid Over-Verbose Spans

When instrumenting methods, skip `self` and add only needed fields:

```rust
// ❌ Avoid — self will be Debug-printed (verbose)
#[instrument]
async fn my_method(&self) { }

// ✅ Instead — skip self, add specific fields
#[instrument(skip(self), fields(self.id = self.id))]
async fn my_method(&self) { }
```

For multiple arguments, skip all and add back what you need:

```rust
// ❌ Avoid
#[instrument]
async fn my_method(&self, arg1: i32, arg2: String) { }

// ✅ Instead
#[instrument(skip_all, fields(arg1))]
async fn my_method(&self, arg1: i32, arg2: String) { }
```

## Instrument Futures, Not JoinHandles

```rust
// ❌ Avoid — span won't propagate to the future
tokio::spawn(fut).instrument(span);

// ✅ Instead
tokio::spawn(fut.instrument(span));
```

## Instrument Work, Not Tasks

Avoid adding spans to long-running tasks. Create spans in the internal loop instead:

```rust
// ❌ Avoid — span open for entire task lifetime
let span = info_span!("task");
tokio::spawn(async {
    loop {
        // work
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}.instrument(span));

// ✅ Instead — span per iteration
tokio::spawn(async {
    loop {
        let span = info_span!("loop_iteration").entered();
        // work
        drop(span);
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
});
```

## Root Spans

Root spans are top-level spans in a trace. Ensure they correspond to a SINGLE UNIT OF WORK:

```rust
// ❌ Avoid — nested work units under one span
let span = info_span!("task");
for item in my_vec {
    let work_span = info_span!("work_unit").entered();
    // work
}

// ✅ Instead — each work unit is a root span
let work_loop = info_span!("work_loop").entered();
for item in my_vec {
    let span = info_span!(parent: None, "work_unit").entered();
    // work
    drop(span);
}
```

With `#[instrument]`:

```rust
// ✅ Create root span
#[instrument(parent = None)]
async fn a_unit_of_work() { }
```

## Be Careful with `instrument(err)`

Using `#[instrument(err)]` emits errors at EACH span level. Only root spans should have `instrument(err)`:

```rust
// ❌ Avoid — error emitted multiple times
#[instrument(err)]
async fn one() -> Result<(), ()> { }

#[instrument(err)]
async fn two() -> Result<(), ()> {
    one().await?;
}

// ✅ Instead — only root span has err
#[instrument]
async fn one() -> Result<(), ()> { }

#[instrument(parent = None, err)]
async fn two() -> Result<(), ()> {
    one().await?;
}
```

To track error bubbling, record additional info:

```rust
#[instrument(err)]
async fn do_thing() -> std::io::Result<()> {
    do_inner().await.inspect_err(|_| {
        tracing::span::Span::current().record("err_source", "do_inner");
    })
}
```

---

# Managing Events

Events represent state at a single point in time. They should be:

- **Informative** — useful data, not over-verbose
- **Descriptive** — clear, concise messages
- **Lifecycle-aware** — record lifecycle of a unit of work
- **Non-repetitive** — fire ONCE in a span's lifetime

## Avoid String Interpolation

Events are structured data. String interpolation loses type information:

```rust
// ❌ Avoid
info!("Value calculated: {}", x);

// ✅ Instead
info!(x, "Value calculated");
```

## Lifecycle Events

Events should capture significant lifecycle steps, not every step:

```rust
// ❌ Avoid — using events for start/end
info!("Parsing input");
let parsed = parse_input(input);
info!("Input parsed");

// ✅ Instead — use spans
let span = info_span!("parse_input").entered();
let parsed = parse_input(input);
drop(span);

// ✅ Even better — use #[instrument]
#[instrument(skip(input), fields(input_size = input.len()))]
fn parse_input(input: String) -> Option<ParsedInput> { }
```

## DRY: Don't Repeat Yourself (at INFO and DEBUG)

If firing the same event many times, you're violating span rules or verbosity rules:

```rust
// ❌ Avoid — same event many times
for i in my_vec {
    info!(i, "processing");
    do_work(i);
}

// ✅ Instead — trace per item, info for summary
for i in my_vec {
    do_work(i);
    trace!(i, "processed vec item");
}
info!(my_vec.len(), "processed my vec");
```

## License

This project is licensed under the [MIT License](LICENSE).
