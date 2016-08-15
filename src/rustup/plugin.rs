use errors::*;
use std::path::PathBuf;
use std::process::Command;
use std::ffi::OsStr;
use toolchain::Toolchain;
use rustup_utils::utils;

pub struct Plugin<'a> {
    pub toolchain: &'a Toolchain<'a>,
    pub binary: PathBuf
}

impl<'a> Plugin<'a> {
    pub fn cmd(&self) -> Command {
        let mut cmd = Command::new(&self.binary);
        self.toolchain.set_env(&mut cmd);
        cmd
    }

    // These commands are only relevent for target plugins
    pub fn target_add(&self, target_desc: &str) -> Result<()> {
        let mut cmd = self.cmd();
        cmd.arg("target-add").arg(target_desc);
        try!(utils::cmd_status("plugin", &mut cmd));
        Ok(())
    }

    pub fn target_run(&self, target_desc: &str, args: &[&OsStr]) -> Result<()> {
        let mut cmd = self.cmd();
        cmd.arg("target-run").arg(target_desc).args(args);
        try!(utils::cmd_status("plugin", &mut cmd));
        Ok(())
    }
}
