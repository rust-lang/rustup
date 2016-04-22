mod quick_error;

use std::fmt;
use std::result::Result as StdResult;
use std::error::Error as StdError;

#[derive(Debug)]
pub struct ErrorChain<E>(pub E, pub Option<Box<StdError + Send>>);

impl<E> ErrorChain<E> {
    pub fn new_chain(e: E) -> ErrorChain<E>
    {
        ErrorChain(e, None)
    }

    pub fn extend_chain<SE>(e: E, c: SE) -> ErrorChain<E>
        where SE: StdError + Send + 'static
    {
        ErrorChain(e, Some(Box::new(c)))
    }
}

impl<E> StdError for ErrorChain<E>
    where E: StdError
{
    fn description(&self) -> &str { self.0.description() }
    fn cause(&self) -> Option<&StdError> {
        match self.1 {
            Some(ref c) => Some(&**c),
            None => None
        }
    }
}

impl<E> fmt::Display for ErrorChain<E>
    where E: fmt::Display
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

pub trait ChainError<T> {
    fn chain_error<F, E>(self, callback: F) -> StdResult<T, ErrorChain<E>>
        where F: FnOnce() -> E,
              E: StdError + Send + 'static;
}

impl<T, E> ChainError<T> for StdResult<T, E>
    where E: StdError + Send + 'static
{
    fn chain_error<F, E2>(self, callback: F) -> StdResult<T, ErrorChain<E2>>
        where F: FnOnce() -> E2, E2: StdError + Send + 'static
    {
        self.map_err(move |e| {
            ErrorChain::extend_chain(callback(), e)
        })
    }
}






pub trait BuildChain2<E> {
    fn new_chain(e: E) -> Self;
    fn extend_chain<SE>(e: E, c: SE) -> Self
        where SE: StdError + Send + 'static;
}

pub trait ChainError2<T> {
    fn chain_error<F, E, EC>(self, callback: F) -> StdResult<T, EC>
        where F: FnOnce() -> E,
              E: StdError + Send + 'static,
              EC: BuildChain2<E>;
}

impl<T, E> ChainError2<T> for StdResult<T, E>
    where E: StdError + Send + 'static
{
    fn chain_error<F, E2, EC>(self, callback: F) -> StdResult<T, EC>
        where F: FnOnce() -> E2,
              E2: StdError + Send + 'static,
              EC: BuildChain2<E2>
    {
        self.map_err(move |e| {
            BuildChain2::extend_chain(callback(), e)
        })
    }
}

#[macro_export]
macro_rules! easy_error {
    (   $(#[$meta:meta])*
        pub enum $name:ident { $($chunks:tt)* }
    ) => {
        quick_error! {
            $(#[$meta])*
            pub enum $name {
                $($chunks)*
            }
        }

        impl $name {
            pub fn unchained(self) -> ErrorChain<$name> {
                $crate::ErrorChain::new_chain(self)
            }

            pub fn chained<E>(self, e: E) -> ErrorChain<$name>
                where E: ::std::error::Error + Send + 'static
            {
                $crate::ErrorChain::extend_chain(self, e)
            }
        }

        #[derive(Debug)]
        pub struct ErrorChain2<E>(pub E, pub Option<Box<::std::error::Error + Send>>);

        impl<E> $crate::BuildChain2<E> for ErrorChain2<E> {
            fn new_chain(e: E) -> Self {
                ErrorChain2(e, None)
            }

            fn extend_chain<SE>(e: E, c: SE) -> Self
                where SE: ::std::error::Error + Send + 'static
            {
                ErrorChain2(e, Some(Box::new(c)))
            }
        }

        impl<E> ::std::error::Error for ErrorChain2<E>
            where E: ::std::error::Error
        {
            fn description(&self) -> &str { self.0.description() }
            fn cause(&self) -> Option<&::std::error::Error> {
                match self.1 {
                    Some(ref c) => Some(&**c),
                    None => None
                }
            }
        }

        impl<E> ::std::fmt::Display for ErrorChain2<E>
            where E: ::std::fmt::Display
        {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }

    };
    (   $(#[$meta:meta])*
        enum $name:ident { $($chunks:tt)* }
    ) => {
        quick_error! {
            $(#[$meta])*
            enum $name {
                $($chunks)*
            }
        }

        impl $name {
            fn unchained(self) -> ErrorChain<$name> {
                $crate::ErrorChain::new_chain(self)
            }

            fn chained<E>(self, e: E) -> ErrorChain<$name>
                where E: ::std::error::Error + Send + 'static
            {
                $crate::ErrorChain::extend_chain(self, e)
            }
        }
    };
}
