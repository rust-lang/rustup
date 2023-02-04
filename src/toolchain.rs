use std::env;
use std::env::consts::EXE_SUFFIX;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use thiserror::Error as ThisError;
use wait_timeout::ChildExt;

use crate::component_for_bin;
use crate::config::Cfg;
use crate::dist::dist::Profile;
use crate::dist::dist::TargetTriple;
use crate::dist::dist::ToolchainDesc;
use crate::dist::download::DownloadCfg;
use crate::dist::manifest::Component;
use crate::dist::manifest::Manifest;
use crate::dist::manifestation::{Changes, Manifestation};
use crate::dist::prefix::InstallPrefix;
use crate::env_var;
use crate::errors::*;
use crate::install::{self, InstallMethod};
use crate::notifications::*;
use crate::process;
use crate::utils::utils;

/// An installed toolchain
trait InstalledToolchain<'a> {
    /// What (root) paths are associated with this installed toolchain.
    fn installed_paths(&self) -> Result<Vec<InstalledPath<'a>>>;
}

/// Installed paths
enum InstalledPath<'a> {
    File { name: &'static str, path: PathBuf },
    Dir { path: &'a Path },
}

/// A fully resolved reference to a toolchain which may or may not exist
pub struct Toolchain<'a> {
    cfg: &'a Cfg,
    name: String,
    path: PathBuf,
    dist_handler: Box<dyn Fn(crate::dist::Notification<'_>) + 'a>,
}

/// Used by the `list_component` function
pub struct ComponentStatus {
    pub component: Component,
    pub name: String,
    pub installed: bool,
    pub available: bool,
}

#[derive(Clone, Debug)]
pub enum UpdateStatus {
    Installed,
    Updated(String), // Stores the version of rustc *before* the update
    Unchanged,
}

static V1_COMMON_COMPONENT_LIST: &[&str] = &["cargo", "rustc", "rust-docs"];

impl<'a> Toolchain<'a> {
    pub(crate) fn from(cfg: &'a Cfg, name: &str) -> Result<Self> {
        let resolved_name = cfg.resolve_toolchain(name)?;
        let path = cfg.toolchains_dir.join(&resolved_name);
        Ok(Toolchain {
            cfg,
            name: resolved_name,
            path,
            dist_handler: Box::new(move |n| (cfg.notify_handler)(n.into())),
        })
    }

    pub(crate) fn from_path(
        cfg: &'a Cfg,
        cfg_file: Option<impl AsRef<Path>>,
        path: impl AsRef<Path>,
    ) -> Result<Self> {
        let path = if let Some(cfg_file) = cfg_file {
            cfg_file.as_ref().parent().unwrap().join(path)
        } else {
            path.as_ref().to_path_buf()
        };

        #[derive(Debug, ThisError)]
        #[error("invalid toolchain path: '{}'", .0.to_string_lossy())]
        struct InvalidToolchainPath(PathBuf);

        // Perform minimal validation; there should at least be a `bin/` that might
        // contain things for us to run.
        if !path.join("bin").is_dir() {
            bail!(InvalidToolchainPath(path));
        }

        Ok(Toolchain {
            cfg,
            name: utils::canonicalize_path(&path, cfg.notify_handler.as_ref())
                .to_str()
                .ok_or_else(|| anyhow!(InvalidToolchainPath(path.clone())))?
                .to_owned(),
            path,
            dist_handler: Box::new(move |n| (cfg.notify_handler)(n.into())),
        })
    }

    pub fn as_installed_common(&'a self) -> Result<InstalledCommonToolchain<'a>> {
        if !self.exists() {
            // Should be verify perhaps?
            return Err(RustupError::ToolchainNotInstalled(self.name.to_owned()).into());
        }
        Ok(InstalledCommonToolchain(self))
    }

    fn as_installed(&'a self) -> Result<Box<dyn InstalledToolchain<'a> + 'a>> {
        if self.is_custom() {
            let toolchain = CustomToolchain::new(self)?;
            Ok(Box::new(toolchain) as Box<dyn InstalledToolchain<'a>>)
        } else {
            let toolchain = DistributableToolchain::new(self)?;
            Ok(Box::new(toolchain) as Box<dyn InstalledToolchain<'a>>)
        }
    }
    pub(crate) fn cfg(&self) -> &Cfg {
        self.cfg
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    fn is_symlink(&self) -> bool {
        use std::fs;
        fs::symlink_metadata(&self.path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    }
    /// Is there a filesystem component with the name of the toolchain in the toolchains dir - valid install or not.
    /// Used to determine whether this toolchain should be uninstallable.
    /// Custom and Distributable. Installed and uninstalled. (perhaps onstalled only?)
    pub fn exists(&self) -> bool {
        // HACK: linked toolchains are symlinks, and, contrary to what std docs
        // lead me to believe `fs::metadata`, used by `is_directory` does not
        // seem to follow symlinks on windows.
        let is_symlink = if cfg!(windows) {
            self.is_symlink()
        } else {
            false
        };
        utils::is_directory(&self.path) || is_symlink
    }
    /// Is there a valid usable toolchain with this name, either in the toolchains dir, or symlinked from it.
    // Could in future check for rustc perhaps.
    // Custom and Distributable. Installed only?
    pub fn verify(&self) -> Result<()> {
        utils::assert_is_directory(&self.path)
    }
    // Custom and Distributable. Installed only.
    pub fn remove(&self) -> Result<()> {
        if self.exists() || self.is_symlink() {
            (self.cfg.notify_handler)(Notification::UninstallingToolchain(&self.name));
        } else {
            (self.cfg.notify_handler)(Notification::ToolchainNotInstalled(&self.name));
            return Ok(());
        }
        let installed = self.as_installed()?;
        for path in installed.installed_paths()? {
            match path {
                InstalledPath::File { name, path } => utils::ensure_file_removed(name, &path)?,
                InstalledPath::Dir { path } => {
                    install::uninstall(path, &|n| (self.cfg.notify_handler)(n.into()))?
                }
            }
        }
        if !self.exists() {
            (self.cfg.notify_handler)(Notification::UninstalledToolchain(&self.name));
        }
        Ok(())
    }

    // Custom only
    pub fn is_custom(&self) -> bool {
        Toolchain::is_custom_name(&self.name)
    }

    pub(crate) fn is_custom_name(name: &str) -> bool {
        ToolchainDesc::from_str(name).is_err()
    }

    // Distributable only
    pub fn is_tracking(&self) -> bool {
        ToolchainDesc::from_str(&self.name)
            .ok()
            .map(|d| d.is_tracking())
            == Some(true)
    }

    // Custom and Distributable. Installed only.
    pub fn doc_path(&self, relative: &str) -> Result<PathBuf> {
        self.verify()?;

        let parts = vec!["share", "doc", "rust", "html"];
        let mut doc_dir = self.path.clone();
        for part in parts {
            doc_dir.push(part);
        }
        doc_dir.push(relative);

        Ok(doc_dir)
    }
    // Custom and Distributable. Installed only.
    pub fn open_docs(&self, relative: &str) -> Result<()> {
        self.verify()?;

        utils::open_browser(&self.doc_path(relative)?)
    }
    // Custom and Distributable. Installed only.
    pub fn make_default(&self) -> Result<()> {
        self.cfg.set_default(&self.name)
    }
    // Custom and Distributable. Installed only.
    pub fn make_override(&self, path: &Path) -> Result<()> {
        self.cfg.settings_file.with_mut(|s| {
            s.add_override(path, self.name.clone(), self.cfg.notify_handler.as_ref());
            Ok(())
        })
    }
    // Distributable and Custom. Installed only.
    pub fn binary_file(&self, name: &str) -> PathBuf {
        let mut path = self.path.clone();
        path.push("bin");
        path.push(name.to_owned() + env::consts::EXE_SUFFIX);
        path
    }
    // Distributable and Custom. Installed only.
    pub fn rustc_version(&self) -> String {
        if let Ok(installed) = self.as_installed_common() {
            let rustc_path = self.binary_file("rustc");
            if utils::is_file(&rustc_path) {
                let mut cmd = Command::new(&rustc_path);
                cmd.arg("--version");
                cmd.stdin(Stdio::null());
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());
                installed.set_ldpath(&mut cmd);

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
        } else {
            String::from("(toolchain not installed)")
        }
    }
}

impl<'a> std::fmt::Debug for Toolchain<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Toolchain")
            .field("name", &self.name)
            .field("path", &self.path)
            .finish()
    }
}

fn install_msg(bin: &str, toolchain: &str, is_default: bool) -> String {
    if Toolchain::is_custom_name(toolchain) {
        return "\nnote: this is a custom toolchain, which cannot use `rustup component add`\n\
        help: if you built this toolchain from source, and used `rustup toolchain link`, then you may be able to build the component with `x.py`".to_string();
    }
    match component_for_bin(bin) {
        Some(c) => format!("\nTo install, run `rustup component add {}{}`", c, {
            if is_default {
                String::new()
            } else {
                format!(" --toolchain {toolchain}")
            }
        }),
        None => String::new(),
    }
}
/// Newtype hosting functions that apply to both custom and distributable toolchains that are installed.
pub struct InstalledCommonToolchain<'a>(&'a Toolchain<'a>);

impl<'a> InstalledCommonToolchain<'a> {
    pub fn create_command<T: AsRef<OsStr>>(&self, binary: T) -> Result<Command> {
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

        let bin_path = self.0.path.join("bin").join(&binary);
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
                if let Ok(distributable) = DistributableToolchain::new(self.0) {
                    if let (Some(component_name), Ok(component_statuses), Ok(Some(manifest))) = (
                        component_for_bin(&binary_lossy),
                        distributable.list_components(),
                        distributable.get_manifest(),
                    ) {
                        let component_status = component_statuses
                            .iter()
                            .find(|cs| cs.component.short_name(&manifest) == component_name)
                            .unwrap_or_else(|| {
                                panic!("component {component_name} should be in the manifest")
                            });
                        if !component_status.available {
                            return Err(anyhow!(format!(
                                "the '{}' component which provides the command '{}' is not available for the '{}' toolchain", component_status.component.short_name(&manifest), binary_lossy, self.0.name)));
                        }
                        if component_status.installed {
                            return Err(anyhow!(format!(
                                "the '{}' binary, normally provided by the '{}' component, is not applicable to the '{}' toolchain", binary_lossy, component_status.component.short_name(&manifest), self.0.name)));
                        }
                    }
                }
                let defaults = self.0.cfg.get_default()?;
                return Err(anyhow!(format!(
                    "'{}' is not installed for the toolchain '{}'{}",
                    binary.to_string_lossy(),
                    self.0.name,
                    install_msg(
                        &binary.to_string_lossy(),
                        &self.0.name,
                        Some(&self.0.name) == defaults.as_ref()
                    )
                )));
            }
            Path::new(&binary)
        };
        let mut cmd = Command::new(path);
        self.set_env(&mut cmd);
        Ok(cmd)
    }

    fn set_env(&self, cmd: &mut Command) {
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

        cmd.env("RUSTUP_TOOLCHAIN", &self.0.name);
        cmd.env("RUSTUP_HOME", &self.0.cfg.rustup_dir);
    }

    fn set_ldpath(&self, cmd: &mut Command) {
        let mut new_path = vec![self.0.path.join("lib")];

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
                path_entries.push(self.0.path.join("bin"));
            }
        }

        env_var::prepend_path("PATH", path_entries, cmd);
    }
}

