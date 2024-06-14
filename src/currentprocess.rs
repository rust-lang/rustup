use std::env;
use std::ffi::OsString;
use std::fmt::Debug;
use std::future::Future;
use std::io;
use std::panic;
use std::path::PathBuf;
use std::sync::Once;
use std::{cell::RefCell, io::IsTerminal};
#[cfg(feature = "test")]
use std::{
    collections::HashMap,
    io::Cursor,
    path::Path,
    sync::{Arc, Mutex},
};

#[cfg(feature = "test")]
use rand::{thread_rng, Rng};
use tracing_subscriber::util::SubscriberInitExt;

pub mod filesource;
pub mod terminalsource;

/// Allows concrete types for the currentprocess abstraction.
#[derive(Clone, Debug)]
pub enum Process {
    OSProcess(OSProcess),
    #[cfg(feature = "test")]
    TestProcess(TestProcess),
}

impl Process {
    pub fn os() -> Self {
        Self::OSProcess(OSProcess::new())
    }

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

    pub fn var(&self, key: &str) -> Result<String, env::VarError> {
        match self {
            Process::OSProcess(_) => env::var(key),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => match p.vars.get(key) {
                Some(val) => Ok(val.to_owned()),
                None => Err(env::VarError::NotPresent),
            },
        }
    }

    pub(crate) fn var_os(&self, key: &str) -> Option<OsString> {
        match self {
            Process::OSProcess(_) => env::var_os(key),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => p.vars.get(key).map(OsString::from),
        }
    }

    pub(crate) fn args(&self) -> Box<dyn Iterator<Item = String> + '_> {
        match self {
            Process::OSProcess(_) => Box::new(env::args()),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => Box::new(p.args.iter().cloned()),
        }
    }

    pub(crate) fn args_os(&self) -> Box<dyn Iterator<Item = OsString> + '_> {
        match self {
            Process::OSProcess(_) => Box::new(env::args_os()),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => Box::new(p.args.iter().map(OsString::from)),
        }
    }

    pub(crate) fn stdin(&self) -> Box<dyn filesource::Stdin> {
        match self {
            Process::OSProcess(_) => Box::new(io::stdin()),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => Box::new(filesource::TestStdin(p.stdin.clone())),
        }
    }

    pub(crate) fn stdout(&self) -> Box<dyn filesource::Writer> {
        match self {
            Process::OSProcess(_) => Box::new(io::stdout()),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => Box::new(filesource::TestWriter(p.stdout.clone())),
        }
    }

    pub(crate) fn stderr(&self) -> Box<dyn filesource::Writer> {
        match self {
            Process::OSProcess(_) => Box::new(io::stderr()),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => Box::new(filesource::TestWriter(p.stderr.clone())),
        }
    }

    pub fn current_dir(&self) -> io::Result<PathBuf> {
        match self {
            Process::OSProcess(_) => env::current_dir(),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => Ok(p.cwd.clone()),
        }
    }

    #[cfg(test)]
    fn id(&self) -> u64 {
        match self {
            Process::OSProcess(_) => std::process::id() as u64,
            #[cfg(feature = "test")]
            Process::TestProcess(p) => p.id,
        }
    }
}

impl home::env::Env for Process {
    fn home_dir(&self) -> Option<PathBuf> {
        match self {
            Process::OSProcess(_) => self.var("HOME").ok().map(|v| v.into()),
            #[cfg(feature = "test")]
            Process::TestProcess(_) => home::env::OS_ENV.home_dir(),
        }
    }

    fn current_dir(&self) -> Result<PathBuf, io::Error> {
        match self {
            Process::OSProcess(_) => self.current_dir(),
            #[cfg(feature = "test")]
            Process::TestProcess(_) => home::env::OS_ENV.current_dir(),
        }
    }

    fn var_os(&self, key: &str) -> Option<OsString> {
        match self {
            Process::OSProcess(_) => self.var_os(key),
            #[cfg(feature = "test")]
            Process::TestProcess(_) => self.var_os(key),
        }
    }
}

