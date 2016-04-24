//! A library for consistent and reliable error handling
//!
//! Based on quick_error! and Cargo's chain_error method.
//!
//! This crate defines an opinionated strategy for error handling in Rust,
//! built on the following principles:
//!
//! * No error should ever be discarded. This library primarily
//!   makes it easy to "chain" errors with the `chain_err` method.
//! * Introducing new errors is trivial. Simple errors can be introduced
//!   at the error site with just a string.
//! * Handling errors is possible with pattern matching.
//! * Conversions between error types are done in an automatic and
//!   consistent way - `From` conversion behavior is never specified
//!   explicitly.
//! * Errors implement Send.
//! * Errors carry backtraces.
//!
//! Similar to other libraries like error-type and quick-error, this
//! library defines a macro, `declare_errors!` that declares the types
//! and implementation boilerplate necessary for fulfilling a
//! particular error-hadling strategy. Most importantly it defines
//! a custom error type (called `Error` by convention) and the `From`
//! conversions that let the `try!` macro and `?` operator work.
//!
//! This library differs in a few ways:
//!
//! * Instead of defining the custom `Error` type as an enum, it is a
//!   struct containing an `ErrorKind` (which defines the
//!   `description` and `display` methods for the error), an opaque,
//!   optional, boxed `std::error::Error + Send + 'static` object
//!   (which defines the `cause`, and establishes the links in the
//!   error chain), and a `Backtrace`.
//! * The macro additionally defines a trait, by convention called
//!   `ChainErr`, that defines a `chain_err` method. This method
//!   on all `std::error::Error + Send + 'static` types extends
//!   the error chain by boxing the current error into an opaque
//!   object and putting it inside a new concrete error.
//! * It provides automatic `From` conversions between other error types
//!   defined by the `declare_errors!` that preserve type information.
//! * It provides automatic `From` conversions between any other error
//!   type that hide the type of the other error in the `cause` box.
//! * It collects a single backtrace at the earliest opportunity and
//!   propagates it down the stack through `From` and `ChainErr`
//!   conversions.
//!
//! To accomplish its goals it makes some tradeoffs:
//!
//! * The split between the `Error` and `ErrorKind` types can make it
//!   slightly more cumbersome to introduce new, unchained,
//!   errors, requiring an `Into` or `From` conversion; as well as
//!   slightly more cumbersome to match on errors with another layer
//!   of types to match.
//! * Because the error type contains `std::error::Error + Send + 'static` objects,
//!   it can't implement `PartialEq` for easy comparisons.
//!
//! ## Declaring error types
//!
//! Generally, you define one family of error types per crate, though
//! it's also perfectly fine to define error types on a finer-grained
//! basis, such as per module.
//!
//! Assuming you are using crate-level error types, typically you will
//! define an `errors` module and inside it call `declare_errors!`:
//!
//! ```rust
//! declare_errors! {
//!     // The type defined for this error. These are the conventional
//!     // and recommended names, but they can be arbitrarily chosen.
//!     types {
//!         Error, ErrorKind, ChainErr, Result;
//!     }
//!
//!     // Automatic conversions between this error chain and other
//!     // error chains. In this case, it will generate an
//!     // `ErrorKind` variant in turn containing `rustup_utils::ErrorKind`,
//!     // with conversions from `rustup_utils::Error`.
//!     //
//!     // This section can be empty.
//!     links {
//!         rustup_dist::Error, rustup_dist::ErrorKind, Dist;
//!         rustup_utils::Error, rustup_utils::ErrorKind, Utils;
//!     }
//!
//!     // Automatic conversions between this error chain and other
//!     // error types not defined by this macro. These will be boxed
//!     // as the error cause, and their descriptions and display text
//!     // reused.
//!
//!     // This section can be empty.
//!     foreign_links {
//!         temp::Error, Temp;
//!     }
//!
//!     // Define the `ErrorKind` variants. The syntax here is the
//!     // same as quick_error!, but the `from()` and `cause()`
//!     // syntax is not supported.
//!     errors {
//!         InvalidToolchainName(t: String) {
//!             description("invalid toolchain name")
//!             display("invalid toolchain name: '{}'", t)
//!         }
//!     }
//! }
//! ```
//!
//! ## Returning new errors
//!
//! Introducing new error chains, with a string message:
//!
//! ```rust
//! fn foo() -> Result<()> {
//!     Err("foo error!".into())
//! }
//! ```
//!
//! Introducing new error chains, with an `ErrorKind`:
//!
//! ```rust
//! fn foo() -> Result<()> {
//!     Err(ErrorKind::FooError.into())
//! }
//! ```
//!
//! Note that the return type is is the typedef `Result`, which is
//! defined by the macro as `pub type Result<T> =
//! ::std::result::Result<T, Error>`. Note that in both cases
//! `.into()` is called to convert a type into the `Error` type: both
//! strings and `ErrorKind` have `From` conversions to turn them into
//! `Error`.
//!
//! When the error is emitted inside a `try!` macro or behind the
//! `?` operator, then the explicit conversion isn't needed, since
//! the behavior of `try!` will automatically convert `Err(ErrorKind)`
//! to `Err(Error)`. So the below is equivalent to the previous:
//!
//! ```rust
//! fn foo() -> Result<()> {
//!     Ok(try!(Err(ErrorKind::FooError)))
//! }
//!
//! fn bar() -> Result<()> {
//!     Ok(try!(Err("bogus!")))
//! ```
//!
//! ## Chaining errors
//!
//! TODO
//!
//! ## Misc
//!
//! iteration, backtraces, foreign errors
//!

