use toml;

use std::error;
use std::fmt::{self, Display};

#[derive(Debug)]
pub enum Error {
    Parsing(Vec<toml::ParserError>),
    MissingKey(String),
    ExpectedType(&'static str, String),
    PackageNotFound(String),
    TargetNotFound(String),
    MissingRoot,
    UnsupportedVersion(String),
}

impl error::Error for Error {
    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            Parsing(_) => "error parsing manifest",
            MissingKey(_) => "missing key",
            ExpectedType(_, _) => "expected type",
            PackageNotFound(_) => "package not found",
            TargetNotFound(_) => "target not found",
            MissingRoot => "manifest has no root package",
            UnsupportedVersion(_) => "unsupported manifest version",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
        use self::Error::*;
        match *self {
            Parsing(ref n) => {
                for e in n {
                    try!(e.fmt(f));
                    try!(writeln!(f, ""));
                }
                Ok(())
            }
            MissingKey(ref n) => write!(f, "missing key: '{}'", n),
            ExpectedType(ref t, ref n) => write!(f, "expected type: '{}' for '{}'", t, n),
            PackageNotFound(ref n) => write!(f, "package not found: '{}'", n),
            TargetNotFound(ref n) => write!(f, "target not found: '{}'", n),
            MissingRoot => write!(f, "manifest has no root package"),
            UnsupportedVersion(ref v) => write!(f, "manifest version '{}' is not supported", v),
        }
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;
