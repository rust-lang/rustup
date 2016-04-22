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
