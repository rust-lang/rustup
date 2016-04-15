/// An interpreter for the rust-installer [1] installation format.
///
/// https://github.com/rust-lang/rust-installer

pub use self::transaction::*;
pub use self::components::*;
pub use self::package::*;

// Transactional file system tools
mod transaction;
// The representation of a package, its components, and installation
//
// FIXME: Because of rust-lang/rust#18241 this must be pub to pub reexport
// the inner Package trait.
pub mod package;
// The representation of *installed* components, and uninstallation
mod components;

