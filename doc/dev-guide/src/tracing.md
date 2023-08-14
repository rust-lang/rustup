# Tracing

The feature "otel" can be used when building rustup to turn on Opentelemetry
tracing with an OLTP GRPC exporter.

This can be very useful for diagnosing performance or correctness issues in more
complicated scenarios.

## Prerequisites

`protoc` must be installed, which can be downloaded from GitHub or installed via  package manager.

## Usage

The normal [OTLP environment
variables](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/protocol/exporter.md)
can be used to customise its behaviour, but often the simplest thing is to just
run a Jaeger docker container on the same host:

```sh
docker run -d --name jaeger   -e COLLECTOR_ZIPKIN_HOST_PORT=:9411   -e COLLECTOR_OTLP_ENABLED=true   -p 6831:6831/udp   -p 6832:6832/udp   -p 5778:5778   -p 16686:16686   -p 4317:4317   -p 4318:4318   -p 14250:14250   -p 14268:14268   -p 14269:14269   -p 9411:9411   jaegertracing/all-in-one:latest
```

Then build rustup-init with tracing:

```sh
cargo build --features=otel
```

Run the operation you want to analyze:

```sh
RUSTUP_FORCE_ARG0="rustup" ./target/debug/rustup-init show
```

And [look in Jaeger for a trace](http://localhost:16686/search?service=rustup).

## Tracing and tests

Tracing can also be used in tests to get a trace of the operations taken during the test.

The custom macro `rustup_macros::test` adds a prelude and suffix to each test to
ensure that there is a tracing context setup, that the test function is a span,
and that the spans from the test are flushed.

Build with features=otel,test to use this feature.

## Adding instrumentation

The `otel` feature uses conditional compilation to only add function instrument
when enabled. Instrumenting a currently uninstrumented function is mostly simply
done like so:

```rust
#[cfg_attr(feature = "otel", tracing::instrument(err, skip_all))]
```

`skip_all` is not required, but some core structs don't implement Debug yet, and
others have a lot of output in Debug : tracing adds some overheads, so keeping
spans lightweight can help avoid frequency bias in the results - where
parameters with large debug in frequently called functions show up as much
slower than they are.

Some good general heuristics:

- Do instrument slow blocking functions
- Do instrument functions with many callers or that call many different things,
  as these tend to help figure the puzzle of what-is-happening
- Default to not instrumenting thin shim functions (or at least, only instrument
  them temporarily while figuring out the shape of a problem)
- Be way of debug build timing - release optimisations make a huge difference,
  though debug is a lot faster to iterate on. If something isn't a problem in
  release don't pay it too much heed in debug.

## Caveats

Cross-thread propogation isn't connected yet. This will cause instrumentation in
a thread to make a new root span until it is fixed. If any Tokio runtime-related
code gets added in those threads this will also cause a panic. We have a couple
of threadpools in use today; if you need to instrument within that context, use
a thunk to propogate the tokio runtime into those threads.
