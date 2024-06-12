use std::{fmt, io::Write};

#[cfg(feature = "otel")]
use once_cell::sync::Lazy;
#[cfg(feature = "otel")]
use opentelemetry_sdk::trace::Tracer;
#[cfg(feature = "otel")]
use tracing::Subscriber;
#[cfg(feature = "otel")]
use tracing_subscriber::{registry::LookupSpan, EnvFilter, Layer};

use crate::currentprocess::{process, terminalsource};

macro_rules! warn {
    ( $ ( $ arg : tt ) * ) => ( $crate::cli::log::warn_fmt ( format_args ! ( $ ( $ arg ) * ) ) )
}
macro_rules! err {
    ( $ ( $ arg : tt ) * ) => ( $crate::cli::log::err_fmt ( format_args ! ( $ ( $ arg ) * ) ) )
}
macro_rules! info {
    ( $ ( $ arg : tt ) * ) => ( $crate::cli::log::info_fmt ( format_args ! ( $ ( $ arg ) * ) ) )
}

macro_rules! verbose {
    ( $ ( $ arg : tt ) * ) => ( $crate::cli::log::verbose_fmt ( format_args ! ( $ ( $ arg ) * ) ) )
}

macro_rules! debug {
    ( $ ( $ arg : tt ) * ) => ( $crate::cli::log::debug_fmt ( format_args ! ( $ ( $ arg ) * ) ) )
}

pub(crate) fn warn_fmt(args: fmt::Arguments<'_>) {
    let mut t = process().stderr().terminal();
    let _ = t.fg(terminalsource::Color::Yellow);
    let _ = t.attr(terminalsource::Attr::Bold);
    let _ = write!(t.lock(), "warning: ");
    let _ = t.reset();
    let _ = t.lock().write_fmt(args);
    let _ = writeln!(t.lock());
}

pub(crate) fn err_fmt(args: fmt::Arguments<'_>) {
    let mut t = process().stderr().terminal();
    let _ = t.fg(terminalsource::Color::Red);
    let _ = t.attr(terminalsource::Attr::Bold);
    let _ = write!(t.lock(), "error: ");
    let _ = t.reset();
    let _ = t.lock().write_fmt(args);
    let _ = writeln!(t.lock());
}

pub(crate) fn info_fmt(args: fmt::Arguments<'_>) {
    let mut t = process().stderr().terminal();
    let _ = t.attr(terminalsource::Attr::Bold);
    let _ = write!(t.lock(), "info: ");
    let _ = t.reset();
    let _ = t.lock().write_fmt(args);
    let _ = writeln!(t.lock());
}

pub(crate) fn verbose_fmt(args: fmt::Arguments<'_>) {
    let mut t = process().stderr().terminal();
    let _ = t.fg(terminalsource::Color::Magenta);
    let _ = t.attr(terminalsource::Attr::Bold);
    let _ = write!(t.lock(), "verbose: ");
    let _ = t.reset();
    let _ = t.lock().write_fmt(args);
    let _ = writeln!(t.lock());
}

pub(crate) fn debug_fmt(args: fmt::Arguments<'_>) {
    if process().var("RUSTUP_DEBUG").is_ok() {
        let mut t = process().stderr().terminal();
        let _ = t.fg(terminalsource::Color::Blue);
        let _ = t.attr(terminalsource::Attr::Bold);
        let _ = write!(t.lock(), "debug: ");
        let _ = t.reset();
        let _ = t.lock().write_fmt(args);
        let _ = writeln!(t.lock());
    }
}

/// A [`tracing::Subscriber`] [`Layer`][`tracing_subscriber::Layer`] that corresponds to Rustup's
/// optional `opentelemetry` (a.k.a. `otel`) feature.
#[cfg(feature = "otel")]
pub fn telemetry<S>() -> impl Layer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    // NOTE: This reads from the real environment variables instead of `process().var_os()`.
    let env_filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("INFO"));
    tracing_opentelemetry::layer()
        .with_tracer(TELEMETRY_DEFAULT_TRACER.clone())
        .with_filter(env_filter)
}

/// The default `opentelemetry` tracer used across Rustup.
///
/// # Note
/// The initializer function will panic if not called within the context of a [`tokio`] runtime.
#[cfg(feature = "otel")]
static TELEMETRY_DEFAULT_TRACER: Lazy<Tracer> = Lazy::new(|| {
    use std::time::Duration;

    use opentelemetry::KeyValue;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::{
        trace::{self, Sampler},
        Resource,
    };

    opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_timeout(Duration::from_secs(3)),
        )
        .with_trace_config(
            trace::config()
                .with_sampler(Sampler::AlwaysOn)
                .with_resource(Resource::new(vec![KeyValue::new("service.name", "rustup")])),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .expect("error installing `OtlpTracePipeline` in the current `tokio` runtime")
});
