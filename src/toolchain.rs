#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::{
    env::{self, consts::EXE_SUFFIX},
    ffi::{OsStr, OsString},
    fmt::Debug,
    fs,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr,
    time::Duration,
};

use anyhow::{Context, anyhow, bail};
use fs_at::OpenOptions;
use same_file::is_same_file;
use tracing::info;
use url::Url;
use wait_timeout::ChildExt;

use crate::{
    RustupError,
    config::{ActiveReason, Cfg, InstalledPath},
    dist::{
        PartialToolchainDesc, TargetTriple,
        component::{Component, Components},
        prefix::InstallPrefix,
    },
    env_var, install,
    notifications::Notification,
    utils::{self, raw::open_dir_following_links},
};

mod distributable;
pub(crate) use distributable::DistributableToolchain;

mod names;
pub(crate) use names::{
    CustomToolchainName, LocalToolchainName, MaybeOfficialToolchainName,
    MaybeResolvableToolchainName, PathBasedToolchainName, ResolvableLocalToolchainName,
    ResolvableToolchainName, ToolchainName,
};

/// A toolchain installed on the local disk
#[derive(Clone, Debug)]
pub(crate) struct Toolchain<'a> {
    pub(super) cfg: &'a Cfg<'a>,
    name: LocalToolchainName,
    path: PathBuf,
}

