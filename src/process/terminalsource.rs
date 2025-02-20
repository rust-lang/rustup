use std::{
    io::{self, Write},
    mem::MaybeUninit,
    ops::DerefMut,
    ptr::addr_of_mut,
    sync::{Arc, Mutex, MutexGuard},
};

pub(crate) use termcolor::Color;
use termcolor::{ColorChoice, ColorSpec, StandardStream, StandardStreamLock, WriteColor};

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
        let choice = match process.var("RUSTUP_TERM_COLOR") {
            Ok(s) if s.eq_ignore_ascii_case("always") => ColorChoice::Always,
            Ok(s) if s.eq_ignore_ascii_case("never") => ColorChoice::Never,
            _ if stream.is_a_tty(process) => ColorChoice::Auto,
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

    pub fn carriage_return(&mut self) -> io::Result<()> {
        match self.inner.lock().unwrap().deref_mut() {
            TerminalInner::StandardStream(s, _color) => s.write(b"\r")?,
            #[cfg(feature = "test")]
            TerminalInner::TestWriter(w, _) => w.write(b"\r")?,
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
