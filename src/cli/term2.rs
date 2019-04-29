//! This provides wrappers around the `StdoutTerminal` and `StderrTerminal` types
//! that does not fail if `StdoutTerminal` etc can't be constructed, which happens
//! if TERM isn't defined.

use lazy_static::lazy_static;
use rustup::utils::tty;
use std::io;
use std::sync::Mutex;

pub use term::color;
pub use term::Attr;
pub use term::Terminal;

mod termhack {
    // Things we should submit to term as improvements: here temporarily.
    use std::collections::HashMap;
    use std::io;
    use term::terminfo::TermInfo;
    #[cfg(windows)]
    use term::WinConsole;
    use term::{StderrTerminal, StdoutTerminal, Terminal, TerminfoTerminal};

    // Works around stdio instances being unclonable.
    pub trait Instantiable {
        fn instance() -> Self;
    }

    impl Instantiable for io::Stdout {
        fn instance() -> Self {
            io::stdout()
        }
    }

    impl Instantiable for io::Stderr {
        fn instance() -> Self {
            io::stderr()
        }
    }

    /// Return a Terminal object for T on this platform.
    /// If there is no terminfo and the platform requires terminfo, then None is returned.
    fn make_terminal<T>(terminfo: Option<TermInfo>) -> Option<Box<Terminal<Output = T> + Send>>
    where
        T: 'static + io::Write + Send + Instantiable,
    {
        let mut result = terminfo
            .map(move |ti| TerminfoTerminal::new_with_terminfo(T::instance(), ti.clone()))
            .map(|t| Box::new(t) as Box<Terminal<Output = T> + Send>);
        #[cfg(windows)]
        {
            result = result.or_else(|| {
                WinConsole::new(T::instance())
                    .ok()
                    .map(|t| Box::new(t) as Box<Terminal<Output = T> + Send>)
            })
        }
        result
    }

    fn make_terminal_with_fallback<T>(
        terminfo: Option<TermInfo>,
    ) -> Box<Terminal<Output = T> + Send>
    where
        T: 'static + io::Write + Send + Instantiable,
    {
        make_terminal(terminfo)
            .or_else(|| {
                let ti = TermInfo {
                    names: vec![],
                    bools: HashMap::new(),
                    numbers: HashMap::new(),
                    strings: HashMap::new(),
                };
                let t = TerminfoTerminal::new_with_terminfo(T::instance(), ti);
                Some(Box::new(t) as Box<Terminal<Output = T> + Send>)
            })
            .unwrap()
    }
    /// Return a Terminal wrapping stdout, or None if a terminal couldn't be
    /// opened.
    #[allow(unused)]
    pub fn stdout(terminfo: Option<TermInfo>) -> Option<Box<StdoutTerminal>> {
        make_terminal(terminfo)
    }

    /// Return a Terminal wrapping stderr, or None if a terminal couldn't be
    /// opened.
    #[allow(unused)]
    pub fn stderr(terminfo: Option<TermInfo>) -> Option<Box<StderrTerminal>> {
        make_terminal(terminfo)
    }

    /// Return a Terminal wrapping stdout.
    pub fn stdout_with_fallback(terminfo: Option<TermInfo>) -> Box<StdoutTerminal> {
        make_terminal_with_fallback(terminfo)
    }

    /// Return a Terminal wrapping stderr.
    pub fn stderr_with_fallback(terminfo: Option<TermInfo>) -> Box<StderrTerminal> {
        make_terminal_with_fallback(terminfo)
    }
}

pub trait Isatty {
    fn isatty() -> bool;
}

impl Isatty for io::Stdout {
    fn isatty() -> bool {
        tty::stdout_isatty()
    }
}

impl Isatty for io::Stderr {
    fn isatty() -> bool {
        tty::stderr_isatty()
    }
}

