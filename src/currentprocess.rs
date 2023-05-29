use std::boxed::Box;
use std::cell::RefCell;
use std::default::Default;
use std::env;
use std::ffi::OsString;
use std::fmt::Debug;
use std::io;
use std::panic;
use std::path::PathBuf;
use std::sync::Once;
#[cfg(feature = "test")]
use std::{
    collections::HashMap,
    io::Cursor,
    path::Path,
    sync::{Arc, Mutex},
};

use enum_dispatch::enum_dispatch;
use home::env as home;
#[cfg(feature = "test")]
use rand::{thread_rng, Rng};

pub mod argsource;
pub mod cwdsource;
pub mod filesource;
mod homethunk;
pub mod varsource;

use argsource::*;
use cwdsource::*;
use filesource::*;
use varsource::*;

/// An abstraction for the current process.
///
/// This acts as a clonable proxy to the global state provided by some key OS
/// interfaces - it is a zero cost abstraction. For the test variant it manages
/// a mutex and takes out locks to ensure consistency.
///
/// This provides replacements env::arg*, env::var*, and the standard files
/// io::std* with traits that are customisable for tests. As a result any macros
/// or code that have non-pluggable usage of those are incompatible with
/// CurrentProcess and must not be used. That includes \[e\]println! as well as
/// third party crates.
///
/// CurrentProcess is used via an instance in a thread local variable; when
/// making new threads, be sure to copy CurrentProcess::process() into the new
/// thread before calling any code that may need to use a CurrentProcess
/// function.
///
/// Run some code using with: this will set the current instance, call your
/// function, then finally reset the instance at the end before returning.
///
/// Testing level interoperation with external code that depends on environment
/// variables could be possible with a hypothetical  `with_projected()` which
/// would be a zero-cost operation in real processes, but in test processes will
/// take a lock out to mutually exclude other code, then overwrite the current
/// value of std::env::vars, restoring it at the end. However, the only use for
/// that today is a test of cargo::home, which is now implemented in a separate
/// crate, so we've just deleted the test.
///
/// A thread local is used to permit the instance to be available to the entire
/// rustup library without needing to explicitly wire this normally global state
/// everywhere; and a trait object with dyn dispatch is likewise used to avoid
/// needing to thread trait parameters across the entire code base: none of the
/// methods are in performance critical loops (except perhaps progress bars -
/// and even there we should be doing debouncing and managing update rates).
#[enum_dispatch]
pub trait CurrentProcess:
    home::Env
    + ArgSource
    + CurrentDirSource
    + VarSource
    + StdoutSource
    + StderrSource
    + StdinSource
    + ProcessSource
    + Debug
{
}

/// Allows concrete types for the currentprocess abstraction.
#[derive(Clone, Debug)]
#[enum_dispatch(
    CurrentProcess,
    ArgSource,
    CurrentDirSource,
    VarSource,
    StdoutSource,
    StderrSource,
    StdinSource,
    ProcessSource
)]
pub enum Process {
    OSProcess(OSProcess),
    #[cfg(feature = "test")]
    TestProcess(TestProcess),
}

impl Process {
    pub fn name(&self) -> Option<String> {
        let arg0 = match self.var("RUSTUP_FORCE_ARG0") {
            Ok(v) => Some(v),
            Err(_) => self.args().next(),
        }
        .map(PathBuf::from);

        arg0.as_ref()
            .and_then(|a| a.file_stem())
            .and_then(std::ffi::OsStr::to_str)
            .map(String::from)
    }
}

/// Obtain the current instance of CurrentProcess
pub fn process() -> Process {
    home_process()
}

/// Obtain the current instance of HomeProcess
pub(crate) fn home_process() -> Process {
    match PROCESS.with(|p| p.borrow().clone()) {
        None => panic!("No process instance"),
        Some(p) => p,
    }
}

static HOOK_INSTALLED: Once = Once::new();

/// Run a function in the context of a process definition.
///
/// If the function panics, the process definition *in that thread* is cleared
/// by an implicitly installed global panic hook.
pub fn with<F, R>(process: Process, f: F) -> R
where
    F: FnOnce() -> R,
{
    HOOK_INSTALLED.call_once(|| {
        let orig_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            clear_process();
            orig_hook(info);
        }));
    });

    PROCESS.with(|p| {
        if let Some(old_p) = &*p.borrow() {
            panic!("current process already set {old_p:?}");
        }
        *p.borrow_mut() = Some(process);
        let result = f();
        *p.borrow_mut() = None;
        result
    })
}

/// Internal - for the panic hook only
fn clear_process() {
    PROCESS.with(|p| p.replace(None));
}

thread_local! {
    pub(crate) static PROCESS:RefCell<Option<Process>> = RefCell::new(None);
}

// PID related things
#[enum_dispatch]
pub trait ProcessSource {
    /// Returns a unique id for the process.
    ///
    /// Real process ids are <= u32::MAX.
    /// Test process ids are > u32::MAX
    fn id(&self) -> u64;
}

// ----------- real process -----------------

#[derive(Clone, Debug, Default)]
pub struct OSProcess {}

impl ProcessSource for OSProcess {
    fn id(&self) -> u64 {
        std::process::id() as u64
    }
}

// ------------ test process ----------------
#[cfg(feature = "test")]
#[derive(Clone, Debug, Default)]
pub struct TestProcess {
    pub cwd: PathBuf,
    pub args: Vec<String>,
    pub vars: HashMap<String, String>,
    pub id: u64,
    pub stdin: TestStdinInner,
    pub stdout: TestWriterInner,
    pub stderr: TestWriterInner,
}

#[cfg(feature = "test")]
impl TestProcess {
    pub fn new<P: AsRef<Path>, A: AsRef<str>>(
        cwd: P,
        args: &[A],
        vars: HashMap<String, String>,
        stdin: &str,
    ) -> Self {
        TestProcess {
            cwd: cwd.as_ref().to_path_buf(),
            args: args.iter().map(|s| s.as_ref().to_string()).collect(),
            vars,
            id: TestProcess::new_id(),
            stdin: Arc::new(Mutex::new(Cursor::new(stdin.to_string()))),
            stdout: Arc::new(Mutex::new(Vec::new())),
            stderr: Arc::new(Mutex::new(Vec::new())),
        }
    }
    fn new_id() -> u64 {
        let low_bits: u64 = std::process::id() as u64;
        let mut rng = thread_rng();
        let high_bits = rng.gen_range(0..u32::MAX) as u64;
        high_bits << 32 | low_bits
    }

    /// Extracts the stdout from the process
    pub fn get_stdout(&self) -> Vec<u8> {
        self.stdout
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Extracts the stderr from the process
    pub fn get_stderr(&self) -> Vec<u8> {
        self.stderr
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

#[cfg(feature = "test")]
impl ProcessSource for TestProcess {
    fn id(&self) -> u64 {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::env;

    use rustup_macros::unit_test as test;

    use super::{process, with, ProcessSource, TestProcess};

    #[test]
    fn test_instance() {
        let proc = TestProcess::new(
            env::current_dir().unwrap(),
            &["foo", "bar", "baz"],
            HashMap::default(),
            "",
        );
        with(proc.clone().into(), || {
            assert_eq!(proc.id(), process().id(), "{:?} != {:?}", proc, process())
        });
    }
}
