use std::{
    io::{self, Write},
    num::NonZero,
};

#[cfg(feature = "test")]
use anstream::StripStream;
use anstream::{AutoStream, ColorChoice};
use console::Term;
use indicatif::TermLike;
use tracing_subscriber::fmt::MakeWriter;

use super::Process;
#[cfg(feature = "test")]
use super::file_source::TestWriter;

/// A colorable terminal that can be written to
pub struct ColorableTerminal {
    inner: TerminalInner,
    is_a_tty: bool,
    color_choice: ColorChoice,
    width: Option<NonZero<u16>>,
}

impl ColorableTerminal {
    pub(super) fn stdout(process: &Process) -> Self {
        let is_a_tty = match process {
            Process::OsProcess(p) => p.stdout_is_a_tty,
            #[cfg(feature = "test")]
            Process::TestProcess(_) => unreachable!(),
        };

        Self::new(StreamSelector::Stdout, is_a_tty, process)
    }

    pub(super) fn stderr(process: &Process) -> Self {
        let is_a_tty = match process {
            Process::OsProcess(p) => p.stderr_is_a_tty,
            #[cfg(feature = "test")]
            Process::TestProcess(_) => unreachable!(),
        };

        Self::new(StreamSelector::Stderr, is_a_tty, process)
    }

    #[cfg(feature = "test")]
    pub(super) fn test(writer: TestWriter, process: &Process) -> Self {
        Self::new(StreamSelector::TestWriter(writer), false, process)
    }

    /// A terminal that supports colorisation of a stream.
    /// If `RUSTUP_TERM_COLOR` is set to `always`, or if the stream is a tty and
    /// `RUSTUP_TERM_COLOR` either unset or set to `auto`,
    /// then color commands will be sent to the stream.
    /// Otherwise color commands are discarded.
    fn new(stream: StreamSelector, is_a_tty: bool, process: &Process) -> Self {
        let choice = process.color_choice(is_a_tty);
        let inner = match stream {
            StreamSelector::Stdout => TerminalInner::Stdout(AutoStream::new(io::stdout(), choice)),
            StreamSelector::Stderr => TerminalInner::Stderr(AutoStream::new(io::stderr(), choice)),
            #[cfg(feature = "test")]
            StreamSelector::TestWriter(w) => TerminalInner::TestWriter(w),
        };
        let width = process
            .var("RUSTUP_TERM_WIDTH")
            .ok()
            .and_then(|s| s.parse::<NonZero<u16>>().ok());
        ColorableTerminal {
            inner,
            is_a_tty,
            color_choice: choice,
            width,
        }
    }

    pub fn lock(&self) -> ColorableTerminalLocked {
        match &self.inner {
            TerminalInner::Stdout(s) => ColorableTerminalLocked::Stdout(AutoStream::new(
                s.as_inner().lock(),
                self.color_choice,
            )),
            TerminalInner::Stderr(s) => ColorableTerminalLocked::Stderr(AutoStream::new(
                s.as_inner().lock(),
                self.color_choice,
            )),
            #[cfg(feature = "test")]
            TerminalInner::TestWriter(w) => {
                ColorableTerminalLocked::TestWriter(StripStream::new(Box::new(w.clone())))
            }
        }
    }

    pub fn is_a_tty(&self) -> bool {
        self.is_a_tty
    }

    pub fn color_choice(&self) -> ColorChoice {
        self.color_choice
    }
}

impl TermLike for ColorableTerminal {
    fn width(&self) -> u16 {
        match self.width {
            Some(n) => n.get(),
            None => Term::stdout().size().1,
        }
    }

    fn move_cursor_up(&self, n: usize) -> io::Result<()> {
        // As the ProgressBar may try to move the cursor up by 0 lines,
        // we need to handle that case to avoid writing an escape sequence
        // that would mess up the terminal.
        if n == 0 {
            return Ok(());
        }
        let mut t = self.lock();
        write!(t, "\x1b[{n}A")?;
        t.flush()
    }

    fn move_cursor_down(&self, n: usize) -> io::Result<()> {
        if n == 0 {
            return Ok(());
        }
        let mut t = self.lock();
        write!(t, "\x1b[{n}B")?;
        t.flush()
    }

    fn move_cursor_right(&self, n: usize) -> io::Result<()> {
        if n == 0 {
            return Ok(());
        }
        let mut t = self.lock();
        write!(t, "\x1b[{n}C")?;
        t.flush()
    }

    fn move_cursor_left(&self, n: usize) -> io::Result<()> {
        if n == 0 {
            return Ok(());
        }
        let mut t = self.lock();
        write!(t, "\x1b[{n}D")?;
        t.flush()
    }

    fn write_line(&self, line: &str) -> io::Result<()> {
        let mut t = self.lock();
        t.write_all(line.as_bytes())?;
        t.write_all(b"\n")?;
        t.flush()
    }

    fn write_str(&self, s: &str) -> io::Result<()> {
        let mut t = self.lock();
        t.write_all(s.as_bytes())?;
        t.flush()
    }

    fn clear_line(&self) -> io::Result<()> {
        let mut t = self.lock();
        t.write_all(b"\r\x1b[2K")?;
        t.flush()
    }

    fn flush(&self) -> io::Result<()> {
        let mut t = self.lock();
        t.flush()
    }
}

impl<'a> MakeWriter<'a> for ColorableTerminal {
    type Writer = ColorableTerminalLocked;

    fn make_writer(&self) -> Self::Writer {
        self.lock()
    }
}

impl std::fmt::Debug for ColorableTerminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ColorableTerminal {{ inner: ... }}")
    }
}

pub enum ColorableTerminalLocked {
    Stdout(AutoStream<io::StdoutLock<'static>>),
    Stderr(AutoStream<io::StderrLock<'static>>),
    #[cfg(feature = "test")]
    TestWriter(StripStream<Box<dyn Write>>),
}

impl ColorableTerminalLocked {
    fn as_write(&mut self) -> &mut dyn Write {
        match self {
            Self::Stdout(s) => s,
            Self::Stderr(s) => s,
            #[cfg(feature = "test")]
            Self::TestWriter(w) => w,
        }
    }
}

impl Write for ColorableTerminalLocked {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.as_write().write(buf)
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.as_write().write_vectored(bufs)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.as_write().flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.as_write().write_all(buf)
    }

    fn write_fmt(&mut self, args: std::fmt::Arguments<'_>) -> io::Result<()> {
        self.as_write().write_fmt(args)
    }
}

/// Internal state for ColorableTerminal
enum TerminalInner {
    Stdout(AutoStream<io::Stdout>),
    Stderr(AutoStream<io::Stderr>),
    #[cfg(feature = "test")]
    TestWriter(TestWriter),
}

/// Select what stream to make a terminal on
pub(super) enum StreamSelector {
    Stdout,
    Stderr,
    #[cfg(feature = "test")]
    TestWriter(TestWriter),
}
