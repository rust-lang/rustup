mod quick_error;

use std::result::Result as StdResult;
use std::error::Error as StdError;


pub trait BuildChain<E> {
    fn new_chain(e: E) -> Self;
    fn extend_chain<SE>(e: E, c: SE) -> Self
        where SE: StdError + Send + 'static;
}

pub trait ChainError<T> {
    fn chain_error<F, E, EC>(self, callback: F) -> StdResult<T, EC>
        where F: FnOnce() -> E,
              E: StdError + Send + 'static,
              EC: BuildChain<E>;
}

impl<T, E> ChainError<T> for StdResult<T, E>
    where E: StdError + Send + 'static
{
    fn chain_error<F, E2, EC>(self, callback: F) -> StdResult<T, EC>
        where F: FnOnce() -> E2,
              E2: StdError + Send + 'static,
              EC: BuildChain<E2>
    {
        self.map_err(move |e| {
            BuildChain::extend_chain(callback(), e)
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
                $crate::BuildChain::new_chain(self)
            }

            pub fn chained<E>(self, e: E) -> ErrorChain<$name>
                where E: ::std::error::Error + Send + 'static
            {
                $crate::BuildChain::extend_chain(self, e)
            }
        }

        #[derive(Debug)]
        pub struct ErrorChain<E>(pub E, pub Option<Box<::std::error::Error + Send>>);

        impl<E> $crate::BuildChain<E> for ErrorChain<E> {
            fn new_chain(e: E) -> Self {
                ErrorChain(e, None)
            }

            fn extend_chain<SE>(e: E, c: SE) -> Self
                where SE: ::std::error::Error + Send + 'static
            {
                ErrorChain(e, Some(Box::new(c)))
            }
        }

        impl<E> ::std::error::Error for ErrorChain<E>
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

        impl<E> ::std::fmt::Display for ErrorChain<E>
            where E: ::std::fmt::Display
        {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }

    };
}