// Decorator to:
// - Disable all terminal controls on non-tty's
// - Swallow errors when we try to use features a terminal doesn't have
//   such as setting colours when no TermInfo DB is present
pub struct AutomationFriendlyTerminal<T>(Box<dyn term::Terminal<Output = T> + Send>)
where
    T: Isatty + io::Write;
pub type StdoutTerminal = AutomationFriendlyTerminal<io::Stdout>;
pub type StderrTerminal = AutomationFriendlyTerminal<io::Stderr>;

macro_rules! swallow_unsupported {
    ( $call:expr ) => {{
        use term::Error::*;
        match $call {
            Ok(()) | Err(ColorOutOfRange) | Err(NotSupported) => Ok(()),
            Err(e) => Err(e),
        }
    }};
}

impl<T> term::Terminal for AutomationFriendlyTerminal<T>
where
    T: io::Write + Isatty,
{
    type Output = T;

    fn fg(&mut self, color: color::Color) -> term::Result<()> {
        if !T::isatty() {
            return Ok(());
        }
        swallow_unsupported!(self.0.fg(color))
    }

    fn bg(&mut self, color: color::Color) -> term::Result<()> {
        if !T::isatty() {
            return Ok(());
        }
        swallow_unsupported!(self.0.bg(color))
    }

    fn attr(&mut self, attr: Attr) -> term::Result<()> {
        if !T::isatty() {
            return Ok(());
        }

        if let Err(e) = self.0.attr(attr) {
            // If `attr` is not supported, try to emulate it
            match attr {
                Attr::Bold => swallow_unsupported!(self.0.fg(color::BRIGHT_WHITE)),
                _ => swallow_unsupported!(Err(e)),
            }
        } else {
            Ok(())
        }
    }

    fn supports_attr(&self, attr: Attr) -> bool {
        self.0.supports_attr(attr)
    }

    fn reset(&mut self) -> term::Result<()> {
        if !T::isatty() {
            return Ok(());
        }
        swallow_unsupported!(self.0.reset())
    }

    /// Returns true if reset is supported.
    fn supports_reset(&self) -> bool {
        self.0.supports_reset()
    }

    fn supports_color(&self) -> bool {
        self.0.supports_color()
    }

    fn cursor_up(&mut self) -> term::Result<()> {
        if !T::isatty() {
            return Ok(());
        }

        swallow_unsupported!(self.0.cursor_up())
    }

    fn delete_line(&mut self) -> term::Result<()> {
        swallow_unsupported!(self.0.delete_line())
    }

    fn carriage_return(&mut self) -> term::Result<()> {
        // This might leak control chars in !isatty? needs checking.
        swallow_unsupported!(self.0.carriage_return())
    }

    fn get_ref(&self) -> &Self::Output {
        self.0.get_ref()
    }

    fn get_mut(&mut self) -> &mut Self::Output {
        self.0.get_mut()
    }

    /// Returns the contained stream, destroying the `Terminal`
    fn into_inner(self) -> Self::Output
    where
        Self: Sized,
    {
        unimplemented!()
        // self.0.into_inner().into_inner()
    }
}

impl<T: Isatty + io::Write> io::Write for AutomationFriendlyTerminal<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.0.flush()
    }
}

lazy_static! {
    // Cache the terminfo database for performance.
    // Caching the actual terminals may be better, as on Windows terminal
    // detection is per-fd, but this at least avoids the IO subsystem and
    // caching the stdout instances is more complex
    static ref TERMINFO: Mutex<Option<term::terminfo::TermInfo>> =
        Mutex::new(term::terminfo::TermInfo::from_env().ok());
}

pub fn stdout() -> StdoutTerminal {
    let info_result = TERMINFO.lock().unwrap().clone();
    AutomationFriendlyTerminal(termhack::stdout_with_fallback(info_result))
}

pub fn stderr() -> StderrTerminal {
    let info_result = TERMINFO.lock().unwrap().clone();
    AutomationFriendlyTerminal(termhack::stderr_with_fallback(info_result))
}
