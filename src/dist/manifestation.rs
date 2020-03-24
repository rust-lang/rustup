//! Maintains a Rust installation by installing individual Rust
//! platform components from a distribution server.

use crate::config::PgpPublicKey;
use crate::dist::component::{Components, Package, TarGzPackage, TarXzPackage, Transaction};
use crate::dist::config::Config;
use crate::dist::dist::{Profile, TargetTriple, DEFAULT_DIST_SERVER};
use crate::dist::download::{DownloadCfg, File};
use crate::dist::manifest::{Component, Manifest, TargetedPackage};
use crate::dist::notifications::*;
use crate::dist::prefix::InstallPrefix;
use crate::dist::temp;
use crate::errors::*;
use crate::utils::utils;
use retry::delay::NoDelay;
use retry::{retry, OperationResult};
use std::path::Path;

pub const DIST_MANIFEST: &str = "multirust-channel-manifest.toml";
pub const CONFIG_FILE: &str = "multirust-config.toml";

enum Format {
    Gz,
    Xz,
}

#[derive(Debug)]
pub struct Manifestation {
    installation: Components,
    target_triple: TargetTriple,
}

#[derive(Debug)]
pub struct Changes {
    pub explicit_add_components: Vec<Component>,
    pub remove_components: Vec<Component>,
}

impl Changes {
    pub fn none() -> Self {
        Self {
            explicit_add_components: Vec::new(),
            remove_components: Vec::new(),
        }
    }

    fn iter_add_components(&self) -> impl Iterator<Item = &Component> {
        self.explicit_add_components.iter()
    }

