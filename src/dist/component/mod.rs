pub use self::components::*;
pub use self::package::*;
/// An interpreter for the rust-installer [1] installation format.
///
/// https://github.com/rust-lang/rust-installer
pub use self::transaction::*;

// Transactional file system tools
mod transaction;
// The representation of a package, its components, and installation
mod package;
// The representation of *installed* components, and uninstallation
mod components;

#[cfg(test)]
mod tests;
