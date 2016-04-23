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
    (

        $(#[$error_chain_meta:meta])*
        pub error_chain $error_chain_name:ident;

        $(#[$error_meta:meta])*
        pub error $error_name:ident { $($error_chunks:tt)* }

    ) => {

        $(#[$error_chain_meta])*
        pub struct $error_chain_name(pub $error_name, pub Option<Box<::std::error::Error + Send>>);

        impl $crate::BuildChain<$error_name> for $error_chain_name {
            fn new_chain(e: $error_name) -> Self {
                $error_chain_name(e, None)
            }

            fn extend_chain<SE>(e: $error_name, c: SE) -> Self
                where SE: ::std::error::Error + Send + 'static
            {
                $error_chain_name(e, Some(Box::new(c)))
            }
        }

        impl ::std::error::Error for $error_chain_name {
            fn description(&self) -> &str { self.0.description() }
            fn cause(&self) -> Option<&::std::error::Error> {
                match self.1 {
                    Some(ref c) => Some(&**c),
                    None => None
                }
            }
        }

        impl ::std::fmt::Display for $error_chain_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }

        quick_error! {
            $(#[$error_meta])*
            pub enum $error_name {
                $($error_chunks)*
            }
        }

        impl $error_name {
            pub fn unchained(self) -> $error_chain_name {
                $crate::BuildChain::new_chain(self)
            }

            pub fn chained<E>(self, e: E) -> $error_chain_name
                where E: ::std::error::Error + Send + 'static
            {
                $crate::BuildChain::extend_chain(self, e)
            }
        }

    };
}
