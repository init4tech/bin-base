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

# init4 Tracing best practices

## **Carefully consider level**

Event and span level should correspond to the significance of the event as follows:

- `TRACE` - low-level, detailed information. This is useful for debugging, but is not necessary for normal operation. This may include very large data or very frequent events. This should be used rarely. Examples: the body of an http request. Every packet received by a network server.
- `DEBUG` - low-level lifecycle information that is useful for debugging, but not for tracing normal operation. This should be used sparingly. Examples: the result of a single database query. The result of a single function call.
- `INFO` - lifecycle information that is useful for understanding the operation of the system. This should be the default level for most events. Examples: the start of request processing. Connection established to a database. Initialization of a service succeeded.
- `WARN` - lifecycle information that indicates a potential problem. These are often ignorable errors that may indicate a problem, but do not prevent the system from operating. Examples: a request took longer than expected. Input failed to parse, but was ignored.
- `ERROR` - lifecycle information that indicates a problem that prevents the system from operating correctly. These are often fatal errors that require human intervention. Examples: a database connection failed. A required file was not found.

By default, our OTLP exporter captures `DEBUG` and higher level events. This means that `trace!` events will not be exported by default. If you need to capture these events, you can change the level of the exporter using the `OTEL_LEVEL` env var.

Our log formatter logs at `INFO` level, so `trace!` and `debug!` events will not be visible in the logs. This can be configured with the `RUST_LOG` env var.

```rust
// avoid this
warn!("Connected to database");

// instead do this
info!("Connected to database");
```

## Import from bin-base

We re-export all necessary crates from `init4-bin-base`. Use those as re-imports, rather than adding them to the local project’s `Cargo.toml`. This is not super important, but just nice for simplifying our dep trees and avoiding having multiple crate versions at once.

Yes this is more verbose, and we may choose to not like this later.

```rust
// avoid this
use tracing::info;

// instead do this
use init4_bin_base::deps::tracing::info;
```

# **Spans**

Spans represent the duration of a unit of work. They are the primary data structure in tracing, and wrap span events produced by the event macros. Spans should be

- time-limited - at most a few seconds.
- work-associated - a specific action the program is taking.
- informative - have useful data attached to them, and not over-verbose.

## **Inheritance**

Spans inherit the currently-entered span as their parent, when they are created. Try to avoid spurious span relationships, as it can lead to non-useful trace data.

```rust
// avoid this
let span = info_span!("outer_function").entered();
...
let my_closure = || {
    let span = info_span!("accidental_child").entered();
// do some work
};
do_work(closure);

// instead do this
let my_closure = || {
    let span = info_span!("not_a_child").entered();
    // do some work
};
let span = info_span!("outer_function").entered();
do_work(closure);
```

