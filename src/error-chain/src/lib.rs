//! A library for consistent and reliable error handling
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
//! Similar to other libraries like [error-type] and [quick-error], this
//! library defines a macro, `error_chain!` that declares the types
//! and implementation boilerplate necessary for fulfilling a
//! particular error-handling strategy. Most importantly it defines
//! a custom error type (called `Error` by convention) and the `From`
//! conversions that let the `try!` macro and `?` operator work.
//!
//! This library differs in a few ways from previous error libs:
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
//!   defined by the `error_chain!` that preserve type information,
//!   and facilitate seamless error composition and matching of composed
//!   errors.
//! * It provides automatic `From` conversions between any other error
//!   type that hides the type of the other error in the `cause` box.
//! * It collects a single backtrace at the earliest opportunity and
//!   propagates it down the stack through `From` and `ChainErr`
//!   conversions.
//!
//! To accomplish its goals it makes some tradeoffs:
//!
//! * The split between the `Error` and `ErrorKind` types can make it
//!   slightly more cumbersome to instantiate new (unchained) errors
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
//! define an `errors` module and inside it call `error_chain!`:
//!
//! ```rust
//! error_chain! {
//!     // The type defined for this error. These are the conventional
//!     // and recommended names, but they can be arbitrarily chosen.
//!     types {
//!         Error, ErrorKind, ChainErr, Result;
//!     }
//!
//!     // Automatic conversions between this error chain and other
//!     // error chains. In this case, it will e.g. generate an
//!     // `ErrorKind` variant called `Dist` which in turn contains
//!     // the `rustup_dist::ErrorKind`, with conversions from
//!     // `rustup_dist::Error`.
//!     //
//!     // This section can be empty.
//!     links {
//!         rustup_dist::Error, rustup_dist::ErrorKind, Dist;
//!         rustup_utils::Error, rustup_utils::ErrorKind, Utils;
//!     }
//!
//!     // Automatic conversions between this error chain and other
//!     // error types not defined by the `error_chain!`. These will be
//!     // boxed as the error cause and wrapped in a new error with,
//!     // in this case, the `ErrorKind::Temp` variant.
//!     //
//!     // This section can be empty.
//!     foreign_links {
//!         temp::Error, Temp,
//!         "temporary file error";
//!     }
//!
//!     // Define additional `ErrorKind` variants. The syntax here is
//!     // the same as `quick_error!`, but the `from()` and `cause()`
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
//! This populates the module with a number of definitions,
//! the most important of which are the `Error` type
//! and the `ErrorKind` type. They look something like the
//! following:
//!
//! ```rust
//! use std::error::Error as StdError;
//! use std::sync::Arc;
//!
//! #[derive(Debug)]
//! pub struct Error(pub ErrorKind,
//!                  pub Option<Box<StdError + Send>>,
//!                  pub Arc<error_chain::Backtrace>);
//!
//! impl Error {
//!     pub fn kind(&self) -> &ErrorKind { ... }
//!     pub fn into_kind(self) -> ErrorKind { ... }
//!     pub fn iter(&self) -> error_chain::ErrorChainIter { ... }
//!     pub fn backtrace(&self) -> &error_chain::Backtrace { ... }
//! }
//!
//! impl StdError for Error { ... }
//! impl Display for Error { ... }
//!
//! #[derive(Debug)]
//! pub enum ErrorKind {
//!     Msg(String),
//!     Dist(rustup_dist::ErrorKind),
//!     Utils(rustup_utils::ErrorKind),
//!     Temp,
//!     InvalidToolchainName(String),
//! }
//! ```
//!
//! This is the basic error structure. You can see that `ErrorKind`
//! has been populated in a variety of ways. All `ErrorKind`s get a
//! `Msg` variant for basic errors. When strings are converted to
//! `ErrorKind`s they become `ErrorKind::Msg`. The "links" defined in
//! the macro are expanded to `Dist` and `Utils` variants, and the
//! "foreign links" to the `Temp` variant.
//!
//! Both types come with a variety of `From` conversions as well:
//! `Error` can be created from `ErrorKind`, `&str` and `String`,
//! and the "link" and "foreign_link" error types. `ErrorKind`
//! can be created from the corresponding `ErrorKind`s of the link
//! types, as well as from `&str` and `String`.
//!
//! `into()` and `From::from` are used heavily to massage types into
//! the right shape. Which one to use in any specific case depends on
//! the influence of type inference, but there are some patterns that
//! arise frequently.
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
//! Note that the return type is the typedef `Result`, which is
//! defined by the macro as `pub type Result<T> =
//! ::std::result::Result<T, Error>`. Note that in both cases
//! `.into()` is called to convert a type into the `Error` type; both
//! strings and `ErrorKind` have `From` conversions to turn them into
//! `Error`.
//!
//! When the error is emitted inside a `try!` macro or behind the
//! `?` operator, the explicit conversion isn't needed; `try!` will
//! automatically convert `Err(ErrorKind)` to `Err(Error)`. So the
//! below is equivalent to the previous:
//!
//! ```rust
//! fn foo() -> Result<()> {
//!     Ok(try!(Err(ErrorKind::FooError)))
//! }
//!
//! fn bar() -> Result<()> {
//!     Ok(try!(Err("bogus!")))
//! }
//! ```
//!
//! ## Chaining errors
//!
//! To extend the error chain:
//!
//! ```
//! use errors::ChainErr;
//! try!(do_something().chain_err(|| "something went wrong"));
//! ```
//!
//! `chain_err` can be called on any `Result` type where the contained
//! error type implements `std::error::Error + Send + 'static`.  If
//! the `Result` is an `Err` then `chain_err` evaluates the closure,
//! which returns *some type that can be converted to `ErrorKind`*,
//! boxes the original error to store as the cause, then returns a new
//! error containing the original error.
//!
//! ## Foreign links
//!
//! Errors that do not conform to the same conventions as this library
//! can still be included in the error chain. They are considered "foreign
//! errors", and are declared using the `foreign_links` block of the
//! `error_chain!` macro. `Error`s are automatically created from
//! foreign errors by the `try!` macro.
//!
//! Foreign links and regular links have one crucial difference:
//! `From` conversions for regular links *do not introduce a new error
//! into the error chain*, while conversions for foreign links *always
//! introduce a new error into the error chain*. So for the example
//! above all errors deriving from the `temp::Error` type will be
//! presented to the user as a new `ErrorKind::Temp` variant, and the
//! cause will be the original `temp::Error` error. In contrast, when
//! `rustup_utils::Error` is converted to `Error` the two `ErrorKinds`
//! are converted between each other to create a new `Error` but the
//! old error is discarded; there is no "cause" created from the
//! original error.
//!
//! ## Backtraces
//!
//! The earliest non-foreign error to be generated creates a single
//! backtrace, which is passed through all `From` conversions and
//! `chain_err` invocations of compatible types. To read the backtrace
//! just call the `backtrace()` method.
//!
//! ## Iteration
//!
//! The `iter` method returns an iterator over the chain of error boxes.
//!
//! [error-type]: https://github.com/DanielKeep/rust-error-type
//! [quick-error]: https://github.com/tailhook/quick-error

