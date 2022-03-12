use std::io::{self, BufRead, Cursor, Read, Result, Write};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::cli::term2::{AutomationFriendlyTerminal, Terminal};
use crate::utils::tty;

/// Stand-in for std::io::Stdin
pub trait Stdin {
    fn lock(&self) -> Box<dyn StdinLock + '_>;
    fn read_line(&self, buf: &mut String) -> Result<usize>;
}

/// Stand-in for std::io::StdinLock
pub trait StdinLock: Read + BufRead {}

/// Stand-in for std::io::stdin
pub trait StdinSource {
    fn stdin(&self) -> Box<dyn Stdin>;
}

// ----------------- OS support for stdin -----------------
impl StdinLock for io::StdinLock<'_> {}

impl Stdin for io::Stdin {
    fn lock(&self) -> Box<dyn StdinLock + '_> {
        Box::new(io::Stdin::lock(self))
    }
    fn read_line(&self, buf: &mut String) -> Result<usize> {
        io::Stdin::read_line(self, buf)
    }
}

impl StdinSource for super::OSProcess {
    fn stdin(&self) -> Box<dyn Stdin> {
        Box::new(io::stdin())
    }
}

// ----------------------- test support for stdin ------------------

struct TestStdinLock<'a> {
    inner: MutexGuard<'a, Cursor<String>>,
}

impl StdinLock for TestStdinLock<'_> {}

impl Read for TestStdinLock<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl BufRead for TestStdinLock<'_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.inner.fill_buf()
    }
    fn consume(&mut self, n: usize) {
        self.inner.consume(n)
    }
}

pub(crate) type TestStdinInner = Arc<Mutex<Cursor<String>>>;

struct TestStdin(TestStdinInner);

impl Stdin for TestStdin {
    fn lock(&self) -> Box<dyn StdinLock + '_> {
        Box::new(TestStdinLock {
            inner: self.0.lock().unwrap_or_else(|e| e.into_inner()),
        })
    }
    fn read_line(&self, buf: &mut String) -> Result<usize> {
        self.lock().read_line(buf)
    }
}

impl StdinSource for super::TestProcess {
    fn stdin(&self) -> Box<dyn Stdin> {
        Box::new(TestStdin(self.stdin.clone()))
    }
}

// -------------- stdout -------------------------------

pub trait Isatty {
    fn isatty(&self) -> bool;
}

/// Stand-in for std::io::StdoutLock
pub trait WriterLock: Write {}

/// Stand-in for std::io::Stdout
pub trait Writer: Write + Isatty + Send {
    fn lock(&self) -> Box<dyn WriterLock + '_>;
}

/// Stand-in for std::io::stdout
pub trait StdoutSource {
    fn stdout(&self) -> Box<dyn Terminal>;
}

// -------------- stderr -------------------------------

/// Stand-in for std::io::stderr
pub trait StderrSource {
    fn stderr(&self) -> Box<dyn Terminal>;
}

// ----------------- OS support for writers -----------------

impl WriterLock for io::StdoutLock<'_> {}

impl Writer for io::Stdout {
    fn lock(&self) -> Box<dyn WriterLock + '_> {
        Box::new(io::Stdout::lock(self))
    }
}

impl Isatty for io::Stdout {
    fn isatty(&self) -> bool {
        tty::stdout_isatty()
    }
}

impl StdoutSource for super::OSProcess {
    fn stdout(&self) -> Box<dyn Terminal> {
        Box::new(AutomationFriendlyTerminal::stdout())
    }
}

impl WriterLock for io::StderrLock<'_> {}

impl Writer for io::Stderr {
    fn lock(&self) -> Box<dyn WriterLock + '_> {
        Box::new(io::Stderr::lock(self))
    }
}

impl Isatty for io::Stderr {
    fn isatty(&self) -> bool {
        tty::stderr_isatty()
    }
}

impl StderrSource for super::OSProcess {
    fn stderr(&self) -> Box<dyn Terminal> {
        Box::new(AutomationFriendlyTerminal::stderr())
    }
}

// ----------------------- test support for writers ------------------

struct TestWriterLock<'a> {
    inner: MutexGuard<'a, Vec<u8>>,
}

impl WriterLock for TestWriterLock<'_> {}

impl Write for TestWriterLock<'_> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

pub(crate) type TestWriterInner = Arc<Mutex<Vec<u8>>>;

struct TestWriter(TestWriterInner);

impl Terminal for TestWriter {
    fn fg(&mut self, _: crate::cli::term2::Color) -> io::Result<()> {
        Ok(())
    }

    fn bg(&mut self, _: crate::cli::term2::Color) -> io::Result<()> {
        Ok(())
    }

    fn attr(&mut self, _: crate::cli::term2::Attr) -> io::Result<()> {
        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn carriage_return(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().write(b"\r").map(|_| ())
    }
}

impl Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Isatty for TestWriter {
    fn isatty(&self) -> bool {
        false
    }
}

impl StdoutSource for super::TestProcess {
    fn stdout(&self) -> Box<dyn Terminal> {
        Box::new(TestWriter(self.stdout.clone()))
    }
}

impl StderrSource for super::TestProcess {
    fn stderr(&self) -> Box<dyn Terminal> {
        Box::new(TestWriter(self.stderr.clone()))
    }
}
