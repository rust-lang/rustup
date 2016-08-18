use std::env;
use std::process::{self, Command};
use std::io::{self, Write};
use std::fs;
use std::path::{Path, PathBuf};
use std::ffi::OsString;

// Allow us to expect any option-like thing
trait IntoOption { type Value; fn into_option(self) -> Option<Self::Value>; }
impl<T> IntoOption for Option<T> { type Value = T; fn into_option(self) -> Option<T> { self } }
impl<T, E> IntoOption for Result<T, E> { type Value = T; fn into_option(self) -> Option<T> { self.ok() } }
impl IntoOption for bool { type Value = (); fn into_option(self) -> Option<()> { if self { Some(()) } else { None } } }

fn expect<O: IntoOption>(ov: O, msg: &str) -> O::Value {
    if let Some(v) = ov.into_option() {
        v
    } else {
        let _ = writeln!(io::stderr(), "target-android: {}", msg);
        process::exit(1)
    }
}

fn info(msg: &str) {
    println!("target-android: {}", msg);
}

pub fn is_directory<P: AsRef<Path>>(path: P) -> bool {
    fs::metadata(path).ok().as_ref().map(fs::Metadata::is_dir) == Some(true)
}
pub fn is_file<P: AsRef<Path>>(path: P) -> bool {
    fs::metadata(path).ok().as_ref().map(fs::Metadata::is_file) == Some(true)
}

