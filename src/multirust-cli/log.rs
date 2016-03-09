use term;
use tty;
use std::fmt;

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
    let mut t = term::stderr().unwrap();
    if tty::stderr_isatty() { let _ = t.fg(term::color::BRIGHT_YELLOW); }
    let _ = write!(t, "warning: ");
    if tty::stderr_isatty() { let _ = t.reset(); }
    let _ = t.write_fmt(args);
    let _ = write!(t, "\n");
}

pub fn err_fmt(args: fmt::Arguments) {
    let mut t = term::stderr().unwrap();
    if tty::stderr_isatty() { let _ = t.fg(term::color::BRIGHT_RED); }
    let _ = write!(t, "error: ");
    if tty::stderr_isatty() { let _ = t.reset(); }
    let _ = t.write_fmt(args);
    let _ = write!(t, "\n");
}

pub fn info_fmt(args: fmt::Arguments) {
    let mut t = term::stderr().unwrap();
    if tty::stderr_isatty() { let _ = t.fg(term::color::BRIGHT_GREEN); }
    let _ = write!(t, "info: ");
    if tty::stderr_isatty() { let _ = t.reset(); }
    let _ = t.write_fmt(args);
    let _ = write!(t, "\n");
}

pub fn verbose_fmt(args: fmt::Arguments) {
    let mut t = term::stderr().unwrap();
    if tty::stderr_isatty() { let _ = t.fg(term::color::BRIGHT_MAGENTA); }
    let _ = write!(t, "verbose: ");
    if tty::stderr_isatty() { let _ = t.reset(); }
    let _ = t.write_fmt(args);
    let _ = write!(t, "\n");
}
