use std::fmt;
use std::io::Write;

use super::term2;
use crate::process;

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
    let mut t = term2::stderr();
    let _ = t.fg(term2::Color::Yellow);
    let _ = t.attr(term2::Attr::Bold);
    let _ = write!(t, "warning: ");
    let _ = t.reset();
    let _ = t.write_fmt(args);
    let _ = writeln!(t);
}

pub(crate) fn err_fmt(args: fmt::Arguments<'_>) {
    let mut t = term2::stderr();
    let _ = t.fg(term2::Color::Red);
    let _ = t.attr(term2::Attr::Bold);
    let _ = write!(t, "error: ");
    let _ = t.reset();
    let _ = t.write_fmt(args);
    let _ = writeln!(t);
}

pub(crate) fn info_fmt(args: fmt::Arguments<'_>) {
    let mut t = term2::stderr();
    let _ = t.attr(term2::Attr::Bold);
    let _ = write!(t, "info: ");
    let _ = t.reset();
    let _ = t.write_fmt(args);
    let _ = writeln!(t);
}

pub(crate) fn verbose_fmt(args: fmt::Arguments<'_>) {
    let mut t = term2::stderr();
    let _ = t.fg(term2::Color::Magenta);
    let _ = t.attr(term2::Attr::Bold);
    let _ = write!(t, "verbose: ");
    let _ = t.reset();
    let _ = t.write_fmt(args);
    let _ = writeln!(t);
}

pub(crate) fn debug_fmt(args: fmt::Arguments<'_>) {
    if process().var("RUSTUP_DEBUG").is_ok() {
        let mut t = term2::stderr();
        let _ = t.fg(term2::Color::Blue);
        let _ = t.attr(term2::Attr::Bold);
        let _ = write!(t, "debug: ");
        let _ = t.reset();
        let _ = t.write_fmt(args);
        let _ = writeln!(t);
    }
}
