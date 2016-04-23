mod quick_error;

#[derive(Debug)]
pub struct ForeignError {
    pub description: String,
    pub display: String,
}

impl ForeignError {
    pub fn new<E>(e: &E) -> ForeignError
        where E: ::std::error::Error
    {
        ForeignError {
            description: e.description().to_string(),
            display: format!("{}", e)
        }
    }
}

#[macro_export]
macro_rules! easy_error {
    (
        chain_error $chain_error_name:ident;

        error_chain $error_chain_name:ident;

        error $error_name:ident { $($error_chunks:tt)* }

    ) => {

        pub trait $chain_error_name<T> {
            fn chain_error<F>(self, callback: F) -> ::std::result::Result<T, $error_chain_name>
                where F: FnOnce() -> $error_name;
        }

        impl<T, E> $chain_error_name<T> for ::std::result::Result<T, E>
            where E: ::std::error::Error + Send + 'static
        {
            fn chain_error<F>(self, callback: F) -> ::std::result::Result<T, $error_chain_name>
                where F: FnOnce() -> $error_name
            {
                self.map_err(move |e| {
                    $error_chain_name::extend_chain(callback(), e)
                })
            }
        }

        #[derive(Debug)]
        pub struct $error_chain_name(pub $error_name, pub Option<Box<::std::error::Error + Send>>);

        impl $error_chain_name {
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
            #[derive(Debug)]
            pub enum $error_name {
                $($error_chunks)*
            }
        }

        impl $error_name {
            pub fn unchained(self) -> $error_chain_name {
                $error_chain_name::new_chain(self)
            }

            pub fn chained<E>(self, e: E) -> $error_chain_name
                where E: ::std::error::Error + Send + 'static
            {
                $error_chain_name::extend_chain(self, e)
            }
        }

    };
}
