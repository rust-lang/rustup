//! The main Rustup command-line interface
//!
//! The rustup binary is a chimera, changing its behavior based on the
//! name of the binary. This is used most prominently to enable
//! Rustup's tool 'proxies' - that is, rustup itself and the rustup
//! proxies are the same binary: when the binary is called 'rustup' or
//! 'rustup.exe' it offers the Rustup command-line interface, and
//! when it is called 'rustc' it behaves as a proxy to 'rustc'.
//!
//! This scheme is further used to distinguish the Rustup installer,
//! called 'rustup-init', which is again just the rustup binary under a
//! different name.

#![recursion_limit = "1024"]

use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use cfg_if::cfg_if;
// Public macros require availability of the internal symbols
use rs_tracing::{
    close_trace_file, close_trace_file_internal, open_trace_file, trace_to_file_internal,
};
use tracing_subscriber::{reload::Handle, EnvFilter, Registry};

use rustup::cli::common;
use rustup::cli::proxy_mode;
use rustup::cli::rustup_mode;
#[cfg(windows)]
use rustup::cli::self_update;
use rustup::cli::setup_mode;
use rustup::env_var::RUST_RECURSION_COUNT_MAX;
use rustup::errors::RustupError;
use rustup::is_proxyable_tools;
use rustup::process::Process;
use rustup::utils::utils;

#[tokio::main]
async fn main() -> Result<ExitCode> {
    #[cfg(windows)]
    pre_rustup_main_init();

    let process = Process::os();
    #[cfg(feature = "otel")]
    opentelemetry::global::set_text_map_propagator(
        opentelemetry_sdk::propagation::TraceContextPropagator::new(),
    );
    let (subscriber, console_filter) = rustup::cli::log::tracing_subscriber(&process);
    tracing::subscriber::set_global_default(subscriber)?;
    let result = run_rustup(&process, console_filter).await;
    // We're tracing, so block until all spans are exported.
    #[cfg(feature = "otel")]
    opentelemetry::global::shutdown_tracer_provider();

    match result {
        Err(e) => {
            common::report_error(&e, &process);
            std::process::exit(1)
        }
        Ok(utils::ExitCode(c)) => std::process::exit(c),
    }
}

#[tracing::instrument(level = "trace")]
async fn run_rustup(
    process: &Process,
    console_filter: Handle<EnvFilter, Registry>,
) -> Result<utils::ExitCode> {
    if let Ok(dir) = process.var("RUSTUP_TRACE_DIR") {
        open_trace_file!(dir)?;
    }
    let result = run_rustup_inner(process, console_filter).await;
    if process.var("RUSTUP_TRACE_DIR").is_ok() {
        close_trace_file!();
    }
    result
}

#[tracing::instrument(level = "trace", err(level = "trace"))]
async fn run_rustup_inner(
    process: &Process,
    console_filter: Handle<EnvFilter, Registry>,
) -> Result<utils::ExitCode> {
    // Guard against infinite proxy recursion. This mostly happens due to
    // bugs in rustup.
    do_recursion_guard(process)?;

    // Before we do anything else, ensure we know where we are and who we
    // are because otherwise we cannot proceed usefully.
    let current_dir = process
        .current_dir()
        .context(RustupError::LocatingWorkingDir)?;
    utils::current_exe()?;

    match process.name().as_deref() {
        Some("rustup") => rustup_mode::main(current_dir, process).await,
        Some(n) if n.starts_with("rustup-setup") || n.starts_with("rustup-init") => {
            // NB: The above check is only for the prefix of the file
            // name. Browsers rename duplicates to
            // e.g. rustup-setup(2), and this allows all variations
            // to work.
            setup_mode::main(current_dir, process, console_filter).await
        }
        Some(n) if n.starts_with("rustup-gc-") => {
            // This is the final uninstallation stage on windows where
            // rustup deletes its own exe
            cfg_if! {
                if #[cfg(windows)] {
                    self_update::complete_windows_uninstall(process)
                } else {
                    unreachable!("Attempted to use Windows-specific code on a non-Windows platform. Aborting.")
                }
            }
        }
        Some(n) => {
            is_proxyable_tools(n)?;
            proxy_mode::main(n, current_dir, process)
                .await
                .map(utils::ExitCode::from)
        }
        None => {
            // Weird case. No arg0, or it's unparsable.
            Err(rustup::cli::errors::CLIError::NoExeName.into())
        }
    }
}

fn do_recursion_guard(process: &Process) -> Result<()> {
    let recursion_count = process
        .var("RUST_RECURSION_COUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if recursion_count > RUST_RECURSION_COUNT_MAX {
        return Err(anyhow!("infinite recursion detected"));
    }

    Ok(())
}

/// Windows pre-main security mitigations.
///
/// This attempts to defend against malicious DLLs that may sit alongside
/// rustup-init in the user's download folder.
#[cfg(windows)]
pub fn pre_rustup_main_init() {
    use windows_sys::Win32::System::LibraryLoader::{
        SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_SYSTEM32,
    };
    // Default to loading delay loaded DLLs from the system directory.
    // For DLLs loaded at load time, this relies on the `delayload` linker flag.
    // This is only necessary prior to Windows 10 RS1. See build.rs for details.
    unsafe {
        let result = SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32);
        // SetDefaultDllDirectories should never fail if given valid arguments.
        // But just to be safe and to catch mistakes, assert that it succeeded.
        assert_ne!(result, 0);
    }
}
