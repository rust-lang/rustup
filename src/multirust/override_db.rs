use std::path::{Path, PathBuf};

use errors::*;
use multirust_dist::{utils, temp};

pub const DB_DELIMITER: &'static str = ";";

#[derive(Debug)]
pub struct OverrideDB(PathBuf);

impl OverrideDB {
    fn path_to_db_key(&self, path: &Path, notify_handler: NotifyHandler) -> Result<String> {
        Ok(utils::canonicalize_path(path, ntfy!(&notify_handler))
               .display()
               .to_string() + DB_DELIMITER)
    }

    pub fn new(path: PathBuf) -> Self {
        OverrideDB(path)
    }

    pub fn remove(&self,
                  path: &Path,
                  temp_cfg: &temp::Cfg,
                  notify_handler: NotifyHandler)
                  -> Result<bool> {
        let key = try!(self.path_to_db_key(path, notify_handler));

        let work_file = try!(temp_cfg.new_file());

        let removed = if utils::is_file(&self.0) {
            try!(utils::filter_file("override db",
                                    &self.0,
                                    &work_file,
                                    |line| !line.starts_with(&key)))
        } else {
            0
        };

        if removed > 0 {
            try!(utils::rename_file("override db", &*work_file, &self.0));
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn set(&self,
               path: &Path,
               toolchain: &str,
               temp_cfg: &temp::Cfg,
               notify_handler: NotifyHandler)
               -> Result<()> {
        let key = try!(self.path_to_db_key(path, notify_handler));

        let work_file = try!(temp_cfg.new_file());

        if utils::is_file(&self.0) {
            try!(utils::filter_file("override db",
                                    &self.0,
                                    &work_file,
                                    |line| !line.starts_with(&key)));
        }

        try!(utils::append_file("override db", &work_file, &(key + toolchain)));

        try!(utils::rename_file("override db", &*work_file, &self.0));

        notify_handler.call(Notification::SetOverrideToolchain(path, toolchain));

        Ok(())
    }

    pub fn find(&self,
                dir_unresolved: &Path,
                notify_handler: NotifyHandler)
                -> Result<Option<(String, PathBuf)>> {
        if !utils::is_file(&self.0) {
            return Ok(None);
        }

        let dir = utils::canonicalize_path(dir_unresolved, ntfy!(&notify_handler));
        let mut path = &*dir;
        while let Some(parent) = path.parent() {
            let key = try!(self.path_to_db_key(path, notify_handler));
            if let Some(toolchain) = try!(utils::match_file("override db", &self.0, |line| {
                if line.starts_with(&key) {
                    Some(line[key.len()..].to_owned())
                } else {
                    None
                }
            })) {
                return Ok(Some((toolchain, path.to_owned())));
            }

            path = parent;
        }

        Ok(None)
    }

    pub fn list(&self) -> Result<Vec<String>> {
        if utils::is_file(&self.0) {
            let contents = try!(utils::read_file("override db", &self.0));

            let overrides: Vec<_> = contents.lines()
                                            .map(|s| s.to_owned())
                                            .collect();

            Ok(overrides)
        } else {
            Ok(Vec::new())
        }
    }
}
