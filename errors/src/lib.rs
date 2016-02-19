use std::any::Any;
use std::error::Error;
use std::fmt::{self, Debug, Display};

/// Wrapper type that implements `Error` for "error" types that do not implement it themselves. The
/// description from the `Display` impl is stored and used in the `Error` impl.
#[derive(Debug)]
pub struct Wrapped<E> {
    inner: E,
    desc: String,
}

impl<E> Wrapped<E> {
    /// Get the inner error value of type `E`.
    pub fn inner(&self) -> &E {
        &self.inner
    }
    /// Get the stored description string.
    pub fn desc(&self) -> &str {
        &self.desc
    }
}

impl<E: Display> From<E> for Wrapped<E> {
    fn from(e: E) -> Wrapped<E> {
        let desc = e.to_string();
        Wrapped {
            inner: e,
            desc: desc,
        }
    }
}

impl<E: Display> Display for Wrapped<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.desc, f)
    }
}

impl<E: Display + Debug + Any> Error for Wrapped<E> {
    fn description(&self) -> &str {
        &self.desc
    }
    fn cause(&self) -> Option<&Error> {
        None
    }
}
