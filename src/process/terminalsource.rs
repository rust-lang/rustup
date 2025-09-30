use console::Term;
use indicatif::TermLike;
use std::{
    io::{self, Write},
    mem::MaybeUninit,
    num::NonZero,
    ops::DerefMut,
    ptr::addr_of_mut,
    sync::{Arc, Mutex, MutexGuard},
};

pub(crate) use termcolor::{Color, ColorChoice};
use termcolor::{ColorSpec, StandardStream, StandardStreamLock, WriteColor};

use super::Process;
#[cfg(feature = "test")]
use super::filesource::{TestWriter, TestWriterLock};

/// Select what stream to make a terminal on
pub(super) enum StreamSelector {
    Stdout,
    Stderr,
    #[cfg(feature = "test")]
    TestWriter(TestWriter),
    #[cfg(all(test, feature = "test"))]
    TestTtyWriter(TestWriter),
}

impl StreamSelector {
    fn is_a_tty(&self, process: &Process) -> bool {
        match self {
            StreamSelector::Stdout => match process {
                Process::OsProcess(p) => p.stdout_is_a_tty,
                #[cfg(feature = "test")]
                Process::TestProcess(_) => unreachable!(),
            },
            StreamSelector::Stderr => match process {
                Process::OsProcess(p) => p.stderr_is_a_tty,
                #[cfg(feature = "test")]
                Process::TestProcess(_) => unreachable!(),
            },
            #[cfg(feature = "test")]
            StreamSelector::TestWriter(_) => false,
            #[cfg(all(test, feature = "test"))]
            StreamSelector::TestTtyWriter(_) => true,
        }
    }
}

/// A colorable terminal that can be written to
pub struct ColorableTerminal {
    // TermColor uses a lifetime on locked variants, but the API we want to
    // emulate from std::io uses a static lifetime for locked variants: so we
    // emulate it. For Test workloads this results in a double-layering of
    // Arc<Mutex<...> which isn't great, but OTOH it is test code. Locking the
    // source is important because otherwise parallel constructed terminals
    // would not be locked out.
    inner: Arc<Mutex<TerminalInner>>,
    is_a_tty: bool,
    color_choice: ColorChoice,
    width: Option<NonZero<u16>>,
}

/// Internal state for ColorableTerminal
enum TerminalInner {
    StandardStream(StandardStream, ColorSpec),
    #[cfg(feature = "test")]
    #[allow(dead_code)] // ColorChoice only read in test code
    TestWriter(TestWriter, ColorChoice),
}

pub struct ColorableTerminalLocked {
    // Must drop the lock before the guard, as the guard borrows from inner.
    locked: TerminalInnerLocked,
    // must drop the guard before inner as the guard borrows from  inner.
    guard: MutexGuard<'static, TerminalInner>,
    inner: Arc<Mutex<TerminalInner>>,
}

