use common::ask;
use itertools::Itertools;
use multirust::{Cfg, Error, Result};
use multirust_dist::dist;
use multirust_dist;
use multirust_utils::{self, utils};
use openssl::crypto::hash::{Type, Hasher};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::thread;
use std::time::Duration;

pub fn maybe_install(cfg: &Cfg) -> Result<()> {
    let exe_path = try!(utils::current_exe());
    if !test_installed(&cfg) {
        if !ask("Install multirust now?").unwrap_or(false) {
            return Ok(());
        }
        let add_to_path = ask("Add multirust to PATH?").unwrap_or(false);
        return install(cfg, false, add_to_path);
    } else if exe_path.parent() != Some(&cfg.multirust_dir.join("bin")) {
        println!("Existing multirust installation detected.");
        if !ask("Replace or update it now?").unwrap_or(false) {
            return Ok(());
        }
        return install(cfg, false, false);
    } else {
        println!("This is the currently installed multirust binary.");
    }
    Ok(())
}

fn test_installed(cfg: &Cfg) -> bool {
    let bin = format!("bin/multirust{}", env::consts::EXE_SUFFIX);
    utils::is_file(cfg.multirust_dir.join(bin))
}

pub fn install(cfg: &Cfg, should_move: bool, add_to_path: bool) -> Result<()> {
    #[allow(dead_code)]
    fn create_bat_proxy(mut path: PathBuf, name: &'static str) -> Result<()> {
        path.push(name.to_owned() + ".bat");
        Ok(try!(utils::write_file(name,
                                  &path,
                                  &format!("@\"%~dp0\\multirust\" proxy {} %*", name))))

    }
    #[allow(dead_code)]
    fn create_sh_proxy(mut path: PathBuf, name: &'static str) -> Result<()> {
        path.push(name.to_owned());
        try!(utils::write_file(name,
                               &path,
                               &format!("#!/bin/sh\n\"`dirname $0`/multirust\" proxy {} \"$@\"",
                                        name)));
        Ok(try!(utils::make_executable(&path)))
    }
    fn create_symlink_proxy(mut path: PathBuf, name: &'static str) -> Result<()> {
        let mut dest_path = path.clone();
        dest_path.push("multirust".to_owned() + env::consts::EXE_SUFFIX);
        path.push(name.to_owned() + env::consts::EXE_SUFFIX);
        Ok(try!(utils::symlink_file(&dest_path, &path)))
    }
    fn create_hardlink_proxy(mut path: PathBuf, name: &'static str) -> Result<()> {
        let mut dest_path = path.clone();
        dest_path.push("multirust".to_owned() + env::consts::EXE_SUFFIX);
        path.push(name.to_owned() + env::consts::EXE_SUFFIX);
        Ok(try!(utils::hardlink_file(&dest_path, &path)))
    }

    let bin_path = cfg.multirust_dir.join("bin");

    try!(utils::ensure_dir_exists("bin", &bin_path, ntfy!(&cfg.notify_handler)));

    let dest_path = bin_path.join("multirust".to_owned() + env::consts::EXE_SUFFIX);
    let src_path = try!(utils::current_exe());

    if should_move {
        if cfg!(windows) {
            // Wait for old version to exit
            thread::sleep(Duration::from_millis(1000));
        }
        try!(utils::rename_file("multirust", &src_path, &dest_path));
    } else {
        try!(utils::copy_file(&src_path, &dest_path));
    }

    let tools = ["rustc", "rustdoc", "cargo", "rust-lldb", "rust-gdb"];
    for tool in &tools {
        // There are five ways to create the proxies:
        // 1) Shell/batch scripts
        //    On windows, `CreateProcess` (on which Command is based) will not look for batch scripts
        // 2) Symlinks
        //    On windows, symlinks require admin privileges to create
        // 3) Copies of the multirust binary
        //    The multirust binary is not exactly small
        // 4) Stub executables
        //    Complicates build process and even trivial rust executables are quite large
        // 5) Hard links
        //    Downsides are yet to be determined
        // As a result, use hardlinks on windows, and symlinks elsewhere.

        // try!(create_bat_proxy(bin_path.clone(), tool));
        // try!(create_sh_proxy(bin_path.clone(), tool));

        if cfg!(windows) {
            try!(create_hardlink_proxy(bin_path.clone(), tool));
        } else {
            try!(create_symlink_proxy(bin_path.clone(), tool));
        }
    }

    #[cfg(windows)]
    fn do_add_to_path(path: PathBuf) -> Result<()> {

        use winreg::RegKey;
        use winapi::*;
        use user32::*;
        use std::ptr;

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = try!(root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
                                   .map_err(|_| Error::PermissionDenied));

        let mut new_path: String = path.into_os_string()
                                       .into_string()
                                       .ok()
                                       .expect("cannot install to invalid unicode path");
        let old_path: String = environment.get_value("PATH").unwrap_or(String::new());
        new_path.push_str(";");
        new_path.push_str(&old_path);
        try!(environment.set_value("PATH", &new_path)
                        .map_err(|_| Error::PermissionDenied));

        // const HWND_BROADCAST: HWND = 0xffff as HWND;
        // const SMTO_ABORTIFHUNG: UINT = 0x0002;

        // Tell other processes to update their environment
        unsafe {
            SendMessageTimeoutA(HWND_BROADCAST,
                                WM_SETTINGCHANGE,
                                0 as WPARAM,
                                "Environment\0".as_ptr() as LPARAM,
                                SMTO_ABORTIFHUNG,
                                5000,
                                ptr::null_mut());
        }

        println!("PATH has been updated. You may need to restart your shell for changes to take \
                  effect.");

        Ok(())
    }
    #[cfg(not(windows))]
    fn do_add_to_path(path: PathBuf) -> Result<()> {
        let home_dir = try!(utils::home_dir().ok_or(multirust_utils::Error::LocatingHome));
        let tmp = path.into_os_string()
                      .into_string()
                      .expect("cannot install to invalid unicode path");
        try!(utils::append_file(".profile",
                                &home_dir.join(".profile"),
                                &format!("\n# Multirust override:\nexport PATH=\"{}:$PATH\"",
                                         &tmp)));

        println!("'~/.profile' has been updated. You will need to start a new login shell for \
                  changes to take effect.");

        Ok(())
    }

    if add_to_path {
        try!(do_add_to_path(bin_path));
    }

    info!("Installed");

    Ok(())
}

