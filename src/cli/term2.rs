//! This provides wrappers around the `StdoutTerminal` and `StderrTerminal` types
//! that does not fail if `StdoutTerminal` etc can't be constructed, which happens
//! if TERM isn't defined.

use std::io;
use std::io::Write;
use std::sync::{Arc, Mutex};

use once_cell::sync::OnceCell;

use termcolor::{ColorChoice, ColorSpec, StandardStream, WriteColor};

use crate::process;

/// Public via Terminal
#[derive(Copy, Clone, Debug)]
pub enum Color {
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
}

/// Public via Terminal
#[derive(Copy, Clone, Debug)]
pub enum Attr {
    Bold,
    ForegroundColor(Color),
}

/// Public via currentprocess::filesource
pub trait Terminal: io::Write {
    fn fg(&mut self, color: Color) -> io::Result<()>;
    fn bg(&mut self, color: Color) -> io::Result<()>;
    fn attr(&mut self, attr: Attr) -> io::Result<()>;
    fn reset(&mut self) -> io::Result<()>;
    fn carriage_return(&mut self) -> io::Result<()>;
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

use crate::currentprocess::filesource::Isatty;

/// Disable all terminal controls on non-tty's
pub(crate) struct AutomationFriendlyTerminal {
    stream: StandardStream,
    color: ColorSpec,
}

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

impl AutomationFriendlyTerminal {
    pub(crate) fn stdout() -> AutomationFriendlyTerminal {
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

    pub(crate) fn stderr() -> AutomationFriendlyTerminal {
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
}

/// A cheaply clonable handle to a terminal.
#[derive(Clone)]
struct SharedTerminal {
    inner: Arc<Mutex<Box<dyn Terminal + Send + Sync>>>,
}

impl io::Write for SharedTerminal {
    fn write(&mut self, buf: &[u8]) -> std::result::Result<usize, io::Error> {
        self.inner.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> std::result::Result<(), io::Error> {
        self.inner.lock().unwrap().flush()
    }
}

impl Terminal for SharedTerminal {
    fn fg(&mut self, color: Color) -> io::Result<()> {
        self.inner.lock().unwrap().fg(color)
    }

    fn bg(&mut self, color: Color) -> io::Result<()> {
        self.inner.lock().unwrap().bg(color)
    }

    fn attr(&mut self, attr: Attr) -> io::Result<()> {
        self.inner.lock().unwrap().attr(attr)
    }

    fn reset(&mut self) -> io::Result<()> {
        self.inner.lock().unwrap().reset()
    }

    fn carriage_return(&mut self) -> io::Result<()> {
        self.inner.lock().unwrap().carriage_return()
    }
}

pub(crate) fn stdout() -> Box<dyn Terminal> {
    static STDOUT: OnceCell<Box<SharedTerminal>> = OnceCell::new();
    let shared = STDOUT.get_or_init(|| {
        Box::new(SharedTerminal {
            inner: Arc::new(Mutex::new(process().stdout())),
        })
    });
    Box::<SharedTerminal>::clone(shared)
}

pub(crate) fn stderr() -> Box<dyn Terminal> {
    static STDERR: OnceCell<Box<SharedTerminal>> = OnceCell::new();
    let shared = STDERR.get_or_init(|| {
        Box::new(SharedTerminal {
            inner: Arc::new(Mutex::new(process().stderr())),
        })
    });
    Box::<SharedTerminal>::clone(shared)
}