/// Newtype to facilitate splitting out custom-toolchain specific code.
pub struct CustomToolchain<'a>(&'a Toolchain<'a>);

impl<'a> CustomToolchain<'a> {
    pub fn new(toolchain: &'a Toolchain<'a>) -> Result<CustomToolchain<'a>> {
        if toolchain.is_custom() {
            Ok(CustomToolchain(toolchain))
        } else {
            Err(anyhow!(format!(
                "{} is not a custom toolchain",
                toolchain.name()
            )))
        }
    }

    // Not installed only.
    pub fn install_from_dir(&self, src: &Path, link: bool) -> Result<()> {
        let mut pathbuf = PathBuf::from(src);

        pathbuf.push("lib");
        utils::assert_is_directory(&pathbuf)?;
        pathbuf.pop();
        pathbuf.push("bin");
        utils::assert_is_directory(&pathbuf)?;
        pathbuf.push(format!("rustc{EXE_SUFFIX}"));
        utils::assert_is_file(&pathbuf)?;

        if link {
            InstallMethod::Link(&utils::to_absolute(src)?, self).install(self.0)?;
        } else {
            InstallMethod::Copy(src, self).install(self.0)?;
        }

        Ok(())
    }
}

impl<'a> InstalledToolchain<'a> for CustomToolchain<'a> {
    fn installed_paths(&self) -> Result<Vec<InstalledPath<'a>>> {
        let path = &self.0.path;
        Ok(vec![InstalledPath::Dir { path }])
    }
}