enum TerminalInnerLocked {
    StandardStream(StandardStreamLock<'static>),
    #[cfg(feature = "test")]
    TestWriter(TestWriterLock<'static>),
}

impl ColorableTerminal {
    /// A terminal that supports colorisation of a stream.
    /// If `RUSTUP_TERM_COLOR` is set to `always`, or if the stream is a tty and
    /// `RUSTUP_TERM_COLOR` either unset or set to `auto`,
    /// then color commands will be sent to the stream.
    /// Otherwise color commands are discarded.
    pub(super) fn new(stream: StreamSelector, process: &Process) -> Self {
        let is_a_tty = stream.is_a_tty(process);
        let choice = match process.var("RUSTUP_TERM_COLOR") {
            Ok(s) if s.eq_ignore_ascii_case("always") => ColorChoice::Always,
            Ok(s) if s.eq_ignore_ascii_case("never") => ColorChoice::Never,
            _ if is_a_tty => ColorChoice::Auto,
            _ => ColorChoice::Never,
        };
        let inner = match stream {
            StreamSelector::Stdout => {
                TerminalInner::StandardStream(StandardStream::stdout(choice), ColorSpec::new())
            }
            StreamSelector::Stderr => {
                TerminalInner::StandardStream(StandardStream::stderr(choice), ColorSpec::new())
            }
            #[cfg(feature = "test")]
            StreamSelector::TestWriter(w) => TerminalInner::TestWriter(w, choice),
            #[cfg(all(test, feature = "test"))]
            StreamSelector::TestTtyWriter(w) => TerminalInner::TestWriter(w, choice),
        };
        let width = process
            .var("RUSTUP_TERM_WIDTH")
            .ok()
            .and_then(|s| s.parse::<NonZero<u16>>().ok());
        ColorableTerminal {
            inner: Arc::new(Mutex::new(inner)),
            is_a_tty,
            color_choice: choice,
            width,
        }
    }

    pub fn lock(&self) -> ColorableTerminalLocked {
        let mut uninit = MaybeUninit::<ColorableTerminalLocked>::uninit();
        let ptr = uninit.as_mut_ptr();

        // Safety: panics during this will leak an arc reference, or an arc
        // reference and a mutex guard, or an arc reference, mutex guard and a
        // stream lock. Drop proceeds in field order after initialization,
        // so the stream lock is dropped before the mutex guard, which is dropped
        // before the arc<mutex>.
        unsafe {
            // let inner: Arc<Mutex<TerminalInner>> = self.inner.clone();
            addr_of_mut!((*ptr).inner).write(self.inner.clone());
            // let guard = inner.lock().unwrap();
            addr_of_mut!((*ptr).guard).write((*ptr).inner.lock().unwrap());
            // let locked = match *guard {....}
            addr_of_mut!((*ptr).locked).write(match (*ptr).guard.deref_mut() {
                TerminalInner::StandardStream(s, _) => {
                    let locked = s.lock();
                    TerminalInnerLocked::StandardStream(locked)
                }
                #[cfg(feature = "test")]
                TerminalInner::TestWriter(w, _) => TerminalInnerLocked::TestWriter(w.lock()),
            });
            // ColorableTerminalLocked { inner, guard, locked }
            uninit.assume_init()
        }
    }

    pub fn fg(&mut self, color: Color) -> io::Result<()> {
        match self.inner.lock().unwrap().deref_mut() {
            TerminalInner::StandardStream(s, spec) => {
                spec.set_fg(Some(color));
                s.set_color(spec)
            }
            #[cfg(feature = "test")]
            TerminalInner::TestWriter(_, _) => Ok(()),
        }
    }

    pub fn attr(&mut self, attr: Attr) -> io::Result<()> {
        match self.inner.lock().unwrap().deref_mut() {
            TerminalInner::StandardStream(s, spec) => {
                match attr {
                    Attr::Bold => spec.set_bold(true),
                    Attr::ForegroundColor(color) => spec.set_fg(Some(color)),
                };
                s.set_color(spec)
            }
            #[cfg(feature = "test")]
            TerminalInner::TestWriter(_, _) => Ok(()),
        }
    }

    pub fn reset(&mut self) -> io::Result<()> {
        match self.inner.lock().unwrap().deref_mut() {
            TerminalInner::StandardStream(s, color) => {
                color.clear();
                s.reset()
            }
            #[cfg(feature = "test")]
            TerminalInner::TestWriter(_, _) => Ok(()),
        }
    }

    pub fn is_a_tty(&self) -> bool {
        self.is_a_tty
    }

    pub fn color_choice(&self) -> ColorChoice {
        self.color_choice
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Attr {
    Bold,
    ForegroundColor(Color),
}

impl io::Write for ColorableTerminal {
    fn write(&mut self, buf: &[u8]) -> std::result::Result<usize, io::Error> {
        match self.inner.lock().unwrap().deref_mut() {
            TerminalInner::StandardStream(s, _) => s.write(buf),
            #[cfg(feature = "test")]
            TerminalInner::TestWriter(w, _) => w.write(buf),
        }
    }

    fn flush(&mut self) -> std::result::Result<(), io::Error> {
        match self.inner.lock().unwrap().deref_mut() {
            TerminalInner::StandardStream(s, _) => s.flush(),
            #[cfg(feature = "test")]
            TerminalInner::TestWriter(w, _) => w.flush(),
        }
    }
}

impl io::Write for ColorableTerminalLocked {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match &mut self.locked {
            TerminalInnerLocked::StandardStream(s) => s.write(buf),
            #[cfg(feature = "test")]
            TerminalInnerLocked::TestWriter(w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.locked {
            TerminalInnerLocked::StandardStream(s) => s.flush(),
            #[cfg(feature = "test")]
            TerminalInnerLocked::TestWriter(w) => w.flush(),
        }
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

impl std::fmt::Debug for ColorableTerminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ColorableTerminal {{ inner: ... }}")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::process::TestProcess;
    use crate::test::Env;

    #[test]
    fn term_color_choice() {
        fn assert_color_choice(env_val: &str, stream: StreamSelector, color_choice: ColorChoice) {
            let mut vars = HashMap::new();
            vars.env("RUSTUP_TERM_COLOR", env_val);
            let tp = TestProcess::with_vars(vars);

            let term = ColorableTerminal::new(stream, &tp.process);
            let inner = term.inner.lock().unwrap();
            assert!(matches!(
                &*inner,
                &TerminalInner::TestWriter(_, choice) if choice == color_choice
            ));
        }

        assert_color_choice(
            "aLWayS",
            StreamSelector::TestWriter(Default::default()),
            ColorChoice::Always,
        );
        assert_color_choice(
            "neVer",
            StreamSelector::TestWriter(Default::default()),
            ColorChoice::Never,
        );
        // tty + `auto` enables the colors.
        assert_color_choice(
            "AutO",
            StreamSelector::TestTtyWriter(Default::default()),
            ColorChoice::Auto,
        );
        // non-tty + `auto` does not enable the colors.
        assert_color_choice(
            "aUTo",
            StreamSelector::TestWriter(Default::default()),
            ColorChoice::Never,
        );
    }
}
