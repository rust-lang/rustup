#[macro_use]
mod macros;
mod sys;

pub use self::sys::CertBundle;

pub enum CertItem {
    File(String),
    Blob(Vec<u8>)
}