This is related to [Root Spans](https://www.notion.so/init4-Tracing-best-practices-19e3e1cd453980409bd3eb5c9866d229?pvs=21)

## **Over-verbose spans**

When instrumenting a `fn` that takes `self` as its first argument, you should almost always `skip(self)`. Add properties from `self` to the span manually. By default, the `self` argument will be added to the span as a field using its `Debug` implementation. This can be very verbose, and is rarely useful. It is better to add only the fields you need to the span.

```rust
// avoid this
#[instrument]
async fn my_method(&self) {
// ...
}

// instead do this
#[instrument(skip(self), fields(self.id = self.id))]
async fn my_method(&self) {
// ...
}
```

When there are several arguments, you should consider skipping all of them, and then adding back only the ones you need. This makes the span more readable, and makes it clear which arguments are important.

```rust
// avoid this
#[instrument]
async fn my_method(&self, arg1: i32, arg2: String) {
// ...
}

// instead do this
#[instrument(skip_all, fields(arg1))]
async fn my_method(&self, arg1: i32, arg2: String) {
// ...
}

// this works too, but is not preferred, as it is not immediately clear
#[instrument(skip(self, arg2))]
async fn my_method(&self, arg1: i32, arg2: String) {
// ...
}
```

## **Instrument futures, not `JoinHandle`s**

When spawning a future, use the `instrument` method on the future itself, rather than on the `JoinHandle` that `tokio::spawn` returns. Attaching the span to the `JoinHandle` will NOT propagate the span to the future when it runs, and the span contents will not be recorded alongside the future's events.

```rust
// avoid this
tokio::spawn(fut).instrument(span);

// instead do this
tokio::spawn(fut.instrument(span));
```

## **Instrument work, not tasks.**

Avoid adding spans to tasks that you expect to run for a long time. Instead, make a new span in the task's internal working loop each time it runs. Then drop that span at the end of the loop. This way, the span will be closed and its contents recorded each time the task runs, rather than being left open for the entire lifetime of the task.

```rust
// avoid this
let span = info_span!("task");
tokio::spawn(async {
    loop {
		// do some work
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}.instrument(span));

// instead do this
tokio::spawn(async {
    loop {
        let span = info_span!("loop_iteration").entered();
				// do some work
        drop(span);
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
};
```

## **Root spans**

When creating a span, think carefully if it should be a "root" span. Root spans are the top-level spans in a trace, and are the entry points for the trace. In groundcover, a root span will be the base of the UI tree. As such, it is important that your root span corresponds to a SINGLE UNIT OF WORK.

If your span contains a succession of work units, consider making a new root root span for each work unit. This will make the trace easier to read and understand.

This is related to [Instrument work, not tasks](https://www.notion.so/init4-Tracing-best-practices-19e3e1cd453980409bd3eb5c9866d229?pvs=21), but is a higher-level concern.

```rust
// avoid this
let span = info_span!("task");
for item in my_vec {
    let work_span = info_span!("work_unit").entered();
    // do some work
}

// instead do this
let work_loop = info_span!("work_loop").entered();
for item in my_vec {
    let span = info_span!(parent: None, "work_unit").entered();
    // do some work
    drop(span);
}

```

When using the `#[instrument]` attribute, you can set `parent` to `None` to create a root span.

```rust
// avoid this
#[instrument]
async fn a_unit_of_work() {
    // do some work
}

// instead do this
#[instrument(parent = None)]
async fn a_unit_of_work() {
    // do some work
}
```

## Be careful using `instrument(err)`

Using `#[instrument(err)]` can result in the same error being emitted many times, as EACH span generated this way will contain a unique `error!` event.

Instead, **only root spans should have `instrument(err)` on them.** This ensures that we always associate the error with the root span that it terminated, rather than associating it at each individual level

```rust
// avoid this
#[instrument(err)]
async fn one() -> Result<(), ()> {
   // do work
}

#[instrument(err)]
async fn two() -> Result<(), ()> {
	one().await?;
	// more work
}

// instead do this
#[instrument]
async fn one() -> Result<(), ()> {
    // do work
}

#[instrument(parent = None, err)]
async fn two() -> Result<(), ()> {
   one().await?;
   // more work
}
```

If we need to inspect the error-bubbling process more, we can add additional info to the current span like this:

```rust
#[instrument]
async fn do_thing2() -> std::io::Result<()> { }

#[instrument(err)]
async fn do_thing3() -> std::io::Result<()> {
    do_thing2().await.inspect_err(|_| {
        tracing::span::Span::current().record("err_source", "do_thing2");
    })
}
```

# **Managing Events**

Events represent state a single point in time. They are associated with the currently-open span. In init4 systems, each event will result in a log line AND will be exported via OTLP to the tracing backend. Events should be:

- informative - have useful data attached to them, and not over-verbose.
- descriptive - the message should be clear and concise.
- lifecycle-aware - events should be used to record the lifecycle of a unit of work.
- non-repetitive - the event should fire ONCE in its span's lifetime.

## **Avoid string interpolation**

Events are structured data, like JSON. They are converted to log lines by a specific formatter, but also kept as fully typed data. The typed data is exported via OTLP. Using string interpolation leads to loss of type information, and can make the data harder to parse and use.

```rust
// avoid this
info!("Value calculated: {}", x);

// instead do this
info!(x, "Value calculated");
```

## **Lifecycle events**

Events should be used to record changes in the state of a unit of work. This means they should capture significant lifecycle steps. Avoid using events to record every step in a process, as this can lead to over-verbose traces. And avoid using events to record starts/ends of a process, as this is what spans are for. Events should be achievements, not steps.

```rust
// avoid this
info!("Parsing input");
let parsed = parse_input(input);
info!("Input parsed");

// instead do this
let span = info_span!("parse_input").entered();
let parsed = parse_input(input);
drop(span);

// even better
#[instrument(skip(input), fields(input_size = input.len()))]
fn parse_input(input: String) -> Option<ParsedInput> {
    // do some work
}
let parsed = parse_input(input);
```

## DRY: Don’t Repeat Yourself (at info and debug)

If you’re firing the same event many times, it’s likely that you’re violating either a span rule, or a verbosity rule. We want to instrument a **single unit of work,** which implies that everything important happens _once_. If the same event is firing many times in the same span, we’ve probably got some poorly designed spans that need to be fixed, or we are firing unimportant events and their level should be downgraded.

```rust
// avoid this
for i in my_vec {
    info!(i, "processing");
    do_work(i);
}

// instead do this
for i in my_vec {
    do_work(i);
    trace!(i, "processed vec item");
}
info!(my_vec.len(), "processed my vec");
```
