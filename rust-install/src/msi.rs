
use winapi::HKEY;
use winreg::{RegKey, EnumKeys};
use winreg::enums::*;

use std::io;
use std::iter;
use std::path::PathBuf;

pub struct Installers(RegKey);
pub struct AllInstallers(Installers, Installers);

pub struct Installer {
    id: String,
    key: RegKey,
}

impl Installer {
    pub fn product_id(&self) -> &str {
        &self.id
    }
    pub fn comments(&self) -> io::Result<String> {
        self.key.get_value("Comments")
    }
    pub fn display_name(&self) -> io::Result<String> {
        self.key.get_value("DisplayName")
    }
    pub fn display_version(&self) -> io::Result<String> {
        self.key.get_value("DisplayVersion")
    }
    pub fn estimated_size_kb(&self) -> io::Result<u32> {
        self.key.get_value("EstimatedSize")
    }
    pub fn install_date(&self) -> io::Result<String> {
        self.key.get_value("InstallDate")
    }
    pub fn install_location(&self) -> io::Result<PathBuf> {
        self.key.get_value("InstallLocation").map(|s: String| s.into())
    }
    pub fn install_source(&self) -> io::Result<PathBuf> {
        self.key.get_value("InstallSource").map(|s: String| s.into())
    }
    pub fn language(&self) -> io::Result<u32> {
        self.key.get_value("Language")
    }
    pub fn publisher(&self) -> io::Result<String> {
        self.key.get_value("Publisher")
    }
    pub fn url_info_about(&self) -> io::Result<String> {
        self.key.get_value("UrlInfoAbout")
    }
    pub fn version(&self) -> io::Result<u32> {
        self.key.get_value("Version")
    }
    pub fn version_major(&self) -> io::Result<u32> {
        self.key.get_value("VersionMajor")
    }
    pub fn version_minor(&self) -> io::Result<u32> {
        self.key.get_value("VersionMinor")
    }
    pub fn system_component(&self) -> bool {
        if let Ok(1u32) = self.key.get_value("SystemComponent") {
            true
        } else {
            false
        }
    }
}

impl<'a> IntoIterator for &'a Installers {
    type IntoIter = InstallerIter<'a>;
	type Item = Installer;

    fn into_iter(self) -> InstallerIter<'a> {
        self.iter()
    }
}

impl AllInstallers {
    pub fn iter(&self) -> iter::Chain<InstallerIter, InstallerIter> {
        self.0.iter().chain(self.1.iter())
    }
}

impl<'a> IntoIterator for &'a AllInstallers {
    type IntoIter = iter::Chain<InstallerIter<'a>, InstallerIter<'a>>;
	type Item = Installer;

    fn into_iter(self) -> iter::Chain<InstallerIter<'a>, InstallerIter<'a>> {
        self.iter()
    }
}

impl Installers {
    pub fn iter(&self) -> InstallerIter {
        InstallerIter(&self.0, self.0.enum_keys())
    }
}

pub struct InstallerIter<'a>(&'a RegKey, EnumKeys<'a>);

impl<'a> Iterator for InstallerIter<'a> {
    type Item = Installer;

    fn next(&mut self) -> Option<Installer> {
        loop {
            let n = self.1.next();

            if let Some(result) = n {
                if let Ok(name) = result {
                    if let Ok(key) = self.0.open_subkey_with_flags(&name, KEY_READ) {
                        if let Ok(1u32) = key.get_value("WindowsInstaller") {
                            return Some(Installer {
                                id: name,
                                key: key,
                            });
                        }
                    }
                }
            } else {
                return None;
            }
        }
    }
}

fn read_installer_registry(key: HKEY) -> io::Result<Installers> {
    let root = RegKey::predef(key);
    let uninstall = try!(root.open_subkey_with_flags("SOFTWARE\\Microsoft\\Windows\\CurrentVersi\
                                                      on\\Uninstall",
                                                     KEY_READ));

    Ok(Installers(uninstall))
}

pub fn local_machine_installers() -> io::Result<Installers> {
    read_installer_registry(HKEY_LOCAL_MACHINE)
}

pub fn current_user_installers() -> io::Result<Installers> {
    read_installer_registry(HKEY_CURRENT_USER)
}

pub fn all_installers() -> io::Result<AllInstallers> {
    Ok(AllInstallers(try!(local_machine_installers()),
                     try!(current_user_installers())))
}
