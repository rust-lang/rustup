use std::{
    io::{self, Write},
    mem::MaybeUninit,
    ops::DerefMut,
    ptr::addr_of_mut,
    sync::{Arc, Mutex, MutexGuard},
};

pub(crate) use termcolor::Color;
use termcolor::{ColorChoice, ColorSpec, StandardStream, StandardStreamLock, WriteColor};

#[cfg(feature = "test")]
use super::filesource::{TestWriter, TestWriterLock};
use super::{process, varsource::VarSource};

/// Select what stream to make a terminal on
pub(super) enum StreamSelector {
    Stdout,
    Stderr,
    #[cfg(feature = "test")]
    TestWriter(TestWriter),
}

impl StreamSelector {
    fn is_a_tty(&self) -> bool {
        match self {
            StreamSelector::Stdout => match process() {
                super::Process::OSProcess(p) => p.stdout_is_a_tty,
                #[cfg(feature = "test")]
                super::Process::TestProcess(_) => unreachable!(),
            },
            StreamSelector::Stderr => match process() {
                super::Process::OSProcess(p) => p.stderr_is_a_tty,
                #[cfg(feature = "test")]
                super::Process::TestProcess(_) => unreachable!(),
            },
            #[cfg(feature = "test")]
            StreamSelector::TestWriter(_) => false,
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
}

/// Internal state for ColorableTerminal
enum TerminalInner {
    StandardStream(StandardStream, ColorSpec),
    #[cfg(feature = "test")]
    TestWriter(TestWriter),
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
    pub(super) fn new(stream: StreamSelector) -> Self {
        let env_override = process().var("RUSTUP_TERM_COLOR");
        let choice = match env_override.as_deref() {
            Ok("always") => ColorChoice::Always,
            Ok("never") => ColorChoice::Never,
            _ if stream.is_a_tty() => ColorChoice::Auto,
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
            StreamSelector::TestWriter(w) => TerminalInner::TestWriter(w),
        };
        ColorableTerminal {
            inner: Arc::new(Mutex::new(inner)),
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
                TerminalInner::TestWriter(w) => TerminalInnerLocked::TestWriter(w.lock()),
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
            TerminalInner::TestWriter(_) => Ok(()),
        }
    }

    pub fn bg(&mut self, color: Color) -> io::Result<()> {
        match self.inner.lock().unwrap().deref_mut() {
            TerminalInner::StandardStream(s, spec) => {
                spec.set_bg(Some(color));
                s.set_color(spec)
            }
            #[cfg(feature = "test")]
            TerminalInner::TestWriter(_) => Ok(()),
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
            TerminalInner::TestWriter(_) => Ok(()),
        }
    }

    pub fn reset(&mut self) -> io::Result<()> {
        match self.inner.lock().unwrap().deref_mut() {
            TerminalInner::StandardStream(s, _color) => s.reset(),
            #[cfg(feature = "test")]
            TerminalInner::TestWriter(_) => Ok(()),
        }
    }

    pub fn carriage_return(&mut self) -> io::Result<()> {
        match self.inner.lock().unwrap().deref_mut() {
            TerminalInner::StandardStream(s, _color) => s.write(b"\r")?,
            #[cfg(feature = "test")]
            TerminalInner::TestWriter(w) => w.write(b"\r")?,
        };
        Ok(())
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
            TerminalInner::TestWriter(w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> std::result::Result<(), io::Error> {
        match self.inner.lock().unwrap().deref_mut() {
            TerminalInner::StandardStream(s, _) => s.flush(),
            #[cfg(feature = "test")]
            TerminalInner::TestWriter(w) => w.flush(),
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
