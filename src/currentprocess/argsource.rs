/// Abstracts over reading the current process environment variables as a
/// zero-cost abstraction to support threaded in-process testing.
use std::env;
use std::ffi::OsString;
use std::marker::PhantomData;

pub trait ArgSource {
    fn args(&self) -> Box<dyn Iterator<Item = String>>;
    fn args_os(&self) -> Box<dyn Iterator<Item = OsString>>;
}

/// Implements ArgSource with `std::env::args`
impl ArgSource for super::OSProcess {
    fn args(&self) -> Box<dyn Iterator<Item = String>> {
        Box::new(env::args())
    }
    fn args_os(&self) -> Box<dyn Iterator<Item = OsString>> {
        Box::new(env::args_os())
    }
}

/// Helper for ArgSource over `Vec<String>`
pub(crate) struct VecArgs<T> {
    v: Vec<String>,
    i: usize,
    _marker: PhantomData<T>,
}

impl<T> From<&Vec<String>> for VecArgs<T> {
    fn from(source: &Vec<String>) -> Self {
        let v = source.clone();
        VecArgs {
            v,
            i: 0,
            _marker: PhantomData,
        }
    }
}

impl<T: From<String>> Iterator for VecArgs<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.i == self.v.len() {
            return None;
        }
        let i = self.i;
        self.i += 1;
        Some(T::from(self.v[i].clone()))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.v.len(), Some(self.v.len()))
    }
}

impl ArgSource for super::TestProcess {
    fn args(&self) -> Box<dyn Iterator<Item = String>> {
        Box::new(VecArgs::<String>::from(&self.args))
    }
    fn args_os(&self) -> Box<dyn Iterator<Item = OsString>> {
        Box::new(VecArgs::<OsString>::from(&self.args))
    }
}