#[cfg(feature = "test")]
impl From<TestProcess> for Process {
    fn from(p: TestProcess) -> Self {
        Self::TestProcess(p)
    }
}

/// Obtain the current instance of CurrentProcess
pub fn process() -> Process {
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
    ensure_hook();

    PROCESS.with(|p| {
        if let Some(old_p) = &*p.borrow() {
            panic!("current process already set {old_p:?}");
        }
        *p.borrow_mut() = Some(process.clone());
        let _guard = crate::cli::log::tracing_subscriber(process).set_default();
        let result = f();
        *p.borrow_mut() = None;
        result
    })
}

fn ensure_hook() {
    HOOK_INSTALLED.call_once(|| {
        let orig_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            clear_process();
            orig_hook(info);
        }));
    });
}

/// Run a function in the context of a process definition and a tokio runtime.
///
/// The process state is injected into a thread-local in every work thread of
/// the runtime, but this requires access to the runtime builder, so this
/// function must be the one to create the runtime.
pub fn with_runtime<'a, R>(
    process: Process,
    mut runtime_builder: tokio::runtime::Builder,
    fut: impl Future<Output = R> + 'a,
) -> R {
    ensure_hook();

    let start_process = process.clone();
    let unpark_process = process.clone();
    let runtime = runtime_builder
        // propagate to blocking threads
        .on_thread_start(move || {
            // assign the process persistently to the thread local.
            PROCESS.with(|p| {
                if let Some(old_p) = &*p.borrow() {
                    panic!("current process already set {old_p:?}");
                }
                *p.borrow_mut() = Some(start_process.clone());
                // Thread exits will clear the process.
            });
        })
        .on_thread_stop(move || {
            PROCESS.with(|p| {
                *p.borrow_mut() = None;
            });
        })
        // propagate to async worker threads
        .on_thread_unpark(move || {
            // assign the process persistently to the thread local.
            PROCESS.with(|p| {
                if let Some(old_p) = &*p.borrow() {
                    panic!("current process already set {old_p:?}");
                }
                *p.borrow_mut() = Some(unpark_process.clone());
                // Thread exits will clear the process.
            });
        })
        .on_thread_park(move || {
            PROCESS.with(|p| {
                *p.borrow_mut() = None;
            });
        })
        .build()
        .unwrap();

    // The current thread doesn't get hooks run on it.
    PROCESS.with(move |p| {
        if let Some(old_p) = &*p.borrow() {
            panic!("current process already set {old_p:?}");
        }
        *p.borrow_mut() = Some(process.clone());
        let result = runtime.block_on(async {
            let _guard = crate::cli::log::tracing_subscriber(process).set_default();
            fut.await
        });
        *p.borrow_mut() = None;
        result
    })
}

/// Internal - for the panic hook only
fn clear_process() {
    PROCESS.with(|p| p.replace(None));
}

thread_local! {
    pub(crate) static PROCESS: RefCell<Option<Process>> = const { RefCell::new(None) };
}

// ----------- real process -----------------

#[derive(Clone, Debug)]
pub struct OSProcess {
    pub(self) stderr_is_a_tty: bool,
    pub(self) stdout_is_a_tty: bool,
}

impl OSProcess {
    pub fn new() -> Self {
        OSProcess {
            stderr_is_a_tty: io::stderr().is_terminal(),
            stdout_is_a_tty: io::stdout().is_terminal(),
        }
    }
}

impl Default for OSProcess {
    fn default() -> Self {
        OSProcess::new()
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
    pub stdin: filesource::TestStdinInner,
    pub stdout: filesource::TestWriterInner,
    pub stderr: filesource::TestWriterInner,
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::env;

    use rustup_macros::unit_test as test;

    use super::{process, with, TestProcess};

    #[test]
    fn test_instance() {
        let proc = TestProcess::new(
            env::current_dir().unwrap(),
            &["foo", "bar", "baz"],
            HashMap::default(),
            "",
        );
        with(proc.clone().into(), || {
            assert_eq!(proc.id, process().id(), "{:?} != {:?}", proc, process())
        });
    }
}
