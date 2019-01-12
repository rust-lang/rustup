use std::env::consts::EXE_SUFFIX;
use std::env;
use std::fs::File;
use std::io::{self, Write, Read};
use std::path::{PathBuf, Path};
use std::process::Command;

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_ref().map(|s| &**s) {
        Some("--version") => {
            let me = env::current_exe().unwrap();
            let mut version_file = PathBuf::from(format!("{}.version", me.display()));
            let mut hash_file = PathBuf::from(format!("{}.version-hash", me.display()));
            let mut version = String::new();
            let mut hash = String::new();
            File::open(&version_file).unwrap().read_to_string(&mut version).unwrap();
            File::open(&hash_file).unwrap().read_to_string(&mut hash).unwrap();
            println!("{} ({})", version, hash);
        }
        Some("--empty-arg-test") => {
            assert_eq!(args.next().unwrap(), "");
        }
        Some("--huge-output") => {
            let mut out = io::stderr();
            for _ in 0 .. 10000 {
                out.write_all(b"error: a value named `fail` has already been defined in this module [E0428]\n").unwrap();
            }
        }
        Some("--call-rustc") => {
            // Used by the fallback_cargo_calls_correct_rustc test. Tests that
            // the environment has been set up right such that invoking cargo
            // will invoke the correct rustc executable.
            let rustc = env::var_os("RUSTC").unwrap_or(format!("rustc{}", EXE_SUFFIX).into());
            Command::new(rustc).arg("--version").status().unwrap();
        }
        _ => panic!("bad mock proxy commandline"),
    }
}

#[cfg(unix)]
fn equivalent(_: &Path, _: &Path) -> bool { false }

#[cfg(windows)]
#[allow(warnings)]
fn equivalent(a: &Path, b: &Path) -> bool {
    use std::mem;
    use std::os::windows::prelude::*;
    use std::os::windows::raw::HANDLE;

    extern "system" {
        fn GetFileInformationByHandle(a: HANDLE, b: *mut BY_HANDLE_FILE_INFORMATION)
            -> i32;
    }

    #[repr(C)]
    struct BY_HANDLE_FILE_INFORMATION {
        dwFileAttributes: u32,
        ftCreationTime: FILETIME,
        ftLastAccessTime: FILETIME,
        ftLastWriteTime: FILETIME,
        dwVolumeSerialNumber: u32,
        nFileSizeHigh: u32,
        nFileSizeLow: u32,
        nNumberOfLinks: u32,
        nFileIndexHigh: u32,
        nFileIndexLow: u32,
    }

    #[repr(C)]
    struct FILETIME {
        dwLowDateTime: u32,
        dwHighDateTime: u32,
    }

    let a = File::open(a).unwrap();
    let b = File::open(b).unwrap();

    unsafe {
        let mut ainfo = mem::zeroed();
        let mut binfo = mem::zeroed();
        GetFileInformationByHandle(a.as_raw_handle(), &mut ainfo);
        GetFileInformationByHandle(b.as_raw_handle(), &mut binfo);

        ainfo.dwVolumeSerialNumber == binfo.dwVolumeSerialNumber &&
            ainfo.nFileIndexHigh == binfo.nFileIndexHigh &&
            ainfo.nFileIndexLow == binfo.nFileIndexLow
    }
}