#[cfg(target_os = "windows")]
const PYTHON_LOCATIONS: [&'static str; 2] = ["prebuilt/windows-x86_64/bin/python.exe", "prebuilt/windows-x86/bin/python.exe"];
#[cfg(target_os = "linux")]
const PYTHON_LOCATIONS: [&'static str; 2] = ["prebuilt/linux-x86_64/bin/python", "prebuilt/linux-x86/bin/python"];
#[cfg(target_os = "macos")]
const PYTHON_LOCATIONS: [&'static str; 2] = ["prebuilt/darwin-x86_64/bin/python", "prebuilt/darwin-x86/bin/python"];
#[cfg(all(target_os = "android", target_arch = "arm"))]
const PYTHON_LOCATIONS: [&'static str; 1] = ["prebuilt/android-arm/bin/python"];

fn locate_python(ndk_path: &Path) -> Option<PathBuf> {
    for loc in PYTHON_LOCATIONS.iter() {
        let path = ndk_path.join(loc);
        if is_file(&path) {
            return Some(path);
        }
    }
    None
}

struct TargetDesc {
    arch: String,
    api: String,
    stl: String
}

impl TargetDesc {
    fn target_triple(&self) -> &'static str {
        match &*self.arch {
            "armeabi" => "arm-linux-androideabi",
            "armeabi-v7a" => "armv7-linux-androideabi",
            "arm64-v8a" => "aarch64-linux-android",
            "x86" => "i686-linux-android",
            "x86_64" => "x86_64-linux-android",
            "mips" => "mipsel-linux-android",
            _ => {
                expect(false, "Unsupported architecture");
                unreachable!()
            }
        }
    }

    fn dir_name(&self) -> String {
        format!("{}-{}-{}", self.arch, self.api, self.stl)
    }

    fn simple_arch(&self) -> &'static str {
        match &*self.arch {
            "armeabi" => "arm",
            "armeabi-v7a" => "arm",
            "arm64-v8a" => "arm64",
            "x86" => "x86",
            "x86_64" => "x86_64",
            "mips" => "mips",
            _ => {
                expect(false, "Unsupported architecture");
                unreachable!()
            }
        }
    }
}

fn parse_target_desc(target_desc: &str) -> TargetDesc {
    let mut parts = target_desc.split(",");
    let arch = expect(parts.next(), "Invalid target descriptor, expected: <arch>[,api=<level>][,stl=<stl>]");
    let mut result = TargetDesc {
        arch: arch.to_owned(),
        api: "21".to_owned(),
        stl: "gnustl".to_owned()
    };
    for part in parts {
        let mut kvp = part.splitn(2, "=");
        let k = kvp.next().unwrap();
        let v = expect(kvp.next(), "Expected: <key>=<value>").to_owned();
        match k {
            "api" => result.api = v,
            "stl" => result.stl = v,
            _ => expect(false, "Unknown key in target descriptor")
        }
    }
    result
}

fn main() {
    // Locate the Android NDK
    let ndk_path: PathBuf = expect(env::var_os("ANDROID_NDK"),
        "Install the Android NDK from `https://developer.android.com/ndk/downloads/index.html`, \
        and set the `ANDROID_NDK` environment variable to point to its root directory."
    ).into();

    // This executable will be located at `<toolchain>/plugins/<plugin_name>/bin/<plugin_name>[.exe]`,
    // so pop two path components to get to our plugin's directory.
    let mut plugin_dir = expect(env::current_exe(), "Failed to locate plugin directory");
    plugin_dir.pop();
    plugin_dir.pop();

    // Ensure toolchains directory exists
    let toolchains_dir = plugin_dir.join("toolchains");
    expect(fs::create_dir_all(&toolchains_dir), "Failed to create toolchains directory");

    // Parse command-line arguments
    let mut args = env::args_os().skip(1);
    let arg0 = args.next();
    match arg0.as_ref().and_then(|s| s.to_str()) {
        Some("target-add") => {
            let target_desc = parse_target_desc(args.next().unwrap().to_str().unwrap());
            let toolchain_dir = toolchains_dir.join(target_desc.dir_name());

            // Add rustup target
            let mut cmd = Command::new("rustup");
            cmd.arg("target").arg("add").arg(target_desc.target_triple());
            expect(cmd.status().ok().and_then(|e| e.success().into_option()), "Failed to add target");

            // Create NDK toolchain
            if !is_directory(&toolchain_dir) {
                let make_standalone_toolchain = ndk_path.join("build/tools/make_standalone_toolchain.py");
                let python_bin = expect(locate_python(&ndk_path), "Failed to locate python in NDK");

                let mut cmd = Command::new(python_bin);
                cmd
                    .arg(make_standalone_toolchain)
                    .arg("--arch").arg(target_desc.simple_arch())
                    .arg("--api").arg(&target_desc.api)
                    .arg("--stl").arg(&target_desc.stl)
                    .arg("--install-dir").arg(&toolchain_dir);
                
                info("Building standalone NDK toolchain...");
                expect(cmd.status().ok().and_then(|e| e.success().into_option()), "Failed to build standalone toolchain");
            }
        },
        Some("target-run") => {
            let target_desc = parse_target_desc(args.next().unwrap().to_str().unwrap());
            let toolchain_dir = toolchains_dir.join(target_desc.dir_name());

            expect(is_directory(&toolchain_dir), "Toolchain does not exist");

            let mut linker_path = toolchain_dir.join("bin");
            linker_path.push(format!("{}-gcc{}", target_desc.target_triple(), env::consts::EXE_SUFFIX));

            let binary = args.next().unwrap();
            let cmd_args: Vec<_> = args.collect();

            let mut cmd = Command::new(&binary);
            cmd
                .args(&cmd_args)
                .arg("--target").arg(target_desc.target_triple());

            let bin_filename = Path::new(&binary);
            match bin_filename.file_stem().unwrap().to_str().unwrap() {
                "rustc" => {
                    let mut linker_arg: OsString = "linker=".to_owned().into();
                    linker_arg.push(&linker_path);
                    cmd.arg("-C").arg(linker_arg);
                },
                "cargo" => {
                    let triple_name = target_desc.target_triple()
                        .replace("-", "_")
                        .chars()
                        .flat_map(|c| c.to_uppercase())
                        .collect::<String>();
                    cmd.env(format!("CARGO_TARGET_{}_LINKER", triple_name), &linker_path);
                },
                _ => ()
            }

            println!("{:?}", cmd);
            
            expect(cmd.status().ok().and_then(|e| e.success().into_option()), "Failed to proxy command");
        },
        _ => unreachable!()
    }
}
