use std::env;
use std::process;
use self_update::{self, InstallOpts};
use errors::*;
use clap::{App, Arg, AppSettings};
use rustup_dist::dist::TargetTriple;
use common;

mod sys_check {
    #[cfg(unix)]
    pub fn home_mismatch() -> bool {
	extern crate libc as c;

	use std::env;
	use std::ffi::CStr;
	use std::mem;
	use std::ops::Deref;
	use std::ptr;

	let mut pwd = unsafe { mem::uninitialized::<c::passwd>() };
	let mut pwdp: *mut c::passwd = ptr::null_mut();
	let mut buf = [0u8; 1024];
	let rv = unsafe { c::getpwuid_r(c::geteuid(), &mut pwd, mem::transmute(&mut buf), buf.len(), &mut pwdp) };
	if rv != 0 || pwdp == ptr::null_mut() {
	    warn!("getpwuid_r: couldn't get user data");
	    return false;
	}
	let pw_dir = unsafe { CStr::from_ptr(pwd.pw_dir) }.to_str().ok();
	let env_home = env::var_os("HOME");
	let env_home = env_home.as_ref().map(Deref::deref);
	match (env_home, pw_dir) {
	    (None, _) | (_, None) => false,
	    (Some(ref eh), Some(ref pd)) => eh != pd
	}
    }

    #[cfg(not(unix))]
    pub fn home_mismatch() -> bool {
	false
    }
}

pub fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    let arg1 = args.get(1).map(|a| &**a);

    // Secret command used during self-update. Not for users.
    if arg1 == Some("--self-replace") {
        return self_update::self_replace();
    }

    let cli = App::new("rustup-init")
        .version(common::version())
        .about("The installer for rustup")
        .setting(AppSettings::DeriveDisplayOrder)
        .arg(Arg::with_name("verbose")
             .short("v")
             .long("verbose")
             .help("Enable verbose output"))
        .arg(Arg::with_name("no-prompt")
             .short("y")
             .help("Disable confirmation prompt."))
        .arg(Arg::with_name("default-host")
             .long("default-host")
             .takes_value(true)
             .help("Choose a default host triple"))
        .arg(Arg::with_name("default-toolchain")
             .long("default-toolchain")
             .takes_value(true)
             .help("Choose a default toolchain to install"))
        .arg(Arg::with_name("no-modify-path")
             .long("no-modify-path")
             .help("Don't configure the PATH environment variable"));

    let matches = cli.get_matches();
    let no_prompt = matches.is_present("no-prompt");
    match (self::sys_check::home_mismatch(), no_prompt) {
	(false, _) => (),
	(true, false) => {
	    err!("$HOME differs from euid-obtained home directory: you may be using sudo");
	    err!("if this is what you want, restart the installation with `-y'");
	    process::exit(1);
	},
	(true, true) => {
	    warn!("$HOME differs from euid-obtained home directory: you may be using sudo");
	}
    }
    let verbose = matches.is_present("verbose");
    let default_host = matches.value_of("default-host").map(|s| s.to_owned()).unwrap_or_else(|| {
        TargetTriple::from_host_or_build().to_string()
    });
    let default_toolchain = matches.value_of("default-toolchain").unwrap_or("stable");
    let no_modify_path = matches.is_present("no-modify-path");

    let opts = InstallOpts {
        default_host_triple: default_host,
        default_toolchain: default_toolchain.to_owned(),
        no_modify_path: no_modify_path,
    };

    try!(self_update::install(no_prompt, verbose, opts));

    Ok(())
}