extern crate backtrace;

pub use backtrace::Backtrace;

mod quick_error;

#[macro_export]
macro_rules! error_chain {
    (
        types {
            $error_name:ident, $error_kind_name:ident,
            $chain_error_name:ident, $result_name:ident;
        }

        links {
            $( $link_error_path:path, $link_kind_path:path, $link_variant:ident;  ) *
        }

        foreign_links {
            $( $foreign_link_error_path:path, $foreign_link_variant:ident,
               $foreign_link_desc:expr;  ) *
        }

        errors {
            $( $error_chunks:tt ) *
        }

    ) => {


        // The Error type
        // --------------

        // This has a simple structure to support pattern matching
        // during error handling. The second field is internal state
        // that is mostly irrelevant for error handling purposes.
        #[derive(Debug)]
        pub struct $error_name(pub $error_kind_name,
                               pub (Option<Box<::std::error::Error + Send>>,
                                    ::std::sync::Arc<$crate::Backtrace>));

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
                &(self.1).1
            }
        }

        impl ::std::error::Error for $error_name {
            fn description(&self) -> &str { self.0.description() }
            fn cause(&self) -> Option<&::std::error::Error> {
                match (self.1).0 {
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
                    $error_name($error_kind_name::$link_variant(e.0), e.1)
                }
            }
        ) *

        $(
            impl From<$foreign_link_error_path> for $error_name {
                fn from(e: $foreign_link_error_path) -> Self {
                    $error_name(
                        $error_kind_name::$foreign_link_variant,
                        (Some(Box::new(e)),
                         ::std::sync::Arc::new($crate::Backtrace::new())))
                }
            }
        ) *

        impl From<$error_kind_name> for $error_name {
            fn from(e: $error_kind_name) -> Self {
                $error_name(e,
                            (None, ::std::sync::Arc::new($crate::Backtrace::new())))
            }
        }

        impl<'a> From<&'a str> for $error_name {
            fn from(s: &'a str) -> Self {
                $error_name(s.into(),
                            (None, ::std::sync::Arc::new($crate::Backtrace::new())))
            }
        }

        impl From<String> for $error_name {
            fn from(s: String) -> Self {
                $error_name(s.into(),
                            (None, ::std::sync::Arc::new($crate::Backtrace::new())))
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
                    $foreign_link_variant {
                        description(&$foreign_link_desc)
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

                    $error_name(callback().into(), (Some(e), backtrace))
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
                    backtrace = Some((e.1).1.clone());
                    e as Box<::std::error::Error + Send + 'static>
                }
            };

            $(

                e = match e.downcast::<$link_error_path>() {
                    Err(e) => e,
                    Ok(e) => {
                        backtrace = Some((e.1).1.clone());
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


use std::error::Error as StdError;
use std::iter::Iterator;

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

