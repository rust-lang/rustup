use common::{run_inner, set_globals};
use multirust::{Cfg, Result, Error};
use multirust_utils::utils;
use std::env;
use std::path::PathBuf;

pub fn main() -> Result<()> {
    let arg0 = env::args().next().map(|a| PathBuf::from(a));
    let arg0 = arg0.as_ref()
        .and_then(|a| a.file_name())
        .and_then(|a| a.to_str());
    let ref arg0 = try!(arg0.ok_or(Error::NoExeName));

    let cfg = try!(set_globals(false));
    try!(cfg.check_metadata_version());
    try!(direct_proxy(&cfg, arg0));

    Ok(())
}

fn direct_proxy(cfg: &Cfg, arg0: &str) -> Result<()> {
    let args: Vec<_> = env::args_os().collect();
    run_inner(cfg,
              cfg.create_command_for_dir(&try!(utils::current_dir()), arg0),
              &args)
}

