use std::io::{self, BufRead, Read, Write};

use super::terminalsource::{ColorableTerminal, StreamSelector};
use crate::process::Process;

/// Stand-in for std::io::Stdin
pub trait Stdin {
    fn lock(&self) -> Box<dyn StdinLock + '_>;
}

/// Stand-in for std::io::StdinLock
pub trait StdinLock: Read + BufRead {}

// ----------------- OS support for stdin -----------------

impl StdinLock for io::StdinLock<'_> {}

impl Stdin for io::Stdin {
    fn lock(&self) -> Box<dyn StdinLock + '_> {
        Box::new(io::Stdin::lock(self))
    }
}

// -------------- stdout -------------------------------

/// This is a stand-in for [`std::io::StdoutLock`] and [`std::io::StderrLock`].
pub trait WriterLock: Write {}

/// This is a stand-in for [`std::io::Stdout`] or [`std::io::Stderr`].
/// TODO: remove Sync.
pub trait Writer: Write + Send + Sync {
    /// This is a stand-in for [`std::io::Stdout::lock`] or [`std::io::Stderr::lock`].
    fn lock(&self) -> Box<dyn WriterLock + '_>;

    /// Query whether a TTY is present. Used in download_tracker - we may want
    /// to remove this entirely with a better progress bar system (in favour of
    /// filtering in the Terminal layer?)
    fn is_a_tty(&self, process: &Process) -> bool;

    /// Construct a terminal on this writer.
    fn terminal(&self, process: &Process) -> ColorableTerminal;
}

// ----------------- OS support for writers -----------------

impl WriterLock for io::StdoutLock<'_> {}

impl Writer for io::Stdout {
    fn is_a_tty(&self, process: &Process) -> bool {
        match process {
            crate::process::Process::OsProcess(p) => p.stdout_is_a_tty,
            #[cfg(feature = "test")]
            crate::process::Process::TestProcess(_) => unreachable!(),
        }
    }

    fn lock(&self) -> Box<dyn WriterLock + '_> {
        Box::new(io::Stdout::lock(self))
    }

    fn terminal(&self, process: &Process) -> ColorableTerminal {
        ColorableTerminal::new(StreamSelector::Stdout, process)
    }
}

impl WriterLock for io::StderrLock<'_> {}

impl Writer for io::Stderr {
    fn is_a_tty(&self, process: &Process) -> bool {
        match process {
            crate::process::Process::OsProcess(p) => p.stderr_is_a_tty,
            #[cfg(feature = "test")]
            crate::process::Process::TestProcess(_) => unreachable!(),
        }
    }

    fn lock(&self) -> Box<dyn WriterLock + '_> {
        Box::new(io::Stderr::lock(self))
    }

    fn terminal(&self, process: &Process) -> ColorableTerminal {
        ColorableTerminal::new(StreamSelector::Stderr, process)
    }
}

#[cfg(feature = "test")]
pub(crate) use self::test_support::*;

#[cfg(feature = "test")]
mod test_support {
    use std::{
        io::Cursor,
        sync::{Arc, Mutex, MutexGuard},
    };

    use super::*;

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

    pub struct TestStdin(pub(in super::super) TestStdinInner);

    impl Stdin for TestStdin {
        fn lock(&self) -> Box<dyn StdinLock + '_> {
            Box::new(TestStdinLock {
                inner: self.0.lock().unwrap_or_else(|e| e.into_inner()),
            })
        }
    }

    // ----------------------- test support for writers ------------------

    pub(in super::super) struct TestWriterLock<'a> {
        inner: MutexGuard<'a, Vec<u8>>,
    }

    impl WriterLock for TestWriterLock<'_> {}

    impl Write for TestWriterLock<'_> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.inner.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    pub(in super::super) type TestWriterInner = Arc<Mutex<Vec<u8>>>;

    /// A thread-safe test file handle that pretends to be e.g. stdout.
    #[derive(Clone, Default)]
    pub(in super::super) struct TestWriter(pub(in super::super) TestWriterInner);

    impl TestWriter {
        pub(in super::super) fn lock(&self) -> TestWriterLock<'_> {
            // The stream can be locked even if a test thread panicked: its state
            // will be ok
            TestWriterLock {
                inner: self.0.lock().unwrap_or_else(|e| e.into_inner()),
            }
        }
    }

    impl Writer for TestWriter {
        fn is_a_tty(&self, _: &Process) -> bool {
            false
        }

        fn lock(&self) -> Box<dyn WriterLock + '_> {
            Box::new(self.lock())
        }

        fn terminal(&self, process: &Process) -> ColorableTerminal {
            ColorableTerminal::new(StreamSelector::TestWriter(self.clone()), process)
        }
    }

    impl Write for TestWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.lock().write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
