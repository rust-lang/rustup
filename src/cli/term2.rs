//! This provides wrappers around the `StdoutTerminal` and `StderrTerminal` types
//! that does not fail if `StdoutTerminal` etc can't be constructed, which happens
//! if TERM isn't defined.

use std::io;
use std::io::Write;

use termcolor::{ColorChoice, ColorSpec, StandardStream, WriteColor};

pub(crate) trait Terminal: io::Write {
    fn fg(&mut self, color: Color) -> io::Result<()>;
    fn bg(&mut self, color: Color) -> io::Result<()>;
    fn attr(&mut self, attr: Attr) -> io::Result<()>;
    fn reset(&mut self) -> io::Result<()>;
    fn carriage_return(&mut self) -> io::Result<()>;
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum Color {
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
}

impl From<Color> for termcolor::Color {
    fn from(color: Color) -> termcolor::Color {
        match color {
            Color::Red => termcolor::Color::Red,
            Color::Green => termcolor::Color::Green,
            Color::Yellow => termcolor::Color::Yellow,
            Color::Blue => termcolor::Color::Blue,
            Color::Magenta => termcolor::Color::Magenta,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum Attr {
    Bold,
    ForegroundColor(Color),
}

use crate::currentprocess::filesource::Isatty;

// Decorator to:
// - Disable all terminal controls on non-tty's
// - Swallow errors when we try to use features a terminal doesn't have
//   such as setting colours when no TermInfo DB is present
pub(crate) struct AutomationFriendlyTerminal {
    stream: StandardStream,
    color: ColorSpec,
}

pub(crate) type StdoutTerminal = AutomationFriendlyTerminal;
pub(crate) type StderrTerminal = AutomationFriendlyTerminal;

impl Isatty for AutomationFriendlyTerminal {
    fn isatty(&self) -> bool {
        self.stream.supports_color()
    }
}

impl Terminal for AutomationFriendlyTerminal {
    fn fg(&mut self, color: Color) -> io::Result<()> {
        if !self.isatty() {
            return Ok(());
        }
        self.color.set_fg(Some(color.into()));
        self.stream.set_color(&self.color)
    }

    fn bg(&mut self, color: Color) -> io::Result<()> {
        if !self.isatty() {
            return Ok(());
        }
        self.color.set_bg(Some(color.into()));
        self.stream.set_color(&self.color)
    }

    fn attr(&mut self, attr: Attr) -> io::Result<()> {
        if !self.isatty() {
            return Ok(());
        }
        match attr {
            Attr::Bold => self.color.set_bold(true),
            Attr::ForegroundColor(color) => self.color.set_fg(Some(color.into())),
        };
        self.stream.set_color(&self.color)
    }

    fn reset(&mut self) -> io::Result<()> {
        if !self.isatty() {
            return Ok(());
        }
        self.stream.reset()
    }

    fn carriage_return(&mut self) -> io::Result<()> {
        self.stream.write(b"\r").map(|_| ())
    }
}

impl io::Write for AutomationFriendlyTerminal {
    fn write(&mut self, buf: &[u8]) -> std::result::Result<usize, io::Error> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> std::result::Result<(), io::Error> {
        self.stream.flush()
    }
}

pub(crate) fn stdout() -> StdoutTerminal {
    let choice = if crate::utils::tty::stdout_isatty() {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };
    AutomationFriendlyTerminal {
        stream: termcolor::StandardStream::stdout(choice),
        color: ColorSpec::new(),
    }
}

pub(crate) fn stderr() -> StderrTerminal {
    let choice = if crate::utils::tty::stderr_isatty() {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };
    AutomationFriendlyTerminal {
        stream: termcolor::StandardStream::stderr(choice),
        color: ColorSpec::new(),
    }
}
