use std::{
    env::{self, consts::EXE_SUFFIX},
    ffi::{OsStr, OsString},
    fmt::Debug,
    fs,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{anyhow, bail};
use derivative::Derivative;
use fs_at::OpenOptions;
use wait_timeout::ChildExt;

use crate::{
    config::Cfg,
    currentprocess::process,
    env_var, install,
    notifications::Notification,
    utils::{raw::open_dir, utils},
    RustupError,
};

use super::{
    custom::CustomToolchain,
    distributable::DistributableToolchain,
    names::{LocalToolchainName, ToolchainName},
};

/// A toolchain installed on the local disk
#[derive(Derivative)]
#[derivative(Clone, Debug)]
pub(crate) struct Toolchain<'a> {
    cfg: &'a Cfg,
    name: LocalToolchainName,
    path: PathBuf,
}

impl<'a> Toolchain<'a> {
    pub(crate) fn new(cfg: &'a Cfg, name: LocalToolchainName) -> Result<Self, RustupError> {
        let path = cfg.toolchain_path(&name);
        if !Toolchain::exists(cfg, &name)? {
            return Err(match name {
                LocalToolchainName::Named(name) => RustupError::ToolchainNotInstalled(name),
                LocalToolchainName::Path(name) => RustupError::PathToolchainNotInstalled(name),
            });
        }
        Ok(Self { cfg, name, path })
    }

    /// Ok(True) if the toolchain exists. Ok(False) if the toolchain or its
    /// containing directory don't exist. Err otherwise.
    pub(crate) fn exists(cfg: &'a Cfg, name: &LocalToolchainName) -> Result<bool, RustupError> {
        let path = cfg.toolchain_path(name);
        // toolchain validation should have prevented a situation where there is
        // no base dir, but defensive programming is defensive.
        let parent = path
            .parent()
            .ok_or_else(|| RustupError::InvalidToolchainName(name.to_string()))?;
        let base_name = path
            .file_name()
            .ok_or_else(|| RustupError::InvalidToolchainName(name.to_string()))?;
        let parent_dir = match open_dir(parent) {
            Ok(d) => d,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            e => e?,
        };
        let opened = OpenOptions::default()
            .read(true)
            .follow(true)
            .open_dir_at(&parent_dir, base_name);
        Ok(opened.is_ok())
    }

