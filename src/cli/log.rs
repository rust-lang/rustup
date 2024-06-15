use std::{fmt, io::Write};

use termcolor::{Color, ColorSpec, WriteColor};
use tracing::{level_filters::LevelFilter, Event, Subscriber};
use tracing_subscriber::{
    fmt::{
        format::{self, FormatEvent, FormatFields},
        FmtContext,
    },
    registry::LookupSpan,
    EnvFilter, Layer,
};

#[cfg(feature = "otel")]
use once_cell::sync::Lazy;
#[cfg(feature = "otel")]
use opentelemetry_sdk::trace::Tracer;

use crate::{currentprocess::Process, utils::notify::NotificationLevel};

macro_rules! debug {
    ( $ ( $ arg : tt ) * ) => ( ::tracing::trace ! ( $ ( $ arg ) * )  )
}

macro_rules! verbose {
    ( $ ( $ arg : tt ) * ) => ( ::tracing::debug ! ( $ ( $ arg ) * )  )
}

macro_rules! info {
    ( $ ( $ arg : tt ) * ) => ( ::tracing::info ! ( $ ( $ arg ) * )  )
}

macro_rules! warn {
    ( $ ( $ arg : tt ) * ) => ( ::tracing::warn ! ( $ ( $ arg ) * )  )
}

macro_rules! err {
    ( $ ( $ arg : tt ) * ) => ( ::tracing::error ! ( $ ( $ arg ) * )  )
}

pub fn tracing_subscriber(process: Process) -> impl tracing::Subscriber {
    use tracing_subscriber::{layer::SubscriberExt, Registry};

    #[cfg(feature = "otel")]
    let telemetry = telemetry(&process);
    let console_logger = console_logger(process);
    #[cfg(feature = "otel")]
    {
        Registry::default().with(console_logger).with(telemetry)
    }
    #[cfg(not(feature = "otel"))]
    {
        Registry::default().with(console_logger)
    }
}

/// A [`tracing::Subscriber`] [`Layer`][`tracing_subscriber::Layer`] that prints out the log
/// lines to the current [`Process`]' `stderr`.
///
/// When the `RUST_LOG` environment variable is present, a standard [`tracing_subscriber`]
/// formatter will be used according to the filtering directives set in its value.
/// Otherwise, this logger will use [`EventFormatter`] to mimic "classic" Rustup `stderr` output.
fn console_logger<S>(process: Process) -> impl Layer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    let has_ansi = match process.var("RUSTUP_TERM_COLOR") {
        Ok(s) if s.eq_ignore_ascii_case("always") => true,
        Ok(s) if s.eq_ignore_ascii_case("never") => false,
        // `RUSTUP_TERM_COLOR` is prioritized over `NO_COLOR`.
        _ if process.var("NO_COLOR").is_ok() => false,
        _ => process.stderr().is_a_tty(),
    };
    let maybe_rust_log_directives = process.var("RUST_LOG");
    let logger = tracing_subscriber::fmt::layer()
        .with_writer(move || process.stderr())
        .with_ansi(has_ansi);
    if let Ok(directives) = maybe_rust_log_directives {
        let env_filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .parse_lossy(directives);
        logger.compact().with_filter(env_filter).boxed()
    } else {
        // Receive log lines from Rustup only.
        let env_filter = EnvFilter::new("rustup=DEBUG");
        logger
            .event_format(EventFormatter)
            .with_filter(env_filter)
            .boxed()
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
            NotificationLevel::Debug => Some(Color::Blue),
            NotificationLevel::Verbose => Some(Color::Magenta),
            NotificationLevel::Info => None,
            NotificationLevel::Warn => Some(Color::Yellow),
            NotificationLevel::Error => Some(Color::Red),
        }
    }
}

/// A [`tracing::Subscriber`] [`Layer`][`tracing_subscriber::Layer`] that corresponds to Rustup's
/// optional `opentelemetry` (a.k.a. `otel`) feature.
#[cfg(feature = "otel")]
fn telemetry<S>(process: &Process) -> impl Layer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    let env_filter = if let Ok(directives) = process.var("RUST_LOG") {
        EnvFilter::builder()
            .with_default_directive(LevelFilter::TRACE.into())
            .parse_lossy(directives)
    } else {
        EnvFilter::new("rustup=TRACE")
    };
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
