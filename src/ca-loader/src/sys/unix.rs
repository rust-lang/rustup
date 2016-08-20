extern crate libc;

use std::ffi::CStr;
use std::fs;
use std::mem;
use std::result::Result;
use super::super::CertItem;

cfg_if! {
    if #[cfg(any(target_os = "android", target_os = "solaris"))] {
        use std::fs::{read_dir, ReadDir};

        pub struct CertBundle(&'static str);

        pub struct CertIter(&'static str, Option<ReadDir>);

        impl IntoIterator for CertBundle {
            type Item = CertItem;
            type IntoIter = CertIter;

            fn into_iter(self) -> Self::IntoIter {
                if let Ok(dir) = read_dir(self.0) {
                    CertIter(self.0, Some(dir))
                } else {
                    CertIter(self.0, None)
                }
            }
        }

        impl Iterator for CertIter {
            type Item = CertItem;

            fn next(&mut self) -> Option<Self::Item> {
                match self.1 {
                    None => return None,
                    Some(ref mut dir) => {
                        match dir.next() {
                            None => return None,
                            Some(Err(_)) => return None,
                            Some(Ok(ref de)) => {
                                if let Ok(ftyp) = de.file_type() {
                                    if !ftyp.is_dir() {
                                        if let Some(s) = de.file_name().to_str() {
                                            let mut full_name = String::from(self.0);
                                            full_name.push('/');
                                            full_name.push_str(s);
                                            return Some(CertItem::File(full_name));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                self.next()
            }
        }

        impl CertBundle {
            pub fn new() -> Result<CertBundle, ()> {
                Ok(CertBundle(try!(sys_path())))
            }
        }
    } else {
        use std::option;

        pub struct CertBundle(Option<CertItem>);

        impl IntoIterator for CertBundle {
            type Item = CertItem;
            type IntoIter = option::IntoIter<CertItem>;

            fn into_iter(self) -> Self::IntoIter {
                self.0.into_iter()
            }
        }

        impl CertBundle {
            pub fn new() -> Result<CertBundle, ()> {
                Ok(CertBundle(Some(CertItem::File(try!(sys_path()).to_string()))))
            }
        }
    }
}

pub fn sys_path() -> Result<&'static str, ()> {
    // Why use mem::uninitialized()? If we didn't, we'd need a bunch of
    // #cfg's for OS variants, since the components of struct utsname are
    // fixed-size char arrays (so no generic initializers), and that size
    // is different across OSs. Furthermore, uname() doesn't care about
    // the contents of struct utsname on input, and will fill it with
    // properly NUL-terminated strings on successful return.
    unsafe {
        let mut uts = mem::uninitialized::<libc::utsname>();

        if libc::uname(&mut uts) < 0 {
            return Err(());
        }
        let sysname = try!(CStr::from_ptr(uts.sysname.as_ptr()).to_str().map_err(|_| ()));
        let release = try!(CStr::from_ptr(uts.release.as_ptr()).to_str().map_err(|_| ()));
        let path = match sysname {
            "FreeBSD" | "OpenBSD" => "/etc/ssl/cert.pem",
            "NetBSD" => "/etc/ssl/certs",
            "Linux" => linux_distro_guess_ca_path(),
            "SunOS" => {
                let major = release.split('.').take(1).collect::<String>();
                let major = major.parse::<u32>().unwrap_or(5);
                let minor = release.split('.').skip(1).take(1).collect::<String>();
                let minor = minor.parse::<u32>().unwrap_or(10);
                if major < 5 || (major == 5 && minor < 11) {
                    "/opt/csw/share/cacertificates/mozilla"
                } else {
                    "/etc/certs/CA"
                }
            }
            _ => unimplemented!()
        };
        Ok(path)
    }
}

cfg_if! {
    if #[cfg(target_os = "android")] {
        fn linux_distro_guess_ca_path() -> &'static str {
            "/system/etc/security/cacerts"
        }
    } else {
        fn linux_distro_guess_ca_path() -> &'static str {
            if let Ok(_debian) = fs::metadata("/etc/debian_version") {
                "/etc/ssl/certs/ca-certificates.crt"
            } else if let Ok(_rh) = fs::metadata("/etc/redhat-release") {
                "/etc/pki/tls/certs/ca-bundle.crt"
            } else if let Ok(_suse) = fs::metadata("/etc/SuSE-release") {
                "/etc/ssl/ca-bundle.pem"
            } else {                                // fallback
                "/etc/pki/tls/cacert.pem"
            }
        }
    }
}
