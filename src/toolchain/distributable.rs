#[cfg(windows)]
use std::fs;
use std::{convert::Infallible, env::consts::EXE_SUFFIX, ffi::OsStr, path::Path, process::Command};

use anyhow::anyhow;
#[cfg(windows)]
use anyhow::Context;

use crate::{
    component_for_bin,
    config::Cfg,
    dist::{
        config::Config,
        manifest::{Component, ComponentStatus, Manifest},
        manifestation::{Changes, Manifestation},
        prefix::InstallPrefix,
        DistOptions, PartialToolchainDesc, Profile, ToolchainDesc,
    },
    install::{InstallMethod, UpdateStatus},
    RustupError,
};

use super::{
    names::{LocalToolchainName, ToolchainName},
    Toolchain,
};

/// An official toolchain installed on the local disk
#[derive(Debug)]
pub(crate) struct DistributableToolchain<'a> {
    pub(super) toolchain: Toolchain<'a>,
    desc: ToolchainDesc,
}

impl<'a> DistributableToolchain<'a> {
    pub(crate) fn from_partial(
        toolchain: Option<PartialToolchainDesc>,
        cfg: &'a Cfg<'a>,
    ) -> anyhow::Result<Self> {
        Ok(Self::try_from(&cfg.toolchain_from_partial(toolchain)?)?)
    }

    pub(crate) fn new(cfg: &'a Cfg<'a>, desc: ToolchainDesc) -> Result<Self, RustupError> {
        Toolchain::new(cfg, (&desc).into()).map(|toolchain| Self { toolchain, desc })
    }

    pub(crate) fn desc(&self) -> &ToolchainDesc {
        &self.desc
    }

    pub(crate) async fn add_component(&self, mut component: Component) -> anyhow::Result<()> {
        // TODO: take multiple components?
        let manifestation = self.get_manifestation()?;
        let manifest = self.get_manifest()?;
        // Rename the component if necessary.
        if let Some(c) = manifest.rename_component(&component) {
            component = c;
        }

        // Validate the component name
        let rust_pkg = manifest
            .packages
            .get("rust")
            .expect("manifest should contain a rust package");
        let targ_pkg = rust_pkg
            .targets
            .get(&self.desc.target)
            .expect("installed manifest should have a known target");

        if !targ_pkg.components.contains(&component) {
            let wildcard_component = component.wildcard();
            if targ_pkg.components.contains(&wildcard_component) {
                component = wildcard_component;
            } else {
                let config = manifestation.read_config()?.unwrap_or_default();
                let suggestion =
                    self.get_component_suggestion(&component, &config, &manifest, false);
                // Check if the target is supported.
                if !targ_pkg
                    .components
                    .iter()
                    .any(|c| c.target() == component.target())
                {
                    return Err(RustupError::UnknownTarget {
                        desc: self.desc.clone(),
                        target: component.target.expect("component target should be known"),
                        suggestion,
                    }
                    .into());
                }
                return Err(RustupError::UnknownComponent {
                    desc: self.desc.clone(),
                    component: component.description(&manifest),
                    suggestion,
                }
                .into());
            }
        }

        let changes = Changes {
            explicit_add_components: vec![component],
            remove_components: vec![],
        };

        let notify_handler =
            &|n: crate::dist::Notification<'_>| (self.toolchain.cfg.notify_handler)(n.into());
        let download_cfg = self.toolchain.cfg.download_cfg(&notify_handler);

        manifestation
            .update(
                &manifest,
                changes,
                false,
                &download_cfg,
                &self.desc.manifest_name(),
                false,
            )
            .await?;

        Ok(())
    }

    pub(crate) fn components(&self) -> anyhow::Result<Vec<ComponentStatus>> {
        let manifestation = self.get_manifestation()?;
        let config = manifestation.read_config()?.unwrap_or_default();
        let manifest = self.get_manifest()?;
        manifest.query_components(self.desc(), &config)
    }

