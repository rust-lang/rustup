use std::{ffi::OsString, io, os::windows::ffi::OsStringExt, path::PathBuf, ptr, slice};

use windows_result::HRESULT;
use windows_sys::Win32::{
    System::Com::CoTaskMemFree,
    UI::Shell::{
        FOLDERID_LocalAppData, FOLDERID_RoamingAppData, KF_FLAG_DONT_VERIFY, SHGetKnownFolderPath,
    },
};

use crate::process::home::{HomeCategory, env::Env};

pub fn category_home_with_env(category: HomeCategory, _env: &impl Env) -> io::Result<PathBuf> {
    known_folder(match category {
        HomeCategory::Cache => &FOLDERID_LocalAppData,
        HomeCategory::Config | HomeCategory::Data | HomeCategory::State => &FOLDERID_RoamingAppData,
    })
}

fn known_folder(id: &windows_sys::core::GUID) -> io::Result<PathBuf> {
    let mut path = ptr::null_mut();

    // SAFETY: `SHGetKnownFolderPath` initializes `path` with a CoTaskMem-allocated,
    // null-terminated UTF-16 string on success. `CoTaskMemFree` accepts null and is
    // called on both result paths; the success path reads only through the terminator.
    unsafe {
        let result = HRESULT(SHGetKnownFolderPath(
            id,
            KF_FLAG_DONT_VERIFY as u32,
            ptr::null_mut(),
            &mut path,
        ));
        if let Err(error) = result.ok() {
            CoTaskMemFree(path.cast());
            return Err(error.into());
        }

        let result = OsString::from_wide(slice::from_raw_parts(path, wcslen(path)));
        CoTaskMemFree(path.cast());
        Ok(result.into())
    }
}

unsafe extern "C" {
    fn wcslen(buf: *const u16) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_use_known_folders_without_environment() -> io::Result<()> {
        let env = PanicEnv;
        let local = known_folder(&FOLDERID_LocalAppData)?;
        let roaming = known_folder(&FOLDERID_RoamingAppData)?;

        assert_eq!(category_home_with_env(HomeCategory::Cache, &env)?, local);
        for category in [
            HomeCategory::Config,
            HomeCategory::Data,
            HomeCategory::State,
        ] {
            assert_eq!(category_home_with_env(category, &env)?, roaming);
        }
        Ok(())
    }

    struct PanicEnv;

    impl Env for PanicEnv {
        fn home_dir(&self) -> Option<PathBuf> {
            panic!("home_dir must not be queried")
        }

        fn current_dir(&self) -> io::Result<PathBuf> {
            panic!("current_dir must not be queried")
        }

        fn var_os(&self, _key: &str) -> Option<OsString> {
            panic!("var_os must not be queried")
        }
    }
}
