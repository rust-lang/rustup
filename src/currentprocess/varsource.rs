/// Abstracts over reading the current process environment variables as a
/// zero-cost abstraction to support threaded in-process testing.
use std::env;
use std::ffi::OsString;

pub trait VarSource {
    // In order to support dyn dispatch we use concrete types rather than the
    // stdlib signature.
    fn var(&self, key: &str) -> std::result::Result<String, env::VarError>;
    fn var_os(&self, key: &str) -> Option<OsString>;
}

/// Implements VarSource with `std::env::env`
impl VarSource for super::OSProcess {
    fn var(&self, key: &str) -> std::result::Result<String, env::VarError> {
        env::var(key)
    }
    fn var_os(&self, key: &str) -> Option<OsString> {
        env::var_os(key)
    }
}

#[cfg(feature = "test")]
impl VarSource for super::TestProcess {
    fn var(&self, key: &str) -> std::result::Result<String, env::VarError> {
        match self.var_os(key) {
            None => Err(env::VarError::NotPresent),
            // safe because we know this only has String in it.
            Some(key) => Ok(key.into_string().unwrap()),
        }
    }
    fn var_os(&self, key: &str) -> Option<OsString> {
        self.vars
            .get(key)
            .map(|s_ref| OsString::from(s_ref.clone()))
    }
}
