use std::fmt;
use std::io::Write;

use crate::currentprocess::{
    filesource::StderrSource, process, terminalsource, varsource::VarSource,
};

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
