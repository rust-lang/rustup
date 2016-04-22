mod quick_error;

use std::fmt;
use std::result::Result as StdResult;
use std::error::Error as StdError;

#[derive(Debug)]
pub struct BasicError(String);

impl BasicError {
    pub fn new<S: AsRef<str>>(s: S) -> BasicError {
        BasicError(s.as_ref().to_string())
    }
}

impl StdError for BasicError {
    fn description(&self) -> &str { &self.0 }
}

impl fmt::Display for BasicError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[derive(Debug)]
pub struct ChainedError {
    error: Box<StdError + Send>,
    cause: Box<StdError + Send>,
}

pub trait ChainError<T> {
    fn chain_error<F, E>(self, callback: F) -> StdResult<T, ChainedError>
        where F: FnOnce() -> E,
              E: StdError + Send + 'static;
}

impl<T, E> ChainError<T> for StdResult<T, E>
    where E: StdError + Send + 'static
{
    fn chain_error<F, E2>(self, callback: F) -> StdResult<T, ChainedError>
        where F: FnOnce() -> E2, E2: StdError + Send + 'static
    {
        self.map_err(move |e| {
            ChainedError {
                error: Box::new(callback()),
                cause: Box::new(e),
            }            
        })
    }
}

impl StdError for ChainedError {
    fn description(&self) -> &str { self.error.description() }
    fn cause(&self) -> Option<&StdError> { Some(&*self.cause) }
}

impl fmt::Display for ChainedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
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
                BasicError(err: $crate::BasicError) {
                    description(::std::error::Error::description(err))
                    from()
                }
                ChainedError(err: $crate::ChainedError) {
                    description(::std::error::Error::description(err))
                    cause(::std::error::Error::cause(err).unwrap())
                    from()
                }
                $($chunks)*
            }
        }
    };
    (   $(#[$meta:meta])*
        enum $name:ident { $($chunks:tt)* }
    ) => {
        quick_error! {
            $(#[$meta])*
            enum $name {
                BasicError(err: $crate::BasicError) {
                    description(::std::error::Error::description(err))
                    from()
                }
                ChainedError(err: $crate::ChainedError) {
                    description(::std::error::Error::description(err))
                    cause(::std::error::Error::cause(err).unwrap())
                    from()
                }
                $($chunks)*
            }
        }
    };
}

#[cfg(test)]
mod test {
    #[test]
    fn chained_error() {
        use super::{BasicError, ChainError};
        use std::error::Error;
        let r: Result<(), BasicError> = Err(BasicError::new("test"));
        let r = r.chain_error(|| BasicError::new("test2"));
        let r = r.unwrap_err();
        assert!(r.description() == "test2");
    }

    #[test]
    fn custom_chained_error() {
        use super::{BasicError, ChainError};
        use std::error::Error;

        easy_error! {
            #[derive(Debug)]
            pub enum CustomError {
            }
        };

        let r: Result<(), BasicError> = Err(BasicError::new("test"));
        let r = r.chain_error(|| BasicError::new("test2"));
        let r = r.unwrap_err();
        let r: CustomError = From::from(r);
        assert!(r.description() == "test2");
    }
}
