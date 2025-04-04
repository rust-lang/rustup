use std::env;
use std::env::consts::EXE_SUFFIX;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let mut args = env::args_os();
    let arg0 = args.next().unwrap();
    if let Some(cargo_subcommand) = PathBuf::from(arg0)
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_prefix("cargo-"))
    {
        let arg1 = args.next().unwrap();
        assert_eq!(arg1, cargo_subcommand);
    }
    match args.next().as_ref().and_then(|s| s.to_str()) {
        Some("--version") => {
            let me = env::current_exe().unwrap();
            let mut version_file = PathBuf::from(format!("{}.version", me.display()));
            let mut hash_file = PathBuf::from(format!("{}.version-hash", me.display()));
            if !version_file.exists() {
                // There's a "MAJOR HACKS" statement in `toolchain.rs` right
                // now where custom toolchains use a `cargo.exe` that's
                // temporarily located elsewhere so they can execute the correct
                // `rustc.exe`. This means that our dummy version files may not
                // be just next to use.
                //
                // Detect this here and work around it.
                assert!(cfg!(windows));
                assert!(env::var_os("RUSTUP_TOOLCHAIN").is_some());
                let mut alt = me.clone();
                alt.pop(); // remove our filename
                assert!(alt.ends_with("fallback"));
                alt.pop(); // pop 'fallback'
                alt.push("toolchains");

                let mut part = PathBuf::from("bin");
                part.push(me.file_name().unwrap());

                let path = alt
                    .read_dir()
                    .unwrap()
                    .map(|e| e.unwrap().path().join(&part))
                    .filter(|p| p.exists())
                    .find(|p| equivalent(&p, &me))
                    .unwrap();

                version_file = format!("{}.version", path.display()).into();
                hash_file = format!("{}.version-hash", path.display()).into();
            }
            let version = std::fs::read_to_string(&version_file).unwrap();
            let hash = std::fs::read_to_string(&hash_file).unwrap();
            println!("{} ({})", version, hash);
        }
        Some("--empty-arg-test") => {
            assert_eq!(args.next().unwrap(), "");
        }
        Some("--huge-output") => {
            let mut out = io::stderr();
            for _ in 0..10000 {
                out.write_all(b"error: a value named `fail` has already been defined in this module [E0428]\n").unwrap();
            }
        }
        Some("--call-rustc") => {
            // Used by the fallback_cargo_calls_correct_rustc test. Tests that
            // the environment has been set up right such that invoking rustc
            // will actually invoke the wrapper
            let rustc = &format!("rustc{}", EXE_SUFFIX);
            Command::new(rustc).arg("--version").status().unwrap();
        }
        Some("--recursive-cargo-subcommand") => {
            let status = Command::new("cargo-foo")
                .args(["foo", "--recursive-cargo"])
                .status()
                .unwrap();
            assert!(status.success());
        }
        Some("--recursive-cargo") => {
            let status = Command::new("cargo")
                .args(&["+nightly", "--version"])
                .status()
                .unwrap();
            assert!(status.success());
        }
        Some("--echo-args") => {
            let mut out = io::stderr();
            for arg in args {
                writeln!(out, "{}", arg.to_string_lossy()).unwrap();
            }
        }
        Some("--echo-path") => {
            let mut out = io::stderr();
            writeln!(out, "{}", std::env::var("PATH").unwrap()).unwrap();
        }
        Some("--echo-cargo-env") => {
            let mut out = io::stderr();
            if let Ok(cargo) = std::env::var("CARGO") {
                writeln!(out, "{cargo}").unwrap();
            } else {
                panic!("CARGO environment variable not set");
            }
        }
        arg => panic!("bad mock proxy commandline: {:?}", arg),
    }
}

#[cfg(unix)]
fn equivalent(_: &Path, _: &Path) -> bool {
    false
}

#[cfg(windows)]
#[allow(non_snake_case)]
fn equivalent(a: &Path, b: &Path) -> bool {
    use std::fs::File;
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::raw::HANDLE;

    #[repr(C)]
    struct FILETIME {
        dwLowDateTime: u32,
        dwHighDateTime: u32,
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

    extern "system" {
        fn GetFileInformationByHandle(a: HANDLE, b: *mut BY_HANDLE_FILE_INFORMATION) -> i32;
    }

    let a = File::open(a).unwrap();
    let b = File::open(b).unwrap();
    let (ainfo, binfo) = unsafe {
        let mut ainfo = MaybeUninit::uninit();
        let mut binfo = MaybeUninit::uninit();
        if GetFileInformationByHandle(a.as_raw_handle(), ainfo.as_mut_ptr()) == 0 {
            return false;
        }
        if GetFileInformationByHandle(b.as_raw_handle(), binfo.as_mut_ptr()) == 0 {
            return false;
        }
        (ainfo.assume_init(), binfo.assume_init())
    };

    ainfo.dwVolumeSerialNumber == binfo.dwVolumeSerialNumber
        && ainfo.nFileIndexHigh == binfo.nFileIndexHigh
        && ainfo.nFileIndexLow == binfo.nFileIndexLow
}