    fn check_invariants(&self, config: &Option<Config>) {
        for component_to_add in self.iter_add_components() {
            assert!(
                !self.remove_components.contains(component_to_add),
                "can't both add and remove components"
            );
        }
        for component_to_remove in &self.remove_components {
            let config = config
                .as_ref()
                .expect("removing component on fresh install?");
            assert!(
                config.components.contains(component_to_remove),
                "removing package that isn't installed"
            );
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum UpdateStatus {
    Changed,
    Unchanged,
}

impl Manifestation {
    /// Open the install prefix for updates from a distribution
    /// channel.  The install prefix directory does not need to exist;
    /// it will be created as needed. If there's an existing install
    /// then the rust-install installation format will be verified. A
    /// bad installer version is the only reason this will fail.
    pub fn open(prefix: InstallPrefix, triple: TargetTriple) -> Result<Self> {
        // TODO: validate the triple with the existing install as well
        // as the metadata format of the existing install
        Ok(Self {
            installation: Components::open(prefix)?,
            target_triple: triple,
        })
    }

    /// Install or update from a given channel manifest, while
    /// selecting extension components to add or remove.
    ///
    /// `update` takes a manifest describing a release of Rust (which
    /// may be either a freshly-downloaded one, or the same one used
    /// for the previous install), as well as lists of extension
    /// components to add and remove.

    /// From that it schedules a list of components to install and
    /// to uninstall to bring the installation up to date.  It
    /// downloads the components' packages. Then in a Transaction
    /// uninstalls old packages and installs new packages, writes the
    /// distribution manifest to "rustlib/rustup-dist.toml" and a
    /// configuration containing the component name-target pairs to
    /// "rustlib/rustup-config.toml".
    pub fn update(
        &self,
        new_manifest: &Manifest,
        changes: Changes,
        force_update: bool,
        download_cfg: &DownloadCfg<'_>,
        notify_handler: &dyn Fn(Notification<'_>),
        toolchain_str: &str,
        implicit_modify: bool,
    ) -> anyhow::Result<UpdateStatus> {
        // Some vars we're going to need a few times
        let temp_cfg = download_cfg.temp_cfg;
        let prefix = self.installation.prefix();
        let rel_installed_manifest_path = prefix.rel_manifest_file(DIST_MANIFEST);
        let installed_manifest_path = prefix.path().join(&rel_installed_manifest_path);

        // Create the lists of components needed for installation
        let config = SyncError::maybe(self.read_config())?;
        let mut update = SyncError::maybe(Update::build_update(
            self,
            new_manifest,
            &changes,
            &config,
            notify_handler,
        ))?;

        if update.nothing_changes() {
            return Ok(UpdateStatus::Unchanged);
        }

        // Validate that the requested components are available
        match update.unavailable_components(new_manifest, toolchain_str) {
            Ok(_) => {}
            Err(e) => {
                if force_update {
                    if let Some(RustupError::RequestedComponentsUnavailable {
                        components, ..
                    }) = e.downcast_ref::<RustupError>()
                    {
                        for component in components {
                            notify_handler(Notification::ForcingUnavailableComponent(
                                component.name(new_manifest).as_str(),
                            ));
                        }
                        update.drop_components_to_install(&components);
                    }
                } else {
                    return Err(e);
                }
            }
        }

        let altered = temp_cfg.dist_server != DEFAULT_DIST_SERVER;

        // Download component packages and validate hashes
        let mut things_to_install: Vec<(Component, Format, File)> = Vec::new();
        let mut things_downloaded: Vec<String> = Vec::new();
        let components = SyncError::maybe(update.components_urls_and_hashes(new_manifest))?;

        const DEFAULT_MAX_RETRIES: usize = 3;
        let max_retries: usize = std::env::var("RUSTUP_MAX_RETRIES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_MAX_RETRIES);

        for (component, format, url, hash) in components {
            notify_handler(Notification::DownloadingComponent(
                &component.short_name(new_manifest),
                &self.target_triple,
                component.target.as_ref(),
            ));
            let url = if altered {
                url.replace(DEFAULT_DIST_SERVER, temp_cfg.dist_server.as_str())
            } else {
                url
            };

            let url_url = SyncError::maybe(utils::parse_url(&url))?;

            let downloaded_file = retry(NoDelay.take(max_retries), || {
                match download_cfg.download(&url_url, &hash) {
                    Ok(f) => OperationResult::Ok(f),
                    Err(e) => match e.kind() {
                        // If there was a broken partial file, try again
                        ErrorKind::DownloadingFile { .. } | ErrorKind::BrokenPartialFile => {
                            notify_handler(Notification::RetryingDownload(&url));
                            OperationResult::Retry(e)
                        }

                        _ => OperationResult::Err(e),
                    },
                }
            })
            .map_err(|e| RustupError::ComponentDownloadFailed {
                component: component.name(new_manifest),
                source: SyncError::new(e),
            })?;

            things_downloaded.push(hash);

            things_to_install.push((component, format, downloaded_file));
        }

        // Begin transaction
        let mut tx = Transaction::new(prefix.clone(), temp_cfg, notify_handler);

        // If the previous installation was from a v1 manifest we need
        // to uninstall it first.
        tx = SyncError::maybe(self.maybe_handle_v2_upgrade(&config, tx))?;

        // Uninstall components
        for component in &update.components_to_uninstall {
            let notification = if implicit_modify {
                Notification::RemovingOldComponent
            } else {
                Notification::RemovingComponent
            };
            notify_handler(notification(
                &component.short_name(new_manifest),
                &self.target_triple,
                component.target.as_ref(),
            ));

            tx = SyncError::maybe(self.uninstall_component(
                &component,
                new_manifest,
                tx,
                &notify_handler,
            ))?;
        }

        // Install components
        for (component, format, installer_file) in things_to_install {
            // For historical reasons, the rust-installer component
            // names are not the same as the dist manifest component
            // names. Some are just the component name some are the
            // component name plus the target triple.
            let pkg_name = component.name_in_manifest();
            let short_pkg_name = component.short_name_in_manifest();
            let short_name = component.short_name(new_manifest);

            notify_handler(Notification::InstallingComponent(
                &short_name,
                &self.target_triple,
                component.target.as_ref(),
            ));

            let notification_converter = |notification: crate::utils::Notification<'_>| {
                notify_handler(notification.into());
            };
            let gz;
            let xz;
            let reader = SyncError::maybe(utils::FileReaderWithProgress::new_file(
                &installer_file,
                &notification_converter,
            ))?;
            let package: &dyn Package = match format {
                Format::Gz => {
                    gz = SyncError::maybe(TarGzPackage::new(
                        reader,
                        temp_cfg,
                        Some(&notification_converter),
                    ))?;
                    &gz
                }
                Format::Xz => {
                    xz = SyncError::maybe(TarXzPackage::new(
                        reader,
                        temp_cfg,
                        Some(&notification_converter),
                    ))?;
                    &xz
                }
            };

            // If the package doesn't contain the component that the
            // manifest says it does then somebody must be playing a joke on us.
            if !package.contains(&pkg_name, Some(&short_pkg_name)) {
                return Err(RustupError::CorruptComponent(short_name).into());
            }

            tx = SyncError::maybe(package.install(
                &self.installation,
                &pkg_name,
                Some(&short_pkg_name),
                tx,
            ))?;
        }

        // Install new distribution manifest
        let new_manifest_str = new_manifest.clone().stringify();
        SyncError::maybe(tx.modify_file(rel_installed_manifest_path))?;
        SyncError::maybe(utils::write_file(
            "manifest",
            &installed_manifest_path,
            &new_manifest_str,
        ))?;

        // Write configuration.
        //
        // NB: This configuration is mostly for keeping track of the name/target pairs
        // that identify installed components. The rust-installer metadata maintained by
        // `Components` *also* tracks what is installed, but it only tracks names, not
        // name/target. Needs to be fixed in rust-installer.
        let mut new_config = Config::new();
        new_config.components = update.final_component_list;
        let config_str = new_config.stringify();
        let rel_config_path = prefix.rel_manifest_file(CONFIG_FILE);
        let config_path = prefix.path().join(&rel_config_path);
        SyncError::maybe(tx.modify_file(rel_config_path))?;
        SyncError::maybe(utils::write_file("dist config", &config_path, &config_str))?;

        // End transaction
        tx.commit();

        SyncError::maybe(download_cfg.clean(&things_downloaded))?;

        Ok(UpdateStatus::Changed)
    }

    pub fn uninstall(
        &self,
        manifest: &Manifest,
        temp_cfg: &temp::Cfg,
        notify_handler: &dyn Fn(Notification<'_>),
    ) -> Result<()> {
        let prefix = self.installation.prefix();

        let mut tx = Transaction::new(prefix.clone(), temp_cfg, notify_handler);

        // Read configuration and delete it
        let rel_config_path = prefix.rel_manifest_file(CONFIG_FILE);
        let config_str = utils::read_file("dist config", &prefix.path().join(&rel_config_path))?;
        let config = Config::parse(&config_str)?;
        tx.remove_file("dist config", rel_config_path)?;

        for component in config.components {
            tx = self.uninstall_component(&component, manifest, tx, notify_handler)?;
        }
        tx.commit();

        Ok(())
    }

    fn uninstall_component<'a>(
        &self,
        component: &Component,
        manifest: &Manifest,
        mut tx: Transaction<'a>,
        notify_handler: &dyn Fn(Notification<'_>),
    ) -> Result<Transaction<'a>> {
        // For historical reasons, the rust-installer component
        // names are not the same as the dist manifest component
        // names. Some are just the component name some are the
        // component name plus the target triple.
        let name = component.name_in_manifest();
        let short_name = component.short_name_in_manifest();
        if let Some(c) = self.installation.find(&name)? {
            tx = c.uninstall(tx)?;
        } else if let Some(c) = self.installation.find(&short_name)? {
            tx = c.uninstall(tx)?;
        } else {
            notify_handler(Notification::MissingInstalledComponent(
                &component.short_name(manifest),
            ));
        }

        Ok(tx)
    }

    // Read the config file. Config files are presently only created
    // for v2 installations.
    pub fn read_config(&self) -> Result<Option<Config>> {
        let prefix = self.installation.prefix();
        let rel_config_path = prefix.rel_manifest_file(CONFIG_FILE);
        let config_path = prefix.path().join(rel_config_path);
        if utils::path_exists(&config_path) {
            let config_str = utils::read_file("dist config", &config_path)?;
            Ok(Some(Config::parse(&config_str)?))
        } else {
            Ok(None)
        }
    }

    pub fn load_manifest(&self) -> Result<Option<Manifest>> {
        let prefix = self.installation.prefix();
        let old_manifest_path = prefix.manifest_file(DIST_MANIFEST);
        if utils::path_exists(&old_manifest_path) {
            let manifest_str = utils::read_file("installed manifest", &old_manifest_path)?;
            Ok(Some(Manifest::parse(&manifest_str)?))
        } else {
            Ok(None)
        }
    }

    /// Installation using the legacy v1 manifest format
    pub fn update_v1(
        &self,
        new_manifest: &[String],
        update_hash: Option<&Path>,
        temp_cfg: &temp::Cfg,
        notify_handler: &dyn Fn(Notification<'_>),
        pgp_keys: &[PgpPublicKey],
    ) -> Result<Option<String>> {
        // If there's already a v2 installation then something has gone wrong
        if self.read_config()?.is_some() {
            return Err(
                "the server unexpectedly provided an obsolete version of the distribution manifest"
                    .into(),
            );
        }

        let url = new_manifest
            .iter()
            .find(|u| u.contains(&format!("{}{}", self.target_triple, ".tar.gz")));
        if url.is_none() {
            return Err(format!(
                "binary package was not provided for '{}'",
                self.target_triple.to_string()
            )
            .into());
        }
        // Only replace once. The cost is inexpensive.
        let url = url
            .unwrap()
            .replace(DEFAULT_DIST_SERVER, temp_cfg.dist_server.as_str());

        notify_handler(Notification::DownloadingComponent(
            "rust",
            &self.target_triple,
            Some(&self.target_triple),
        ));

        use std::path::PathBuf;
        let dld_dir = PathBuf::from("bogus");
        let dlcfg = DownloadCfg {
            dist_root: "bogus",
            download_dir: &dld_dir,
            temp_cfg,
            notify_handler,
            pgp_keys,
        };

        let dl = dlcfg.download_and_check(&url, update_hash, ".tar.gz")?;
        if dl.is_none() {
            return Ok(None);
        };
        let (installer_file, installer_hash) = dl.unwrap();

        let prefix = self.installation.prefix();

        notify_handler(Notification::InstallingComponent(
            "rust",
            &self.target_triple,
            Some(&self.target_triple),
        ));

        // Begin transaction
        let mut tx = Transaction::new(prefix, temp_cfg, notify_handler);

        // Uninstall components
        let components = self.installation.list()?;
        for component in components {
            tx = component.uninstall(tx)?;
        }

        // Install all the components in the installer
        let notification_converter = |notification: crate::utils::Notification<'_>| {
            notify_handler(notification.into());
        };
        let reader =
            utils::FileReaderWithProgress::new_file(&installer_file, &notification_converter)?;
        let package: &dyn Package =
            &TarGzPackage::new(reader, temp_cfg, Some(&notification_converter))?;

        for component in package.components() {
            tx = package.install(&self.installation, &component, None, tx)?;
        }

        // End transaction
        tx.commit();

        Ok(Some(installer_hash))
    }

    // If the previous installation was from a v1 manifest, then it
    // doesn't have a configuration or manifest-derived list of
    // component/target pairs. Uninstall it using the installer's
    // component list before upgrading.
    fn maybe_handle_v2_upgrade<'a>(
        &self,
        config: &Option<Config>,
        mut tx: Transaction<'a>,
    ) -> Result<Transaction<'a>> {
        let installed_components = self.installation.list()?;
        let looks_like_v1 = config.is_none() && !installed_components.is_empty();

        if !looks_like_v1 {
            return Ok(tx);
        }

        for component in installed_components {
            tx = component.uninstall(tx)?;
        }

        Ok(tx)
    }
}

#[derive(Debug)]
struct Update {
    components_to_uninstall: Vec<Component>,
    components_to_install: Vec<Component>,
    final_component_list: Vec<Component>,
    missing_components: Vec<Component>,
}

impl Update {
    /// Returns components to uninstall, install, and the list of all
    /// components that will be up to date after the update.
    fn build_update(
        manifestation: &Manifestation,
        new_manifest: &Manifest,
        changes: &Changes,
        config: &Option<Config>,
        notify_handler: &dyn Fn(Notification<'_>),
    ) -> Result<Self> {
        // The package to install.
        let rust_package = new_manifest.get_package("rust")?;
        let rust_target_package = rust_package.get_target(Some(&manifestation.target_triple))?;

        changes.check_invariants(&config);

        // The list of components already installed, empty if a new install
        let mut starting_list = config
            .as_ref()
            .map(|c| c.components.clone())
            .unwrap_or_default();

        let installed_components = manifestation.installation.list()?;
        let looks_like_v1 = config.is_none() && !installed_components.is_empty();
        if looks_like_v1 {
            let mut profile_components = new_manifest
                .get_profile_components(Profile::Default, &manifestation.target_triple)?;
            starting_list.append(&mut profile_components);
        }

        let mut result = Self {
            components_to_uninstall: vec![],
            components_to_install: vec![],
            final_component_list: vec![],
            missing_components: vec![],
        };

        // Find the final list of components we want to be left with when
        // we're done: required components, added components, and existing
        // installed components.
        result.build_final_component_list(
            &starting_list,
            rust_target_package,
            new_manifest,
            &changes,
        );

        // If this is a full upgrade then the list of components to
        // uninstall is all that are currently installed, and those
        // to install the final list. It's a complete reinstall.
        //
        // If it's a modification then the components to uninstall are
        // those that are currently installed but not in the final list.
        // To install are those on the final list but not already
        // installed.
        let old_manifest = manifestation.load_manifest()?;
        let just_modifying_existing_install = old_manifest.as_ref() == Some(new_manifest);

        if just_modifying_existing_install {
            for existing_component in &starting_list {
                if !result.final_component_list.contains(existing_component) {
                    result
                        .components_to_uninstall
                        .push(existing_component.clone())
                }
            }
            for component in &result.final_component_list {
                if !starting_list.contains(component) {
                    result.components_to_install.push(component.clone());
                } else if changes.explicit_add_components.contains(&component) {
                    notify_handler(Notification::ComponentAlreadyInstalled(
                        &component.description(new_manifest),
                    ));
                }
            }
        } else {
            result.components_to_uninstall = starting_list;
            result.components_to_install = result.final_component_list.clone();
        }

        Ok(result)
    }

    /// Build the list of components we'll have installed at the end
    fn build_final_component_list(
        &mut self,
        starting_list: &[Component],
        rust_target_package: &TargetedPackage,
        new_manifest: &Manifest,
        changes: &Changes,
    ) {
        // Add requested components
        for component in &changes.explicit_add_components {
            self.final_component_list.push(component.clone());
        }

        // Add components that are already installed
        for existing_component in starting_list {
            let removed = changes.remove_components.contains(existing_component);

            if !removed {
                // If there is a rename in the (new) manifest, then we uninstall the component with the
                // old name and install a component with the new name
                if let Some(renamed_component) = new_manifest.rename_component(&existing_component)
                {
                    let is_already_included =
                        self.final_component_list.contains(&renamed_component);
                    if !is_already_included {
                        self.final_component_list.push(renamed_component);
                    }
                } else {
                    let is_already_included =
                        self.final_component_list.contains(existing_component);
                    if !is_already_included {
                        let component_is_present =
                            rust_target_package.components.contains(existing_component);

                        if component_is_present {
                            self.final_component_list.push(existing_component.clone());
                        } else {
                            // Component not available, check if this is a case of
                            // where rustup brokenly installed `rust-src` during
                            // the 1.20.x series
                            if existing_component.contained_within(&rust_target_package.components)
                            {
                                // It is the case, so we need to create a fresh wildcard
                                // component using the package name and add it to the final
                                // component list
                                self.final_component_list
                                    .push(existing_component.wildcard());
                            } else {
                                self.missing_components.push(existing_component.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    fn nothing_changes(&self) -> bool {
        self.components_to_uninstall.is_empty() && self.components_to_install.is_empty()
    }

    fn unavailable_components(
        &self,
        new_manifest: &Manifest,
        toolchain_str: &str,
    ) -> anyhow::Result<()> {
        let mut unavailable_components: Vec<Component> = self
            .components_to_install
            .iter()
            .filter(|c| {
                use crate::dist::manifest::*;
                let pkg: Option<&Package> =
                    new_manifest.get_package(&c.short_name_in_manifest()).ok();
                let target_pkg: Option<&TargetedPackage> =
                    pkg.and_then(|p| p.get_target(c.target.as_ref()).ok());
                target_pkg.map(TargetedPackage::available) != Some(true)
            })
            .cloned()
            .collect();

        unavailable_components.extend_from_slice(&self.missing_components);

        if !unavailable_components.is_empty() {
            return Err(RustupError::RequestedComponentsUnavailable {
                components: unavailable_components,
                manifest: new_manifest.clone(),
                toolchain: toolchain_str.to_owned(),
            }
            .into());
        }

        Ok(())
    }

    fn drop_components_to_install(&mut self, to_drop: &[Component]) {
        let components: Vec<_> = self
            .components_to_install
            .drain(..)
            .filter(|c| !to_drop.contains(&c))
            .collect();
        self.components_to_install.extend(components);
        let final_components: Vec<_> = self
            .final_component_list
            .drain(..)
            .filter(|c| !to_drop.contains(&c))
            .collect();
        self.final_component_list = final_components;
    }

    /// Map components to urls and hashes
    fn components_urls_and_hashes(
        &self,
        new_manifest: &Manifest,
    ) -> Result<Vec<(Component, Format, String, String)>> {
        let mut components_urls_and_hashes = Vec::new();
        for component in &self.components_to_install {
            let package = new_manifest.get_package(&component.short_name_in_manifest())?;
            let target_package = package.get_target(component.target.as_ref())?;

            let bins = match target_package.bins {
                None => continue,
                Some(ref bins) => bins,
            };
            let c_u_h = if let (Some(url), Some(hash)) = (bins.xz_url.clone(), bins.xz_hash.clone())
            {
                (component.clone(), Format::Xz, url, hash)
            } else {
                (
                    component.clone(),
                    Format::Gz,
                    bins.url.clone(),
                    bins.hash.clone(),
                )
            };
            components_urls_and_hashes.push(c_u_h);
        }

        Ok(components_urls_and_hashes)
    }
}
