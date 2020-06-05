use std::fmt;
use std::io::Write;
use term2::Terminal;

use super::term2;

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

pub fn warn_fmt(args: fmt::Arguments<'_>) {
    let mut t = term2::stderr();
    let _ = t.fg(term2::color::YELLOW);
    let _ = t.attr(term2::Attr::Bold);
    let _ = write!(t, "warning: ");
    let _ = t.reset();
    let _ = t.write_fmt(args);
    let _ = writeln!(t);
}

pub fn err_fmt(args: fmt::Arguments<'_>) {
    let mut t = term2::stderr();
    let _ = t.fg(term2::color::RED);
    let _ = t.attr(term2::Attr::Bold);
    let _ = write!(t, "error: ");
    let _ = t.reset();
    let _ = t.write_fmt(args);
    let _ = writeln!(t);
}

pub fn info_fmt(args: fmt::Arguments<'_>) {
    let mut t = term2::stderr();
    let _ = t.attr(term2::Attr::Bold);
    let _ = write!(t, "info: ");
    let _ = t.reset();
    let _ = t.write_fmt(args);
    let _ = writeln!(t);
}

pub fn verbose_fmt(args: fmt::Arguments<'_>) {
    let mut t = term2::stderr();
    let _ = t.fg(term2::color::MAGENTA);
    let _ = t.attr(term2::Attr::Bold);
    let _ = write!(t, "verbose: ");
    let _ = t.reset();
    let _ = t.write_fmt(args);
    let _ = writeln!(t);
}

pub fn debug_fmt(args: fmt::Arguments<'_>) {
    if std::env::var("RUSTUP_DEBUG").is_ok() {
        let mut t = term2::stderr();
        let _ = t.fg(term2::color::BLUE);
        let _ = t.attr(term2::Attr::Bold);
        let _ = write!(t, "debug: ");
        let _ = t.reset();
        let _ = t.write_fmt(args);
        let _ = writeln!(t);
    }
}
