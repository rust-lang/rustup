# Tracing

Similar to other tools in the Rust ecosystem like rustc and cargo,
rustup also provides observability/logging features via the `tracing` crate.

The verbosity of logs is controlled via the `RUSTUP_LOG` environment
variable using `tracing_subscriber`'s [directive syntax].

[directive syntax]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives

## Console-based tracing

A `tracing_subscriber` that prints log lines directly to `stderr` is directly
available in the prebuilt version of rustup since v1.28.0.

For historical reasons, if `RUSTUP_LOG` is not set, this subscriber will print
the log lines in a format that mimics the "legacy" `stderr` output in older
versions of rustup:

```console
> rustup default stable
info: using existing install for 'stable-aarch64-apple-darwin'
info: default toolchain set to 'stable-aarch64-apple-darwin'

  stable-aarch64-apple-darwin unchanged - rustc 1.79.0 (129f3b996 2024-06-10)
```

However, once `RUSTUP_LOG` is set to any value, rustup's "custom logging mode" will
be activated, and `tracing_subscriber`'s builtin output format will be used instead:

```console
> RUSTUP_LOG=trace rustup default stable
2024-06-16T12:08:48.732894Z  INFO rustup::cli::common: using existing install for 'stable-aarch64-apple-darwin'
2024-06-16T12:08:48.739232Z  INFO rustup::cli::common: default toolchain set to 'stable-aarch64-apple-darwin'

  stable-aarch64-apple-darwin unchanged - rustc 1.79.0 (129f3b996 2024-06-10)
```

Please note that since `RUSTUP_LOG=trace` essentially accepts log lines from
all possible sources, you might sometimes see log lines coming from rustup's
dependencies, such as `hyper_util` in the following example:

```console
> RUSTUP_LOG=trace rustup update
[..]
2024-06-16T12:12:45.569428Z TRACE hyper_util::client::legacy::client: http1 handshake complete, spawning background dispatcher task
2024-06-16T12:12:45.648682Z TRACE hyper_util::client::legacy::pool: pool dropped, dropping pooled (("https", static.rust-lang.org))

   stable-aarch64-apple-darwin unchanged - rustc 1.79.0 (129f3b996 2024-06-10)
  nightly-aarch64-apple-darwin unchanged - rustc 1.81.0-nightly (3cf924b93 2024-06-15)

2024-06-16T12:12:45.693350Z  INFO rustup::cli::rustup_mode: cleaning up downloads & tmp directories
```

It is also possible to limit the sources of the log lines and the desired
max level for each source. For example, set `RUSTUP_LOG=rustup=DEBUG` to
receive log lines only from `rustup` itself with a max verbosity of `DEBUG`.

## Opentelemetry tracing

> **Prerequisites:** Before following the instructions in this section,
> `protoc` must be installed, which can be downloaded from GitHub
> or installed via a package manager.

The feature `otel` can be used when building rustup to turn on Opentelemetry
tracing with an OLTP GRPC exporter.

This can be very useful for diagnosing performance or correctness issues in more
complicated scenarios.

The normal [OTLP environment
variables](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/protocol/exporter.md)
can be used to customise its behaviour, but often the simplest thing is to just
run a Jaeger docker container on the same host:

```sh
docker run -d --name jaeger   -e COLLECTOR_ZIPKIN_HOST_PORT=:9411   -e COLLECTOR_OTLP_ENABLED=true   -p 6831:6831/udp   -p 6832:6832/udp   -p 5778:5778   -p 16686:16686   -p 4317:4317   -p 4318:4318   -p 14250:14250   -p 14268:14268   -p 14269:14269   -p 9411:9411   jaegertracing/all-in-one:latest
```

Then build `rustup-init` with tracing:

```sh
cargo build --features=otel
```

Run the operation you want to analyze. For example, we can now run `rustup show` with tracing:

```sh
RUSTUP_FORCE_ARG0="rustup" ./target/debug/rustup-init show
```

And [look in Jaeger for a trace](http://localhost:16686/search?service=rustup).

Tracing can also be used in tests to get a trace of the operations taken during the test.
To use this feature, build the project with `--features=otel,test`.

### Adding instrumentation

The `otel` feature uses conditional compilation to only add function instrument
when enabled. Instrumenting a currently uninstrumented function is mostly simply
done like so:

```rust
#[tracing::instrument(level = "trace", err(level = "trace"), skip_all)]
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
