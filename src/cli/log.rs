use std::{fmt, io::Write};

#[cfg(feature = "otel")]
use opentelemetry_sdk::trace::Tracer;
use termcolor::{Color, ColorSpec, WriteColor};
use tracing::{Event, Subscriber, level_filters::LevelFilter};
use tracing_subscriber::{
    EnvFilter, Layer, Registry,
    fmt::{
        FmtContext,
        format::{self, FormatEvent, FormatFields},
    },
    layer::SubscriberExt,
    registry::LookupSpan,
    reload,
};

use crate::{process::Process, utils::notify::NotificationLevel};

pub fn tracing_subscriber(
    process: &Process,
) -> (
    impl tracing::Subscriber + use<>,
    reload::Handle<EnvFilter, Registry>,
) {
    #[cfg(feature = "otel")]
    let telemetry = telemetry(process);
    let (console_logger, console_filter) = console_logger(process);
    #[cfg(feature = "otel")]
    {
        (
            Registry::default().with(console_logger).with(telemetry),
            console_filter,
        )
    }
    #[cfg(not(feature = "otel"))]
    {
        (Registry::default().with(console_logger), console_filter)
    }
}

/// A [`tracing::Subscriber`] [`Layer`][`tracing_subscriber::Layer`] that prints out the log
/// lines to the current [`Process`]' `stderr`.
///
/// When the `RUSTUP_LOG` environment variable is present, a standard [`tracing_subscriber`]
/// formatter will be used according to the filtering directives set in its value.
/// Otherwise, this logger will use [`EventFormatter`] to mimic "classic" Rustup `stderr` output.
fn console_logger<S>(process: &Process) -> (impl Layer<S> + use<S>, reload::Handle<EnvFilter, S>)
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    let has_ansi = match process.var("RUSTUP_TERM_COLOR") {
        Ok(s) if s.eq_ignore_ascii_case("always") => true,
        Ok(s) if s.eq_ignore_ascii_case("never") => false,
        // `RUSTUP_TERM_COLOR` is prioritized over `NO_COLOR`.
        _ if process.var("NO_COLOR").is_ok() => false,
        _ => process.stderr().is_a_tty(process),
    };
    let maybe_rustup_log_directives = process.var("RUSTUP_LOG");
    let process = process.clone();
    let logger = tracing_subscriber::fmt::layer()
        .with_writer(move || process.stderr())
        .with_ansi(has_ansi);
    if let Ok(directives) = maybe_rustup_log_directives {
        let (env_filter, handle) = reload::Layer::new(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .parse_lossy(directives),
        );
        (logger.compact().with_filter(env_filter).boxed(), handle)
    } else {
        // Receive log lines from Rustup only.
        let (env_filter, handle) = reload::Layer::new(EnvFilter::new("rustup=INFO"));
        (
            logger
                .event_format(EventFormatter)
                .with_filter(env_filter)
                .boxed(),
            handle,
        )
    }
}

// Adapted from
// https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/trait.FormatEvent.html#examples
struct EventFormatter;

impl<S, N> FormatEvent<S, N> for EventFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: format::Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let has_ansi = writer.has_ansi_escapes();
        let level = NotificationLevel::from(*event.metadata().level());
        {
            let mut buf = termcolor::Buffer::ansi();
            if has_ansi {
                _ = buf.set_color(ColorSpec::new().set_bold(true).set_fg(level.fg_color()));
            }
            _ = write!(buf, "{level}: ");
            if has_ansi {
                _ = buf.reset();
            }
            writer.write_str(std::str::from_utf8(buf.as_slice()).unwrap())?;
        }
        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

impl NotificationLevel {
    fn fg_color(&self) -> Option<Color> {
        match self {
            NotificationLevel::Trace => Some(Color::Blue),
            NotificationLevel::Debug => Some(Color::Magenta),
            NotificationLevel::Info => None,
            NotificationLevel::Warn => Some(Color::Yellow),
            NotificationLevel::Error => Some(Color::Red),
        }
    }
}

/// A [`tracing::Subscriber`] [`Layer`][`tracing_subscriber::Layer`] that corresponds to Rustup's
/// optional `opentelemetry` (a.k.a. `otel`) feature.
#[cfg(feature = "otel")]
fn telemetry<S>(process: &Process) -> impl Layer<S> + use<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    let env_filter = if let Ok(directives) = process.var("RUSTUP_LOG") {
        EnvFilter::builder()
            .with_default_directive(LevelFilter::TRACE.into())
            .parse_lossy(directives)
    } else {
        EnvFilter::new("rustup=TRACE")
    };
    tracing_opentelemetry::layer()
        .with_tracer(telemetry_default_tracer())
        .with_filter(env_filter)
}

/// The default `opentelemetry` tracer used across Rustup.
///
/// # Note
/// This function will panic if not called within the context of a [`tokio`] runtime.
#[cfg(feature = "otel")]
fn telemetry_default_tracer() -> Tracer {
    use std::time::Duration;

    use opentelemetry::{global, trace::TracerProvider as _};
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::{
        Resource,
        trace::{Sampler, SdkTracerProvider},
    };

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_timeout(Duration::from_secs(3))
        .build()
        .unwrap();

    let provider = SdkTracerProvider::builder()
        .with_sampler(Sampler::AlwaysOn)
        .with_resource(Resource::builder().with_service_name("rustup").build())
        .with_batch_exporter(exporter)
        .build();

    global::set_tracer_provider(provider.clone());
    provider.tracer("tracing-otel-subscriber")
}

#[cfg(feature = "otel")]
#[must_use]
pub struct GlobalTelemetryGuard {
    _private: (),
}

#[cfg(feature = "otel")]
pub fn set_global_telemetry() -> GlobalTelemetryGuard {
    opentelemetry::global::set_text_map_propagator(
        opentelemetry_sdk::propagation::TraceContextPropagator::new(),
    );
    GlobalTelemetryGuard { _private: () }
}

#[cfg(feature = "otel")]
impl Drop for GlobalTelemetryGuard {
    fn drop(&mut self) {
        // We're tracing, so block until all spans are exported.
        opentelemetry::global::set_tracer_provider(
            opentelemetry::trace::noop::NoopTracerProvider::new(),
        );
    }
}