/// Newtype to facilitate splitting out distributable-toolchain specific code.
pub struct DistributableToolchain<'a>(&'a Toolchain<'a>);

impl<'a> DistributableToolchain<'a> {
    pub fn new(toolchain: &'a Toolchain<'a>) -> Result<DistributableToolchain<'a>> {
        if toolchain.is_custom() {
            Err(anyhow!(format!(
                "{} is a custom toolchain",
                toolchain.name()
            )))
        } else {
            Ok(DistributableToolchain(toolchain))
        }
    }

    /// Temporary helper until we further split this into a newtype for
    /// InstalledDistributableToolchain - one where the type can protect component operations.
    pub fn new_for_components(toolchain: &'a Toolchain<'a>) -> Result<DistributableToolchain<'a>> {
        DistributableToolchain::new(toolchain).context(RustupError::ComponentsUnsupported(
            toolchain.name().to_string(),
        ))
    }

    // Installed only.
    pub(crate) fn add_component(&self, mut component: Component) -> Result<()> {
        if let Some(desc) = self.get_toolchain_desc_with_manifest()? {
            // Rename the component if necessary.
            if let Some(c) = desc.manifest.rename_component(&component) {
                component = c;
            }

            // Validate the component name
            let rust_pkg = desc
                .manifest
                .packages
                .get("rust")
                .expect("manifest should contain a rust package");
            let targ_pkg = rust_pkg
                .targets
                .get(&desc.toolchain.target)
                .expect("installed manifest should have a known target");

            if !targ_pkg.components.contains(&component) {
                let wildcard_component = component.wildcard();
                if targ_pkg.components.contains(&wildcard_component) {
                    component = wildcard_component;
                } else {
                    return Err(RustupError::UnknownComponent {
                        name: self.0.name.to_string(),
                        component: component.description(&desc.manifest),
                        suggestion: self.get_component_suggestion(
                            &component,
                            &desc.manifest,
                            false,
                        ),
                    }
                    .into());
                }
            }

            let changes = Changes {
                explicit_add_components: vec![component],
                remove_components: vec![],
            };

            desc.manifestation.update(
                &desc.manifest,
                changes,
                false,
                &self.download_cfg(),
                &self.download_cfg().notify_handler,
                &desc.toolchain.manifest_name(),
                false,
            )?;

            Ok(())
        } else {
            Err(RustupError::MissingManifest {
                name: self.0.name.to_string(),
            }
            .into())
        }
    }

    // Create a command as a fallback for another toolchain. This is used
    // to give custom toolchains access to cargo
    // Installed only.
    pub fn create_fallback_command<T: AsRef<OsStr>>(
        &self,
        binary: T,
        primary_toolchain: &Toolchain<'_>,
    ) -> Result<Command> {
        // With the hacks below this only works for cargo atm
        assert!(binary.as_ref() == "cargo" || binary.as_ref() == "cargo.exe");

        if !self.0.exists() {
            return Err(RustupError::ToolchainNotInstalled(self.0.name.to_owned()).into());
        }
        let installed_primary = primary_toolchain.as_installed_common()?;

        let src_file = self.0.path.join("bin").join(format!("cargo{EXE_SUFFIX}"));

        // MAJOR HACKS: Copy cargo.exe to its own directory on windows before
        // running it. This is so that the fallback cargo, when it in turn runs
        // rustc.exe, will run the rustc.exe out of the PATH environment
        // variable, _not_ the rustc.exe sitting in the same directory as the
        // fallback. See the `fallback_cargo_calls_correct_rustc` test case and
        // PR 812.
        //
        // On Windows, spawning a process will search the running application's
        // directory for the exe to spawn before searching PATH, and we don't want
        // it to do that, because cargo's directory contains the _wrong_ rustc. See
        // the documentation for the lpCommandLine argument of CreateProcess.
        let exe_path = if cfg!(windows) {
            use std::fs;
            let fallback_dir = self.0.cfg.rustup_dir.join("fallback");
            fs::create_dir_all(&fallback_dir)
                .context("unable to create dir to hold fallback exe")?;
            let fallback_file = fallback_dir.join("cargo.exe");
            if fallback_file.exists() {
                fs::remove_file(&fallback_file).context("unable to unlink old fallback exe")?;
            }
            fs::hard_link(&src_file, &fallback_file).context("unable to hard link fallback exe")?;
            fallback_file
        } else {
            src_file
        };
        let mut cmd = Command::new(exe_path);
        installed_primary.set_env(&mut cmd); // set up the environment to match rustc, not cargo
        cmd.env("RUSTUP_TOOLCHAIN", &primary_toolchain.name);
        Ok(cmd)
    }

    // Installed and not-installed?
    pub(crate) fn desc(&self) -> Result<ToolchainDesc> {
        ToolchainDesc::from_str(&self.0.name)
    }

    fn download_cfg(&self) -> DownloadCfg<'_> {
        self.0.cfg.download_cfg(&*self.0.dist_handler)
    }

    // Installed only?
    fn get_component_suggestion(
        &self,
        component: &Component,
        manifest: &Manifest,
        only_installed: bool,
    ) -> Option<String> {
        use strsim::damerau_levenshtein;

        // Suggest only for very small differences
        // High number can result in inaccurate suggestions for short queries e.g. `rls`
        const MAX_DISTANCE: usize = 3;

        let components = self.list_components();
        if let Ok(components) = components {
            let short_name_distance = components
                .iter()
                .filter(|c| !only_installed || c.installed)
                .map(|c| {
                    (
                        damerau_levenshtein(
                            &c.component.name(manifest)[..],
                            &component.name(manifest)[..],
                        ),
                        c,
                    )
                })
                .min_by_key(|t| t.0)
                .expect("There should be always at least one component");

            let long_name_distance = components
                .iter()
                .filter(|c| !only_installed || c.installed)
                .map(|c| {
                    (
                        damerau_levenshtein(
                            &c.component.name_in_manifest()[..],
                            &component.name(manifest)[..],
                        ),
                        c,
                    )
                })
                .min_by_key(|t| t.0)
                .expect("There should be always at least one component");

            let mut closest_distance = short_name_distance;
            let mut closest_match = short_name_distance.1.component.short_name(manifest);

            // Find closer suggestion
            if short_name_distance.0 > long_name_distance.0 {
                closest_distance = long_name_distance;

                // Check if only targets differ
                if closest_distance.1.component.short_name_in_manifest()
                    == component.short_name_in_manifest()
                {
                    closest_match = long_name_distance.1.component.target();
                } else {
                    closest_match = long_name_distance
                        .1
                        .component
                        .short_name_in_manifest()
                        .to_string();
                }
            } else {
                // Check if only targets differ
                if closest_distance.1.component.short_name(manifest)
                    == component.short_name(manifest)
                {
                    closest_match = short_name_distance.1.component.target();
                }
            }

            // If suggestion is too different don't suggest anything
            if closest_distance.0 > MAX_DISTANCE {
                None
            } else {
                Some(closest_match)
            }
        } else {
            None
        }
    }

    // Installed only.
    pub(crate) fn get_manifest(&self) -> Result<Option<Manifest>> {
        Ok(self.get_toolchain_desc_with_manifest()?.map(|d| d.manifest))
    }

    // Not installed only?
    pub(crate) fn install_from_dist(
        &self,
        force_update: bool,
        allow_downgrade: bool,
        components: &[&str],
        targets: &[&str],
        profile: Option<Profile>,
    ) -> Result<UpdateStatus> {
        let update_hash = self.update_hash()?;
        let old_date = self.get_manifest().ok().and_then(|m| m.map(|m| m.date));
        InstallMethod::Dist {
            desc: &self.desc()?,
            profile: profile
                .map(Ok)
                .unwrap_or_else(|| self.0.cfg.get_profile())?,
            update_hash: Some(&update_hash),
            dl_cfg: self.download_cfg(),
            force_update,
            allow_downgrade,
            exists: self.0.exists(),
            old_date: old_date.as_deref(),
            components,
            targets,
        }
        .install(self.0)
    }

    // Installed or not installed.
    pub fn install_from_dist_if_not_installed(&self) -> Result<UpdateStatus> {
        let update_hash = self.update_hash()?;
        (self.0.cfg.notify_handler)(Notification::LookingForToolchain(&self.0.name));
        if !self.0.exists() {
            Ok(InstallMethod::Dist {
                desc: &self.desc()?,
                profile: self.0.cfg.get_profile()?,
                update_hash: Some(&update_hash),
                dl_cfg: self.download_cfg(),
                force_update: false,
                allow_downgrade: false,
                exists: false,
                old_date: None,
                components: &[],
                targets: &[],
            }
            .install(self.0)?)
        } else {
            (self.0.cfg.notify_handler)(Notification::UsingExistingToolchain(&self.0.name));
            Ok(UpdateStatus::Unchanged)
        }
    }

    pub(crate) fn get_toolchain_desc_with_manifest(
        &self,
    ) -> Result<Option<ToolchainDescWithManifest>> {
        if !self.0.exists() {
            bail!(RustupError::ToolchainNotInstalled(self.0.name.to_owned()));
        }
        let toolchain = &self.0.name;
        let toolchain = ToolchainDesc::from_str(toolchain)
            .context(RustupError::ComponentsUnsupported(self.0.name.to_string()))?;

        let prefix = InstallPrefix::from(self.0.path.to_owned());
        let manifestation = Manifestation::open(prefix, toolchain.target.clone())?;
        Ok(manifestation
            .load_manifest()?
            .map(|manifest| ToolchainDescWithManifest {
                toolchain,
                manifestation,
                manifest,
            }))
    }

    pub fn list_components(&self) -> Result<Vec<ComponentStatus>> {
        if let Some(toolchain) = self.get_toolchain_desc_with_manifest()? {
            toolchain.list_components()
        } else {
            Err(RustupError::ComponentsUnsupported(self.0.name.to_string()).into())
        }
    }

    // Installed only.
    pub(crate) fn remove_component(&self, mut component: Component) -> Result<()> {
        if let Some(desc) = self.get_toolchain_desc_with_manifest()? {
            // Rename the component if necessary.
            if let Some(c) = desc.manifest.rename_component(&component) {
                component = c;
            }

            let dist_config = desc.manifestation.read_config()?.unwrap();
            if !dist_config.components.contains(&component) {
                let wildcard_component = component.wildcard();
                if dist_config.components.contains(&wildcard_component) {
                    component = wildcard_component;
                } else {
                    return Err(RustupError::UnknownComponent {
                        name: self.0.name.to_string(),
                        component: component.description(&desc.manifest),
                        suggestion: self.get_component_suggestion(&component, &desc.manifest, true),
                    }
                    .into());
                }
            }

            let changes = Changes {
                explicit_add_components: vec![],
                remove_components: vec![component],
            };

            desc.manifestation.update(
                &desc.manifest,
                changes,
                false,
                &self.download_cfg(),
                &self.download_cfg().notify_handler,
                &desc.toolchain.manifest_name(),
                false,
            )?;

            Ok(())
        } else {
            Err(RustupError::MissingManifest {
                name: self.0.name.to_string(),
            }
            .into())
        }
    }

    // Installed only.
    pub fn show_dist_version(&self) -> Result<Option<String>> {
        let update_hash = self.update_hash()?;

        match crate::dist::dist::dl_v2_manifest(
            self.download_cfg(),
            Some(&update_hash),
            &self.desc()?,
        )? {
            Some((manifest, _)) => Ok(Some(manifest.get_rust_version()?.to_string())),
            None => Ok(None),
        }
    }

    // Installed only.
    pub fn show_version(&self) -> Result<Option<String>> {
        match self.get_manifest()? {
            Some(manifest) => Ok(Some(manifest.get_rust_version()?.to_string())),
            None => Ok(None),
        }
    }

    // Installed only.
    fn update_hash(&self) -> Result<PathBuf> {
        self.0.cfg.get_hash_file(&self.0.name, true)
    }

    // Installed only.
    pub fn guess_v1_manifest(&self) -> bool {
        let prefix = InstallPrefix::from(self.0.path().to_owned());
        // If all the v1 common components are present this is likely to be
        // a v1 manifest install.  The v1 components are not called the same
        // in a v2 install.
        for component in V1_COMMON_COMPONENT_LIST {
            let manifest = format!("manifest-{component}");
            let manifest_path = prefix.manifest_file(&manifest);
            if !utils::path_exists(manifest_path) {
                return false;
            }
        }
        // It's reasonable to assume this is a v1 manifest installation
        true
    }
}

