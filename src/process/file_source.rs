use std::io::{self, BufRead, Read};

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

#[cfg(feature = "test")]
pub(crate) use self::test_support::*;

#[cfg(feature = "test")]
mod test_support {
    use std::{
        io::{Cursor, Write},
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

    pub(in super::super) type TestWriterInner = Arc<Mutex<Vec<u8>>>;

    /// A thread-safe test file handle that pretends to be e.g. stdout.
    #[derive(Clone, Default)]
    pub(in super::super) struct TestWriter(pub(in super::super) TestWriterInner);

    impl Write for TestWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            // The stream can be locked even if a test thread panicked: its state will be ok
            self.0.lock().unwrap_or_else(|e| e.into_inner()).write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