extern crate backtrace;

use std::error::Error as StdError;
use std::iter::Iterator;

pub use backtrace::Backtrace;

mod quick_error;

#[derive(Debug)]
pub struct ForeignError {
    pub description: String,
    pub display: String,
}

impl ForeignError {
    pub fn new<E>(e: &E) -> ForeignError
        where E: StdError
    {
        ForeignError {
            description: e.description().to_string(),
            display: format!("{}", e)
        }
    }
}

pub struct ErrorChainIter<'a>(pub Option<&'a StdError>);

impl<'a> Iterator for ErrorChainIter<'a> {

    type Item = &'a StdError;

    fn next<'b>(&'b mut self) -> Option<&'a StdError> {
        match self.0.take() {
            Some(e) => {
                self.0 = e.cause();
                Some(e)
            }
            None => None
        }
    }
}

#[macro_export]
macro_rules! declare_errors {
    (
        types {
            $error_name:ident, $error_kind_name:ident,
            $chain_error_name:ident, $result_name:ident;
        }

        links {
            $( $link_error_path:path, $link_kind_path:path, $link_variant:ident;  ) *
        }

        foreign_links {
            $( $foreign_link_error_path:path, $foreign_link_variant:ident;  ) *
        }

        errors {
            $( $error_chunks:tt ) *
        }

    ) => {


        // The Error type
        // --------------

        #[derive(Debug)]
        pub struct $error_name(pub $error_kind_name,
                               pub Option<Box<::std::error::Error + Send>>,
                               pub ::std::sync::Arc<$crate::Backtrace>);

        #[allow(unused)]
        impl $error_name {
            pub fn kind(&self) -> &$error_kind_name {
                &self.0
            }

            pub fn into_kind(self) -> $error_kind_name {
                self.0
            }

            pub fn iter(&self) -> $crate::ErrorChainIter {
                $crate::ErrorChainIter(Some(self))
            }

            pub fn backtrace(&self) -> &$crate::Backtrace {
                &self.2
            }
        }

        impl ::std::error::Error for $error_name {
            fn description(&self) -> &str { self.0.description() }
            fn cause(&self) -> Option<&::std::error::Error> {
                match self.1 {
                    Some(ref c) => Some(&**c),
                    None => None
                }
            }
        }

        impl ::std::fmt::Display for $error_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }

        $(
            impl From<$link_error_path> for $error_name {
                fn from(e: $link_error_path) -> Self {
                    $error_name($error_kind_name::$link_variant(e.0), e.1, e.2)
                }
            }
        ) *

        $(
            impl From<$foreign_link_error_path> for $error_name {
                fn from(e: $foreign_link_error_path) -> Self {
                    $error_name(
                        $error_kind_name::$foreign_link_variant($crate::ForeignError::new(&e)),
                        Some(Box::new(e)),
                        ::std::sync::Arc::new($crate::Backtrace::new()))
                }
            }
        ) *

        impl From<$error_kind_name> for $error_name {
            fn from(e: $error_kind_name) -> Self {
                $error_name(e, None,
                            ::std::sync::Arc::new($crate::Backtrace::new()))
            }
        }

        impl<'a> From<&'a str> for $error_name {
            fn from(s: &'a str) -> Self {
                $error_name(s.into(), None,
                            ::std::sync::Arc::new($crate::Backtrace::new()))
            }
        }

        impl From<String> for $error_name {
            fn from(s: String) -> Self {
                $error_name(s.into(), None,
                            ::std::sync::Arc::new($crate::Backtrace::new()))
            }
        }


        // The ErrorKind type
        // --------------

        quick_error! {
            #[derive(Debug)]
            pub enum $error_kind_name {

                Msg(s: String) {
                    description(&s)
                    display("{}", s)
                }

                $(
                    $link_variant(e: $link_kind_path) {
                        description(e.description())
                        display("{}", e)
                    }
                ) *

                $(
                    $foreign_link_variant(e: $crate::ForeignError) {
                        description(&e.description)
                        display("{}", e.display)
                    }
                ) *

                $($error_chunks)*
            }
        }

        $(
            impl From<$link_kind_path> for $error_kind_name {
                fn from(e: $link_kind_path) -> Self {
                    $error_kind_name::$link_variant(e)
                }
            }
        ) *

        impl<'a> From<&'a str> for $error_kind_name {
            fn from(s: &'a str) -> Self {
                $error_kind_name::Msg(s.to_string())
            }
        }

        impl From<String> for $error_kind_name {
            fn from(s: String) -> Self {
                $error_kind_name::Msg(s)
            }
        }


        // The ChainErr trait
        // ------------------

        pub trait $chain_error_name<T> {
            fn chain_err<F, EK>(self, callback: F) -> ::std::result::Result<T, $error_name>
                where F: FnOnce() -> EK,
                      EK: Into<$error_kind_name>;
        }

        impl<T, E> $chain_error_name<T> for ::std::result::Result<T, E>
            where E: ::std::error::Error + Send + 'static
        {
            fn chain_err<F, EK>(self, callback: F) -> ::std::result::Result<T, $error_name>
                where F: FnOnce() -> EK,
                      EK: Into<$error_kind_name>
            {
                self.map_err(move |e| {
                    let e = Box::new(e) as Box<::std::error::Error + Send + 'static>;
                    let (e, backtrace) = backtrace_from_box(e);
                    let backtrace = backtrace.unwrap_or_else(
                        || ::std::sync::Arc::new($crate::Backtrace::new()));

                    $error_name(callback().into(), Some(e), backtrace)
                })
            }
        }

        // Use downcasts to extract the backtrace from types we know,
        // to avoid generating a new one. It would be better to not
        // define this in the macro, but types need some additional
        // machinery to make it work.
        fn backtrace_from_box(mut e: Box<::std::error::Error + Send + 'static>)
                              -> (Box<::std::error::Error + Send + 'static>,
                                  Option<::std::sync::Arc<$crate::Backtrace>>) {
            let mut backtrace = None;

            e = match e.downcast::<$error_name>() {
                Err(e) => e,
                Ok(e) => {
                    backtrace = Some(e.2.clone());
                    e as Box<::std::error::Error + Send + 'static>
                }
            };

            $(

                e = match e.downcast::<$link_error_path>() {
                    Err(e) => e,
                    Ok(e) => {
                        backtrace = Some(e.2.clone());
                        e as Box<::std::error::Error + Send + 'static>
                    }
                };

            ) *

            (e, backtrace)
        }

        // The Result type
        // ---------------

        pub type $result_name<T> = ::std::result::Result<T, $error_name>;
    };
}