    /// Are all the components installed in this distribution
    pub(crate) fn components_exist(
        &self,
        components: &[&str],
        targets: &[&str],
    ) -> anyhow::Result<bool> {
        let manifestation = self.get_manifestation()?;
        let manifest = manifestation.load_manifest()?;
        let manifest = match manifest {
            None => {
                // No manifest found. If this is a v1 install that's understandable
                // and we assume the components are all good, otherwise we need to
                // have a go at re-fetching the manifest to try again.
                return Ok(self.guess_v1_manifest());
            }
            Some(manifest) => manifest,
        };
        let config = manifestation.read_config()?.unwrap_or_default();
        let installed_components = manifest.query_components(&self.desc, &config)?;
        // check if all the components we want are installed
        let wanted_components = components.iter().all(|name| {
            installed_components.iter().any(|status| {
                let cname = status.component.short_name(&manifest);
                let cname = cname.as_str();
                let cnameim = status.component.short_name_in_manifest();
                let cnameim = cnameim.as_str();
                (cname == *name || cnameim == *name) && status.installed
            })
        });
        // And that all the targets we want are installed
        let wanted_targets = targets.iter().all(|name| {
            installed_components
                .iter()
                .filter(|c| c.component.short_name_in_manifest() == "rust-std")
                .any(|status| {
                    let ctarg = status.component.target();
                    (ctarg == *name) && status.installed
                })
        });
        Ok(wanted_components && wanted_targets)
    }

    /// Create a command as a fallback for another toolchain. This is used
    /// to give custom toolchains access to cargo
    pub fn create_fallback_command<T: AsRef<OsStr>>(
        &self,
        binary: T,
        installed_primary: &Toolchain<'_>,
    ) -> Result<Command, anyhow::Error> {
        // With the hacks below this only works for cargo atm
        let binary = binary.as_ref();
        assert!(binary == "cargo" || binary == "cargo.exe");

        let src_file = self
            .toolchain
            .path()
            .join("bin")
            .join(format!("cargo{EXE_SUFFIX}"));

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
        #[cfg(windows)]
        let exe_path = {
            let fallback_dir = self.toolchain.cfg.rustup_dir.join("fallback");
            fs::create_dir_all(&fallback_dir)
                .context("unable to create dir to hold fallback exe")?;
            let fallback_file = fallback_dir.join("cargo.exe");
            if fallback_file.exists() {
                fs::remove_file(&fallback_file).context("unable to unlink old fallback exe")?;
            }
            fs::hard_link(src_file, &fallback_file).context("unable to hard link fallback exe")?;
            fallback_file
        };
        #[cfg(not(windows))]
        let exe_path = src_file;

        let mut cmd = Command::new(exe_path);
        installed_primary.set_env(&mut cmd); // set up the environment to match rustc, not cargo
        cmd.env("RUSTUP_TOOLCHAIN", installed_primary.name().to_string());
        Ok(cmd)
    }

