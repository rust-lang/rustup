//! This provides wrappers around the StdoutTerminal and StderrTerminal types
//! that does not fail if StdoutTerminal etc can't be constructed, which happens
//! if TERM isn't defined.

use std::io;
use term;
use tty;

pub use term::color;

pub fn stdout() -> StdoutTerminal {
    StdoutTerminal(term::stdout())
}

pub fn stderr() -> StderrTerminal {
    StderrTerminal(term::stderr())
}

pub struct StdoutTerminal(Option<Box<term::StdoutTerminal>>);
pub struct StderrTerminal(Option<Box<term::StderrTerminal>>);

impl io::Write for StdoutTerminal {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        if let Some(ref mut t) = self.0 {
            t.write(buf)
        } else {
            let mut t = io::stdout();
            t.write(buf)
        }
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        if let Some(ref mut t) = self.0 {
            t.flush()
        } else {
            let mut t = io::stdout();
            t.flush()
        }
    }
}

impl io::Write for StderrTerminal {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        if let Some(ref mut t) = self.0 {
            t.write(buf)
        } else {
            let mut t = io::stderr();
            t.write(buf)
        }
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        if let Some(ref mut t) = self.0 {
            t.flush()
        } else {
            let mut t = io::stdout();
            t.flush()
        }
    }
}

impl StdoutTerminal {
    pub fn fg(&mut self, color: color::Color) -> Result<(), term::Error> {
        if !tty::stderr_isatty() { return Ok(()) }

        if let Some(ref mut t) = self.0 {
            t.fg(color)
        } else {
            Ok(())
        }
    }

    pub fn reset(&mut self) -> Result<(), term::Error> {
        if !tty::stderr_isatty() { return Ok(()) }

        if let Some(ref mut t) = self.0 {
            t.reset()
        } else {
            Ok(())
        }
    }
}

impl StderrTerminal {
    pub fn fg(&mut self, color: color::Color) -> Result<(), term::Error> {
        if !tty::stderr_isatty() { return Ok(()) }

        if let Some(ref mut t) = self.0 {
            t.fg(color)
        } else {
            Ok(())
        }
    }

    pub fn reset(&mut self) -> Result<(), term::Error> {
        if !tty::stderr_isatty() { return Ok(()) }

        if let Some(ref mut t) = self.0 {
            t.reset()
        } else {
            Ok(())
        }
    }
}