    pub(crate) fn cfg(&self) -> &'a Cfg {
        self.cfg
    }

    pub(crate) fn name(&self) -> &LocalToolchainName {
        &self.name
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// The path to a binary within the toolchain, without regard for cargo-fallback logic
    pub fn binary_file(&self, name: &str) -> PathBuf {
        let mut path = self.path.clone();
        path.push("bin");
        path.push(name.to_owned() + env::consts::EXE_SUFFIX);
        path
    }

    /// Not intended to be public, but more code golf required to get it hidden.
    /// pub because of create_fallback_command
    pub fn set_env(&self, cmd: &mut Command) {
        self.set_ldpath(cmd);

        // Older versions of Cargo used a slightly different definition of
        // cargo home. Rustup does not read HOME on Windows whereas the older
        // versions of Cargo did. Rustup and Cargo should be in sync now (both
        // using the same `home` crate), but this is retained to ensure cargo
        // and rustup agree in older versions.
        if let Ok(cargo_home) = utils::cargo_home() {
            cmd.env("CARGO_HOME", &cargo_home);
        }

        env_var::inc("RUST_RECURSION_COUNT", cmd);

        cmd.env("RUSTUP_TOOLCHAIN", format!("{}", self.name));
        cmd.env("RUSTUP_HOME", &self.cfg().rustup_dir);
    }

    /// Apply the appropriate LD path for a command being run from a toolchain.
    fn set_ldpath(&self, cmd: &mut Command) {
        let mut new_path = vec![self.path.join("lib")];

        #[cfg(not(target_os = "macos"))]
        mod sysenv {
            pub const LOADER_PATH: &str = "LD_LIBRARY_PATH";
        }
        #[cfg(target_os = "macos")]
        mod sysenv {
            // When loading and linking a dynamic library or bundle, dlopen
            // searches in LD_LIBRARY_PATH, DYLD_LIBRARY_PATH, PWD, and
            // DYLD_FALLBACK_LIBRARY_PATH.
            // In the Mach-O format, a dynamic library has an "install path."
            // Clients linking against the library record this path, and the
            // dynamic linker, dyld, uses it to locate the library.
            // dyld searches DYLD_LIBRARY_PATH *before* the install path.
            // dyld searches DYLD_FALLBACK_LIBRARY_PATH only if it cannot
            // find the library in the install path.
            // Setting DYLD_LIBRARY_PATH can easily have unintended
            // consequences.
            pub const LOADER_PATH: &str = "DYLD_FALLBACK_LIBRARY_PATH";
        }
        if cfg!(target_os = "macos")
            && process()
                .var_os(sysenv::LOADER_PATH)
                .filter(|x| x.len() > 0)
                .is_none()
        {
            // These are the defaults when DYLD_FALLBACK_LIBRARY_PATH isn't
            // set or set to an empty string. Since we are explicitly setting
            // the value, make sure the defaults still work.
            if let Some(home) = process().var_os("HOME") {
                new_path.push(PathBuf::from(home).join("lib"));
            }
            new_path.push(PathBuf::from("/usr/local/lib"));
            new_path.push(PathBuf::from("/usr/lib"));
        }

        env_var::prepend_path(sysenv::LOADER_PATH, new_path, cmd);

        // Prepend CARGO_HOME/bin to the PATH variable so that we're sure to run
        // cargo/rustc via the proxy bins. There is no fallback case for if the
        // proxy bins don't exist. We'll just be running whatever happens to
        // be on the PATH.
        let mut path_entries = vec![];
        if let Ok(cargo_home) = utils::cargo_home() {
            path_entries.push(cargo_home.join("bin"));
        }

        if cfg!(target_os = "windows") {
            // Historically rustup has included the bin directory in PATH to
            // work around some bugs (see
            // https://github.com/rust-lang/rustup/pull/3178 for more
            // information). This shouldn't be needed anymore, and it causes
            // problems because calling tools recursively (like `cargo
            // +nightly metadata` from within a cargo subcommand). The
            // recursive call won't work because it is not executing the
            // proxy, so the `+` toolchain override doesn't work.
            //
            // This is opt-in to allow us to get more real-world testing.
            if process()
                .var_os("RUSTUP_WINDOWS_PATH_ADD_BIN")
                .map_or(true, |s| s == "1")
            {
                path_entries.push(self.path.join("bin"));
            }
        }

        env_var::prepend_path("PATH", path_entries, cmd);
    }

    /// Infallible function that describes the version of rustc in an installed distribution
    #[cfg_attr(feature = "otel", tracing::instrument)]
    pub fn rustc_version(&self) -> String {
        // TODO: use create_command instead of manual construction!
        let rustc_path = self.binary_file("rustc");
        if utils::is_file(&rustc_path) {
            let mut cmd = Command::new(&rustc_path);
            cmd.arg("--version");
            cmd.stdin(Stdio::null());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            self.set_ldpath(&mut cmd);

            // some toolchains are faulty with some combinations of platforms and
            // may fail to launch but also to timely terminate.
            // (known cases include Rust 1.3.0 through 1.10.0 in recent macOS Sierra.)
            // we guard against such cases by enforcing a reasonable timeout to read.
            let mut line1 = None;
            if let Ok(mut child) = cmd.spawn() {
                let timeout = Duration::new(10, 0);
                match child.wait_timeout(timeout) {
                    Ok(Some(status)) if status.success() => {
                        let out = child
                            .stdout
                            .expect("Child::stdout requested but not present");
                        let mut line = String::new();
                        if BufReader::new(out).read_line(&mut line).is_ok() {
                            let lineend = line.trim_end_matches(&['\r', '\n'][..]).len();
                            line.truncate(lineend);
                            line1 = Some(line);
                        }
                    }
                    Ok(None) => {
                        let _ = child.kill();
                        return String::from("(timeout reading rustc version)");
                    }
                    Ok(Some(_)) | Err(_) => {}
                }
            }

            if let Some(line1) = line1 {
                line1
            } else {
                String::from("(error reading rustc version)")
            }
        } else {
            String::from("(rustc does not exist)")
        }
    }

    #[cfg_attr(feature="otel", tracing::instrument(err,fields(binary, recursion=process().var("RUST_RECURSION_COUNT").ok())))]
    pub fn create_command<T: AsRef<OsStr> + Debug>(
        &self,
        binary: T,
    ) -> Result<Command, anyhow::Error> {
        // Create the path to this binary within the current toolchain sysroot
        let binary = if let Some(binary_str) = binary.as_ref().to_str() {
            if binary_str.to_lowercase().ends_with(EXE_SUFFIX) {
                binary.as_ref().to_owned()
            } else {
                OsString::from(format!("{binary_str}{EXE_SUFFIX}"))
            }
        } else {
            // Very weird case. Non-unicode command.
            binary.as_ref().to_owned()
        };

        let bin_path = self.path.join("bin").join(&binary);
        let path = if utils::is_file(&bin_path) {
            &bin_path
        } else {
            let recursion_count = process()
                .var("RUST_RECURSION_COUNT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if recursion_count > env_var::RUST_RECURSION_COUNT_MAX - 1 {
                let binary_lossy: String = binary.to_string_lossy().into();
                if matches!(
                    &self.name,
                    LocalToolchainName::Named(ToolchainName::Official(_))
                ) {
                    let distributable = DistributableToolchain::try_from(self)?;
                    // Design note: this is a bit of an awkward cast from
                    // general (toolchain) to more specialised (distributable);
                    // perhaps this function should something implemented on a
                    // trait, permitting removal of that case.
                    return Err(distributable.recursion_error(binary_lossy).unwrap_err());
                } else {
                    let t = &self.name;
                    return Err(anyhow!(
                        "'{binary_lossy}' is not installed for the custom toolchain '{t}'.\nnote: this is a custom toolchain, which cannot use `rustup component add`\n\
                        help: if you built this toolchain from source, and used `rustup toolchain link`, then you may be able to build the component with `x.py`"
                    ));
                }
            }
            Path::new(&binary)
        };
        let mut cmd = Command::new(path);
        self.set_env(&mut cmd);
        Ok(cmd)
    }

    pub fn doc_path(&self, relative: &str) -> anyhow::Result<PathBuf> {
        let parts = vec!["share", "doc", "rust", "html"];
        let mut doc_dir = self.path.clone();
        for part in parts {
            doc_dir.push(part);
        }
        doc_dir.push(relative);

        Ok(doc_dir)
    }

    pub fn open_docs(&self, relative: &str) -> anyhow::Result<()> {
        utils::open_browser(&self.doc_path(relative)?)
    }

    /// Remove the toolchain from disk
    ///
    ///
    pub fn ensure_removed(cfg: &'a Cfg, name: LocalToolchainName) -> anyhow::Result<()> {
        let path = cfg.toolchain_path(&name);
        let name = match name {
            LocalToolchainName::Named(t) => t,
            LocalToolchainName::Path(_) => bail!("Cannot remove a path based toolchain"),
        };
        match Self::exists(cfg, &(&name).into())? {
            true => {
                (cfg.notify_handler)(Notification::UninstallingToolchain(&name));
                let installed_paths = match &name {
                    ToolchainName::Custom(_) => CustomToolchain::installed_paths(&path),
                    ToolchainName::Official(desc) => {
                        DistributableToolchain::installed_paths(cfg, desc, &path)
                    }
                }?;
                for path in installed_paths {
                    match path {
                        InstalledPath::File { name, path } => {
                            utils::ensure_file_removed(name, &path)?
                        }
                        InstalledPath::Dir { path } => {
                            install::uninstall(path, &|n| (cfg.notify_handler)(n.into()))?
                        }
                    }
                }
            }
            false => {
                // Might be a dangling symlink
                if path.is_symlink() {
                    (cfg.notify_handler)(Notification::UninstallingToolchain(&name));
                    fs::remove_dir_all(&path)?;
                } else {
                    info!("no toolchain installed for '{name}'");
                }
            }
        }

        if !path.is_symlink() && !path.exists() {
            (cfg.notify_handler)(Notification::UninstalledToolchain(&name));
        }
        Ok(())
    }
}

/// Installed paths
pub(crate) enum InstalledPath<'a> {
    File { name: &'static str, path: PathBuf },
    Dir { path: &'a Path },
}