impl<'a> Toolchain<'a> {
    pub(crate) async fn from_local(
        name: LocalToolchainName,
        install_if_missing: bool,
        cfg: &'a Cfg<'a>,
    ) -> anyhow::Result<Toolchain<'a>> {
        match Self::new(cfg, name) {
            Ok(tc) => Ok(tc),
            Err(RustupError::ToolchainNotInstalled {
                name: ToolchainName::Official(desc),
                ..
            }) if install_if_missing => {
                Ok(
                    DistributableToolchain::install(cfg, &desc, &[], &[], cfg.get_profile()?, true)
                        .await?
                        .1
                        .toolchain,
                )
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Calls Toolchain::new(), but augments the error message with more context
    /// from the ActiveReason if the toolchain isn't installed.
    pub(crate) fn with_reason(
        cfg: &'a Cfg<'a>,
        name: LocalToolchainName,
        reason: &ActiveReason,
    ) -> anyhow::Result<Self> {
        match Self::new(cfg, name.clone()) {
            Err(RustupError::ToolchainNotInstalled { .. }) => (),
            result => {
                return Ok(result?);
            }
        }

        let reason_err = match reason {
            ActiveReason::Environment => {
                "the RUSTUP_TOOLCHAIN environment variable specifies an uninstalled toolchain"
                    .to_string()
            }
            ActiveReason::CommandLine => {
                "the +toolchain on the command line specifies an uninstalled toolchain".to_string()
            }
            ActiveReason::OverrideDB(path) => format!(
                "the directory override for '{}' specifies an uninstalled toolchain",
                utils::canonicalize_path(path, cfg.notify_handler.as_ref()).display(),
            ),
            ActiveReason::ToolchainFile(path) => format!(
                "the toolchain file at '{}' specifies an uninstalled toolchain",
                utils::canonicalize_path(path, cfg.notify_handler.as_ref()).display(),
            ),
            ActiveReason::Default => {
                "the default toolchain does not describe an installed toolchain".to_string()
            }
        };

        Err(anyhow!(reason_err).context(format!("override toolchain '{name}' is not installed")))
    }

    pub(crate) fn new(cfg: &'a Cfg<'a>, name: LocalToolchainName) -> Result<Self, RustupError> {
        let path = cfg.toolchain_path(&name);
        if !Toolchain::exists(cfg, &name)? {
            return Err(match name {
                LocalToolchainName::Named(name) => {
                    let is_active = matches!(cfg.active_toolchain(), Ok(Some((t, _))) if t == name);
                    RustupError::ToolchainNotInstalled { name, is_active }
                }
                LocalToolchainName::Path(name) => RustupError::PathToolchainNotInstalled(name),
            });
        }
        Ok(Self { cfg, name, path })
    }

    /// Ok(True) if the toolchain exists. Ok(False) if the toolchain or its
    /// containing directory don't exist. Err otherwise.
    pub(crate) fn exists(cfg: &Cfg<'_>, name: &LocalToolchainName) -> Result<bool, RustupError> {
        let path = cfg.toolchain_path(name);
        // toolchain validation should have prevented a situation where there is
        // no base dir, but defensive programming is defensive.
        let parent = path
            .parent()
            .ok_or_else(|| RustupError::InvalidToolchainName(name.to_string()))?;
        let base_name = path
            .file_name()
            .ok_or_else(|| RustupError::InvalidToolchainName(name.to_string()))?;
        let parent_dir = match open_dir_following_links(parent) {
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

    pub(crate) fn name(&self) -> &LocalToolchainName {
        &self.name
    }

    pub(super) fn path(&self) -> &Path {
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
        if let Ok(cargo_home) = self.cfg.process.cargo_home() {
            cmd.env("CARGO_HOME", &cargo_home);
        }

        env_var::inc("RUST_RECURSION_COUNT", cmd, self.cfg.process);

        cmd.env("RUSTUP_TOOLCHAIN", format!("{}", self.name));
        cmd.env("RUSTUP_HOME", &self.cfg.rustup_dir);
    }

    /// Apply the appropriate LD path for a command being run from a toolchain.
    fn set_ldpath(&self, cmd: &mut Command) {
        #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
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

        #[cfg(target_os = "macos")]
        if self
            .cfg
            .process
            .var_os(sysenv::LOADER_PATH)
            .filter(|x| !x.is_empty())
            .is_none()
        {
            // These are the defaults when DYLD_FALLBACK_LIBRARY_PATH isn't
            // set or set to an empty string. Since we are explicitly setting
            // the value, make sure the defaults still work.
            if let Some(home) = self.cfg.process.var_os("HOME") {
                new_path.push(PathBuf::from(home).join("lib"));
            }
            new_path.push(PathBuf::from("/usr/local/lib"));
            new_path.push(PathBuf::from("/usr/lib"));
        }

        env_var::insert_path(sysenv::LOADER_PATH, new_path, None, cmd, self.cfg.process);

        // Prepend CARGO_HOME/bin to the PATH variable so that we're sure to run
        // cargo/rustc via the proxy bins. There is no fallback case for if the
        // proxy bins don't exist. We'll just be running whatever happens to
        // be on the PATH.
        let mut path_entries = vec![];
        if let Ok(cargo_home) = self.cfg.process.cargo_home() {
            path_entries.push(cargo_home.join("bin"));
        }

        // On Windows, we append the "bin" directory to PATH by default.
        // Windows loads DLLs from PATH and the "bin" directory contains DLLs
        // that proc macros and other tools not in the sysroot use.
        // It's appended rather than prepended so that the exe files in "bin"
        // do not take precedence over anything else in PATH.
        //
        // Historically rustup prepended the bin directory in PATH but doing so causes
        // problems because calling tools recursively (like `cargo
        // +nightly metadata` from within a cargo subcommand). The
        // recursive call won't work because it is not executing the
        // proxy, so the `+` toolchain override doesn't work.
        // See: https://github.com/rust-lang/rustup/pull/3178
        //
        // This behaviour was then changed to not add the bin directory at all.
        // But this caused another set of problems due to the sysroot DLLs
        // not being found by the loader, e.g. for proc macros.
        // See: https://github.com/rust-lang/rustup/issues/3825
        //
        // Which is how we arrived at the current default described above.
        //
        // The `RUSTUP_WINDOWS_PATH_ADD_BIN` environment variable allows
        // users to opt-in to one of the old behaviours in case the new
        // default causes any new issues.
        //
        // FIXME: The `RUSTUP_WINDOWS_PATH_ADD_BIN` environment variable can
        // be removed once we're confident that the default behaviour works.
        let append = if cfg!(target_os = "windows") {
            let add_bin = self.cfg.process.var("RUSTUP_WINDOWS_PATH_ADD_BIN");
            match add_bin.as_deref().unwrap_or("append") {
                // Don't add to PATH at all
                "0" => None,
                // Prepend to PATH
                "1" => {
                    path_entries.push(self.path.join("bin"));
                    None
                }
                // Append to PATH (the default)
                _ => Some(self.path.join("bin")),
            }
        } else {
            None
        };

        env_var::insert_path("PATH", path_entries, append, cmd, self.cfg.process);
    }

    /// Infallible function that describes the version of rustc in an installed distribution
    #[tracing::instrument(level = "trace")]
    pub fn rustc_version(&self) -> String {
        match self.create_command("rustc") {
            Ok(mut cmd) => {
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
            }
            Err(_) => String::from("(rustc does not exist)"),
        }
    }

    pub(crate) fn command(&self, binary: &str) -> anyhow::Result<Command> {
        // Should push the cargo fallback into a custom toolchain type? And then
        // perhaps a trait that create command layers on?
        if let "cargo" | "cargo.exe" = binary {
            if let Some(cmd) = self.maybe_do_cargo_fallback()? {
                info!("`cargo` is unavailable for the active toolchain");
                info!("falling back to {:?}", cmd.get_program());
                return Ok(cmd);
            }
        } else if let "rust-analyzer" | "rust-analyzer.exe" = binary {
            if let Some(cmd) = self.maybe_do_rust_analyzer_fallback(binary)? {
                info!("`rust-analyzer` is unavailable for the active toolchain");
                info!("falling back to {:?}", cmd.get_program());
                return Ok(cmd);
            }
        }

        self.create_command(binary)
    }

    // Custom toolchains don't have cargo, so here we detect that situation and
    // try to find a different cargo.
    fn maybe_do_cargo_fallback(&self) -> anyhow::Result<Option<Command>> {
        if let LocalToolchainName::Named(ToolchainName::Official(_)) = self.name() {
            return Ok(None);
        }

        // breadcrumb in case of regression: we used to get the cargo path and
        // cargo.exe path separately, not using the binary_file helper. This may
        // matter if calling a binary with some personality that allows .exe and
        // not .exe to coexist (e.g. wine) - but that's not something we aim to
        // support : the host should always be correct.
        let cargo_path = self.binary_file("cargo");
        if cargo_path.exists() {
            return Ok(None);
        }

        let default_host_triple = self.cfg.get_default_host_triple()?;
        // XXX: This could actually consider all installed distributable
        // toolchains in principle.
        for fallback in ["nightly", "beta", "stable"] {
            let resolved =
                PartialToolchainDesc::from_str(fallback)?.resolve(&default_host_triple)?;
            if let Ok(fallback) = DistributableToolchain::new(self.cfg, resolved) {
                let cmd = fallback.create_fallback_command("cargo", self)?;
                return Ok(Some(cmd));
            }
        }

        Ok(None)
    }

    /// Tries to find `rust-analyzer` on the PATH when the active toolchain does
    /// not have `rust-analyzer` installed.
    ///
    /// This happens from time to time often because the user wants to use a
    /// more recent build of RA than the one shipped with rustup, or because
    /// rustup isn't shipping RA on their host platform at all.
    ///
    /// See the following issues for more context:
    /// - <https://github.com/rust-lang/rustup/issues/3299>
    /// - <https://github.com/rust-lang/rustup/issues/3846>
    fn maybe_do_rust_analyzer_fallback(&self, binary: &str) -> anyhow::Result<Option<Command>> {
        if self.binary_file("rust-analyzer").exists() {
            return Ok(None);
        }

        let proc = self.cfg.process;
        let Some(path) = proc.var_os("PATH") else {
            return Ok(None);
        };

        let me = env::current_exe()?;

        // Try to find the first `rust-analyzer` under the `$PATH` that is both
        // an existing file and not the same file as `me`, i.e. not a rustup proxy.
        for mut p in env::split_paths(&path) {
            p.push(binary);
            let is_external_ra = p.is_file()
                // We report `true` on `is_same_file()` error to prevent an invalid `p`
                // from becoming the candidate.
                && !is_same_file(&me, &p).unwrap_or(true);
            // On Unix, we additionally check if the file is executable.
            #[cfg(unix)]
            let is_external_ra = is_external_ra
                && p.metadata()
                    .is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0);
            if is_external_ra {
                let mut ra = Command::new(p);
                self.set_env(&mut ra);
                return Ok(Some(ra));
            }
        }
        Ok(None)
    }

    #[cfg_attr(feature="otel", tracing::instrument(err, fields(binary, recursion = self.cfg.process.var("RUST_RECURSION_COUNT").ok())))]
    fn create_command<T: AsRef<OsStr> + Debug>(&self, binary: T) -> Result<Command, anyhow::Error> {
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
            let recursion_count = self
                .cfg
                .process
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

        // If we're running cargo and the `CARGO` environment variable is set
        // to a rustup proxy then change `CARGO` to be the real cargo binary,
        // but only if we know the absolute path to cargo.
        // This works around an issue with old versions of cargo not updating
        // the environment variable itself.
        if Path::new(&binary).file_stem() == Some("cargo".as_ref()) && path.is_absolute() {
            if let Some(cargo) = self.cfg.process.var_os("CARGO") {
                if fs::read_link(&cargo).is_ok_and(|p| p.file_stem() == Some("rustup".as_ref())) {
                    cmd.env("CARGO", path);
                }
            }
        }
        Ok(cmd)
    }

    #[cfg(not(windows))]
    pub(crate) fn man_path(&self) -> PathBuf {
        let mut buf = PathBuf::from(&self.path);
        buf.extend(["share", "man"]);
        buf
    }

    pub fn doc_path(&self, relative: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let relative = relative.as_ref();
        if relative.is_absolute() {
            return Ok(relative.to_owned());
        }

        let mut doc_dir = self.path.clone();
        doc_dir.extend(["share", "doc", "rust", "html"]);
        doc_dir.push(relative);

        Ok(doc_dir)
    }

    pub fn open_docs(
        &self,
        relative: impl AsRef<Path>,
        fragment: Option<&str>,
    ) -> anyhow::Result<()> {
        let relative = relative.as_ref();
        let mut doc_url = Url::from_file_path(self.doc_path(relative)?)
            .ok()
            .with_context(|| anyhow!("invalid doc file absolute path `{}`", relative.display()))?;
        doc_url.set_fragment(fragment);
        utils::open_browser(doc_url.to_string())
    }

    /// Remove the toolchain from disk
    ///
    ///
    pub fn ensure_removed(cfg: &Cfg<'_>, name: LocalToolchainName) -> anyhow::Result<()> {
        let path = cfg.toolchain_path(&name);
        let name = match name {
            LocalToolchainName::Named(t) => t,
            LocalToolchainName::Path(_) => bail!("Cannot remove a path based toolchain"),
        };
        let fs_modified = match Self::exists(cfg, &(&name).into())? {
            true => {
                (cfg.notify_handler)(Notification::UninstallingToolchain(&name));
                let installed_paths = match &name {
                    ToolchainName::Custom(_) => Ok(vec![InstalledPath::Dir { path: &path }]),
                    ToolchainName::Official(desc) => cfg.installed_paths(desc, &path),
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
                true
            }
            false => {
                // Might be a dangling symlink
                if path.is_symlink() {
                    (cfg.notify_handler)(Notification::UninstallingToolchain(&name));
                    fs::remove_dir_all(&path)?;
                    true
                } else {
                    let name = name.to_string();
                    info!("no toolchain installed for '{name}'");
                    if name == "self" {
                        info!(
                            "if you meant to uninstall rustup itself, use `rustup self uninstall`"
                        );
                    }
                    false
                }
            }
        };

        if !path.is_symlink() && !path.exists() && fs_modified {
            (cfg.notify_handler)(Notification::UninstalledToolchain(&name));
        }
        Ok(())
    }

    /// Get the list of installed components for any toolchain
    ///
    /// NB: An assumption is made that custom toolchains always have a `rustlib/components` file
    pub fn installed_components(&self) -> anyhow::Result<Vec<Component>> {
        let prefix = InstallPrefix::from(self.path.clone());
        let components = Components::open(prefix)?;
        components.list()
    }

    /// Get the list of installed targets for any toolchain
    pub fn installed_targets(&self) -> anyhow::Result<Vec<TargetTriple>> {
        let targets = self
            .installed_components()?
            .into_iter()
            .filter_map(|c| {
                if c.name().starts_with("rust-std-") {
                    Some(TargetTriple::new(
                        c.name().trim_start_matches("rust-std-").to_string(),
                    ))
                } else {
                    None
                }
            })
            .collect();
        Ok(targets)
    }
}