    fn get_component_suggestion(
        &self,
        component: &Component,
        config: &Config,
        manifest: &Manifest,
        only_installed: bool,
    ) -> Option<String> {
        use strsim::damerau_levenshtein;

        // Suggest only for very small differences
        // High number can result in inaccurate suggestions for short queries e.g. `rls`
        const MAX_DISTANCE: usize = 3;

        let components = manifest.query_components(&self.desc, config);
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

    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn get_manifestation(&self) -> anyhow::Result<Manifestation> {
        let prefix = InstallPrefix::from(self.toolchain.path());
        Manifestation::open(prefix, self.desc.target.clone())
    }

    /// Get the manifest associated with this distribution
    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn get_manifest(&self) -> anyhow::Result<Manifest> {
        self.get_manifestation()?
            .load_manifest()
            .transpose()
            .unwrap_or_else(|| match self.guess_v1_manifest() {
                true => Err(RustupError::ComponentsUnsupportedV1(self.desc.to_string()).into()),
                false => Err(RustupError::MissingManifest(self.desc.clone()).into()),
            })
    }

    /// Guess whether this is a V1 or V2 manifest distribution.
    pub(crate) fn guess_v1_manifest(&self) -> bool {
        InstallPrefix::from(self.toolchain.path().to_owned()).guess_v1_manifest()
    }

    #[tracing::instrument(level = "trace", err(level = "trace"), skip_all)]
    pub(crate) async fn install(
        cfg: &'a Cfg<'a>,
        toolchain: &ToolchainDesc,
        components: &[&str],
        targets: &[&str],
        profile: Profile,
        force: bool,
    ) -> anyhow::Result<(UpdateStatus, DistributableToolchain<'a>)> {
        let hash_path = cfg.get_hash_file(toolchain, true)?;
        let update_hash = Some(&hash_path as &Path);

        let status = InstallMethod::Dist(DistOptions {
            cfg,
            toolchain,
            profile,
            update_hash,
            dl_cfg: cfg.download_cfg(&|n| (cfg.notify_handler)(n.into())),
            force,
            allow_downgrade: false,
            exists: false,
            old_date_version: None,
            components,
            targets,
        })
        .install()
        .await?;
        Ok((status, Self::new(cfg, toolchain.clone())?))
    }

    #[tracing::instrument(level = "trace", err(level = "trace"), skip_all)]
    pub(crate) async fn update(
        &mut self,
        components: &[&str],
        targets: &[&str],
        profile: Profile,
    ) -> anyhow::Result<UpdateStatus> {
        self.update_extra(components, targets, profile, true, false)
            .await
    }

    /// Update a toolchain with control over the channel behaviour
    #[tracing::instrument(level = "trace", err(level = "trace"), skip_all)]
    pub(crate) async fn update_extra(
        &mut self,
        components: &[&str],
        targets: &[&str],
        profile: Profile,
        force: bool,
        allow_downgrade: bool,
    ) -> anyhow::Result<UpdateStatus> {
        let old_date_version =
            // Ignore a missing manifest: we can't report the old version
            // correctly, and it probably indicates an incomplete install, so do
            // not report an old rustc version either.
            self.get_manifest()
                .map(|m| {
                    (
                        m.date,
                        // should rustc_version be a free function on a trait?
                        // note that prev_version can be junk if the rustc component is missing ...
                        self.toolchain.rustc_version(),
                    )
                })
                .ok();

        let cfg = self.toolchain.cfg;
        let hash_path = cfg.get_hash_file(&self.desc, true)?;
        let update_hash = Some(&hash_path as &Path);

        InstallMethod::Dist(DistOptions {
            cfg,
            toolchain: &self.desc,
            profile,
            update_hash,
            dl_cfg: cfg.download_cfg(&|n| (cfg.notify_handler)(n.into())),
            force,
            allow_downgrade,
            exists: true,
            old_date_version,
            components,
            targets,
        })
        .install()
        .await
    }

    pub fn recursion_error(&self, binary_lossy: String) -> Result<Infallible, anyhow::Error> {
        let prefix = InstallPrefix::from(self.toolchain.path());
        let manifestation = Manifestation::open(prefix, self.desc.target.clone())?;
        let manifest = self.get_manifest()?;
        let config = manifestation.read_config()?.unwrap_or_default();
        let component_statuses = manifest.query_components(&self.desc, &config)?;
        let desc = &self.desc;
        if let Some(component_name) = component_for_bin(&binary_lossy) {
            let component_status = component_statuses
                .iter()
                .find(|cs| cs.component.short_name(&manifest) == component_name)
                .ok_or_else(|| anyhow!("component {component_name} should be in the manifest"))?;
            let short_name = component_status.component.short_name(&manifest);
            if !component_status.available {
                Err(anyhow!(
                                "the '{short_name}' component which provides the command '{binary_lossy}' is not available for the '{desc}' toolchain"))
            } else if component_status.installed {
                Err(anyhow!(
                    "the '{binary_lossy}' binary, normally provided by the '{short_name}' component, is not applicable to the '{desc}' toolchain"))
            } else {
                // available, not installed, recommend installation
                let selector = match self.toolchain.cfg.get_default()? {
                    Some(ToolchainName::Official(n)) if n == self.desc => String::new(),
                    _ => format!("--toolchain {} ", self.toolchain.name()),
                };
                Err(anyhow!("'{binary_lossy}' is not installed for the toolchain '{desc}'.\nTo install, run `rustup component add {selector}{component_name}`"))
            }
        } else {
            // Unknown binary - no component to recommend
            Err(anyhow!(
                "Unknown binary '{binary_lossy}' in official toolchain '{desc}'."
            ))
        }
    }

    pub(crate) async fn remove_component(&self, mut component: Component) -> anyhow::Result<()> {
        // TODO: take multiple components?
        let manifestation = self.get_manifestation()?;
        let config = manifestation.read_config()?.unwrap_or_default();
        let manifest = self.get_manifest()?;

        // Rename the component if necessary.
        if let Some(c) = manifest.rename_component(&component) {
            component = c;
        }

        if !config.components.contains(&component) {
            let wildcard_component = component.wildcard();
            if config.components.contains(&wildcard_component) {
                component = wildcard_component;
            } else {
                let suggestion =
                    self.get_component_suggestion(&component, &config, &manifest, true);
                // Check if the target is installed.
                if !config
                    .components
                    .iter()
                    .any(|c| c.target() == component.target())
                {
                    return Err(RustupError::TargetNotInstalled {
                        desc: self.desc.clone(),
                        target: component.target.expect("component target should be known"),
                        suggestion,
                    }
                    .into());
                }
                return Err(RustupError::UnknownComponent {
                    desc: self.desc.clone(),
                    component: component.description(&manifest),
                    suggestion,
                }
                .into());
            }
        }

        let changes = Changes {
            explicit_add_components: vec![],
            remove_components: vec![component],
        };

        let notify_handler =
            &|n: crate::dist::Notification<'_>| (self.toolchain.cfg.notify_handler)(n.into());
        let download_cfg = self.toolchain.cfg.download_cfg(&notify_handler);

        manifestation
            .update(
                &manifest,
                changes,
                false,
                &download_cfg,
                &self.desc.manifest_name(),
                false,
            )
            .await?;

        Ok(())
    }

    pub async fn show_dist_version(&self) -> anyhow::Result<Option<String>> {
        let update_hash = self.toolchain.cfg.get_hash_file(&self.desc, false)?;
        let notify_handler =
            &|n: crate::dist::Notification<'_>| (self.toolchain.cfg.notify_handler)(n.into());
        let download_cfg = self.toolchain.cfg.download_cfg(&notify_handler);

        match crate::dist::dl_v2_manifest(download_cfg, Some(&update_hash), &self.desc).await? {
            Some((manifest, _)) => Ok(Some(manifest.get_rust_version()?.to_string())),
            None => Ok(None),
        }
    }

    pub fn show_version(&self) -> anyhow::Result<Option<String>> {
        match self.get_manifestation()?.load_manifest()? {
            Some(manifest) => Ok(Some(manifest.get_rust_version()?.to_string())),
            None => Ok(None),
        }
    }
}

impl<'a> TryFrom<&Toolchain<'a>> for DistributableToolchain<'a> {
    type Error = RustupError;

    fn try_from(value: &Toolchain<'a>) -> Result<Self, Self::Error> {
        match value.name() {
            LocalToolchainName::Named(ToolchainName::Official(desc)) => Ok(Self {
                toolchain: value.clone(),
                desc: desc.clone(),
            }),
            n => Err(RustupError::ComponentsUnsupported(n.to_string())),
        }
    }
}

impl<'a> From<DistributableToolchain<'a>> for Toolchain<'a> {
    fn from(value: DistributableToolchain<'a>) -> Self {
        value.toolchain
    }
}
