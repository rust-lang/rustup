use crate::term2;
use std::fmt;
use std::io::Write;

macro_rules! warn {
    ( $ ( $ arg : tt ) * ) => ( $crate::log::warn_fmt ( format_args ! ( $ ( $ arg ) * ) ) )
}
macro_rules! err {
    ( $ ( $ arg : tt ) * ) => ( $crate::log::err_fmt ( format_args ! ( $ ( $ arg ) * ) ) )
}
macro_rules! info {
    ( $ ( $ arg : tt ) * ) => ( $crate::log::info_fmt ( format_args ! ( $ ( $ arg ) * ) ) )
}

macro_rules! verbose {
    ( $ ( $ arg : tt ) * ) => ( $crate::log::verbose_fmt ( format_args ! ( $ ( $ arg ) * ) ) )
}

macro_rules! debug {
    ( $ ( $ arg : tt ) * ) => ( $crate::log::debug_fmt ( format_args ! ( $ ( $ arg ) * ) ) )
}

pub fn warn_fmt(args: fmt::Arguments<'_>) {
    let mut t = term2::stderr();
    let _ = t.fg(term2::color::BRIGHT_YELLOW);
    let _ = t.attr(term2::Attr::Bold);
    let _ = write!(t, "warning: ");
    let _ = t.reset();
    let _ = t.write_fmt(args);
    let _ = writeln!(t);
}

pub fn err_fmt(args: fmt::Arguments<'_>) {
    let mut t = term2::stderr();
    let _ = t.fg(term2::color::BRIGHT_RED);
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
    let _ = t.fg(term2::color::BRIGHT_MAGENTA);
    let _ = t.attr(term2::Attr::Bold);
    let _ = write!(t, "verbose: ");
    let _ = t.reset();
    let _ = t.write_fmt(args);
    let _ = writeln!(t);
}

pub fn debug_fmt(args: fmt::Arguments<'_>) {
    if std::env::var("RUSTUP_DEBUG").is_ok() {
        let mut t = term2::stderr();
        let _ = t.fg(term2::color::BRIGHT_BLUE);
        let _ = t.attr(term2::Attr::Bold);
        let _ = write!(t, "verbose: ");
        let _ = t.reset();
        let _ = t.write_fmt(args);
        let _ = writeln!(t);
    }
}
