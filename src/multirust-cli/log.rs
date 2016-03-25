use term2;
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

pub fn warn_fmt(args: fmt::Arguments) {
    let mut t = term2::stderr();
    let _ = t.fg(term2::color::BRIGHT_YELLOW);
    let _ = write!(t, "warning");
    let _ = t.reset();
    let _ = write!(t, ": ");
    let _ = t.write_fmt(args);
    let _ = write!(t, "\n");
}

pub fn err_fmt(args: fmt::Arguments) {
    let mut t = term2::stderr();
    let _ = t.fg(term2::color::BRIGHT_RED);
    let _ = write!(t, "error");
    let _ = t.reset();
    let _ = write!(t, ": ");
    let _ = t.write_fmt(args);
    let _ = write!(t, "\n");
}

pub fn info_fmt(args: fmt::Arguments) {
    let mut t = term2::stderr();
    let _ = t.fg(term2::color::BRIGHT_CYAN);
    let _ = write!(t, "info");
    let _ = t.reset();
    let _ = write!(t, ": ");
    let _ = t.write_fmt(args);
    let _ = write!(t, "\n");
}

pub fn verbose_fmt(args: fmt::Arguments) {
    let mut t = term2::stderr();
    let _ = t.fg(term2::color::BRIGHT_MAGENTA);
    let _ = write!(t, "verbose");
    let _ = t.reset();
    let _ = write!(t, ": ");
    let _ = t.write_fmt(args);
    let _ = write!(t, "\n");
}
