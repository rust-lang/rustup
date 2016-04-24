extern crate backtrace;

pub use backtrace::Backtrace;

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
                               pub $crate::Backtrace);

        #[allow(unused)]
        impl $error_name {
            pub fn inner(&self) -> &$error_kind_name {
                &self.0
            }

            pub fn into_inner(self) -> $error_kind_name {
                self.0
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
                        $crate::Backtrace::new())
                }
            }
        ) *

        impl From<$error_kind_name> for $error_name {
            fn from(e: $error_kind_name) -> Self {
                $error_name(e, None, $crate::Backtrace::new())
            }
        }

        impl<'a> From<&'a str> for $error_name {
            fn from(s: &'a str) -> Self {
                $error_name(s.into(), None, $crate::Backtrace::new())
            }
        }

        impl From<String> for $error_name {
            fn from(s: String) -> Self {
                $error_name(s.into(), None, $crate::Backtrace::new())
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



        // Lesser types
        // ------------

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
                    // FIXME: Unfortunate backtrace if E already has one
                    $error_name(callback().into(), Some(Box::new(e)), $crate::Backtrace::new())
                })
            }
        }

        pub type $result_name<T> = ::std::result::Result<T, $error_name>;
    };
}