pub fn uninstall(cfg: &Cfg, no_prompt: bool) -> Result<()> {
    if !no_prompt &&
       !ask("This will delete all toolchains, overrides, aliases, and other multirust data \
            associated with this user. Continue?")
            .unwrap_or(false) {
        println!("aborting");
        return Ok(());
    }

    #[cfg(windows)]
    fn inner(cfg: &Cfg) -> Result<()> {
        let mut cmd = Command::new("cmd");
        let _ = cmd.arg("/C")
                   .arg("start")
                   .arg("cmd")
                   .arg("/C")
                   .arg(&format!("echo Uninstalling... & ping -n 4 127.0.0.1>nul & rd /S /Q {} \
                                  & echo Uninstalled",
                                 cfg.multirust_dir.display()))
                   .spawn();
        Ok(())
    }
    #[cfg(not(windows))]
    fn inner(cfg: &Cfg) -> Result<()> {
        println!("Uninstalling...");
        Ok(try!(utils::remove_dir("multirust", &cfg.multirust_dir, ntfy!(&cfg.notify_handler))))
    }

    warn!("This will not attempt to remove the '.multirust/bin' directory from your PATH");
    try!(inner(cfg));

    process::exit(0);
}

pub fn update(cfg: &Cfg) -> Result<()> {
    // Get host triple
    let (arch, os, maybe_env) = dist::get_host_triple();
    let triple = if let Some(env) = maybe_env {
        format!("{}-{}-{}", arch, os, env)
    } else {
        format!("{}-{}", arch, os)
    };

    // Get download URL
    let url = format!("https://github.\
                       com/Diggsey/multirust-rs-binaries/raw/master/{}/multirust-rs{}",
                      triple,
                      env::consts::EXE_SUFFIX);

    // Calculate own hash
    let mut hasher = Hasher::new(Type::SHA256);
    try!(utils::tee_file("self", &try!(utils::current_exe()), &mut hasher));
    let current_hash = hasher.finish()
                             .iter()
                             .map(|b| format!("{:02x}", b))
                             .join("");

    // Download latest hash
    let mut latest_hash = {
        let hash_url = try!(utils::parse_url(&(url.clone() + ".sha256")));
        let hash_file = try!(cfg.temp_cfg.new_file());
        try!(utils::download_file(hash_url, &hash_file, None, ntfy!(&cfg.notify_handler)));
        try!(utils::read_file("hash", &hash_file))
    };
    latest_hash.truncate(64);

    // If up-to-date
    if latest_hash == current_hash {
        info!("Already up to date!");
        return Ok(());
    }

    // Get download path
    let download_file = try!(cfg.temp_cfg.new_file_with_ext("multirust-", env::consts::EXE_SUFFIX));
    let download_url = try!(utils::parse_url(&url));

    // Download new version
    let mut hasher = Hasher::new(Type::SHA256);
    try!(utils::download_file(download_url,
                              &download_file,
                              Some(&mut hasher),
                              ntfy!(&cfg.notify_handler)));
    let download_hash = hasher.finish()
                              .iter()
                              .map(|b| format!("{:02x}", b))
                              .join("");

    // Check that hash is correct
    if latest_hash != download_hash {
        return Err(Error::Install(multirust_dist::Error::ChecksumFailed {
            url: url,
            expected: latest_hash,
            calculated: download_hash,
        }));
    }

    // Mark as executable
    try!(utils::make_executable(&download_file));

    #[cfg(windows)]
    fn inner(path: &Path) -> Result<()> {
        let mut cmd = Command::new("cmd");
        let _ = cmd.arg("/C")
                   .arg("start")
                   .arg(path)
                   .arg("self")
                   .arg("install")
                   .arg("-m")
                   .spawn();
        Ok(())
    }
    #[cfg(not(windows))]
    fn inner(path: &Path) -> Result<()> {
        Ok(try!(utils::cmd_status("update",
                                  Command::new(path).arg("self").arg("install").arg("-m"))))
    }

    println!("Installing...");
    try!(inner(&download_file));
    process::exit(0);
}