/// Helper type to avoid parsing a manifest more than once
pub(crate) struct ToolchainDescWithManifest {
    toolchain: ToolchainDesc,
    manifestation: Manifestation,
    pub manifest: Manifest,
}

impl ToolchainDescWithManifest {
    pub(crate) fn list_components(&self) -> Result<Vec<ComponentStatus>> {
        let config = self.manifestation.read_config()?;

        // Return all optional components of the "rust" package for the
        // toolchain's target triple.
        let mut res = Vec::new();

        let rust_pkg = self
            .manifest
            .packages
            .get("rust")
            .expect("manifest should contain a rust package");
        let targ_pkg = rust_pkg
            .targets
            .get(&self.toolchain.target)
            .expect("installed manifest should have a known target");

        for component in &targ_pkg.components {
            let installed = config
                .as_ref()
                .map(|c| component.contained_within(&c.components))
                .unwrap_or(false);

            let component_target = TargetTriple::new(&component.target());

            // Get the component so we can check if it is available
            let component_pkg = self
                .manifest
                .get_package(component.short_name_in_manifest())
                .unwrap_or_else(|_| {
                    panic!(
                        "manifest should contain component {}",
                        &component.short_name(&self.manifest)
                    )
                });
            let component_target_pkg = component_pkg
                .targets
                .get(&component_target)
                .expect("component should have target toolchain");

            res.push(ComponentStatus {
                component: component.clone(),
                name: component.name(&self.manifest),
                installed,
                available: component_target_pkg.available(),
            });
        }

        res.sort_by(|a, b| a.component.cmp(&b.component));

        Ok(res)
    }
}

impl<'a> InstalledToolchain<'a> for DistributableToolchain<'a> {
    fn installed_paths(&self) -> Result<Vec<InstalledPath<'a>>> {
        let path = &self.0.path;
        Ok(vec![
            InstalledPath::File {
                name: "update hash",
                path: self.update_hash()?,
            },
            InstalledPath::Dir { path },
        ])
    }
}
