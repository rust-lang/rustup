use std::{
    env::consts::EXE_SUFFIX,
    path::{Path, PathBuf},
};

use crate::{config::Cfg, install::InstallMethod, utils::utils};

use super::{names::CustomToolchainName, toolchain::InstalledPath};

/// An official toolchain installed on the local disk
#[derive(Debug)]
pub(crate) struct CustomToolchain;

impl CustomToolchain {
    pub(crate) fn install_from_dir(
        cfg: &Cfg,
        src: &Path,
        dest: &CustomToolchainName,
        link: bool,
    ) -> anyhow::Result<Self> {
        let mut pathbuf = PathBuf::from(src);

        pathbuf.push("lib");
        utils::assert_is_directory(&pathbuf)?;
        pathbuf.pop();
        pathbuf.push("bin");
        utils::assert_is_directory(&pathbuf)?;
        pathbuf.push(format!("rustc{EXE_SUFFIX}"));
        utils::assert_is_file(&pathbuf)?;

        if link {
            InstallMethod::Link {
                src: &utils::to_absolute(src)?,
                dest,
                cfg,
            }
            .install()?;
        } else {
            InstallMethod::Copy { src, dest, cfg }.install()?;
        }
        Ok(Self)
    }

    pub(crate) fn installed_paths(path: &Path) -> anyhow::Result<Vec<InstalledPath<'_>>> {
        Ok(vec![InstalledPath::Dir { path }])
    }
}
