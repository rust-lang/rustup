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
                    let backtrace = backtrace.unwrap_or_else(|| $crate::Backtrace::new());

                    $error_name(callback().into(), Some(e), backtrace)
                })
            }
        }

        fn backtrace_from_box(mut e: Box<::std::error::Error + Send + 'static>)
                              -> (Box<::std::error::Error + Send + 'static>, Option<$crate::Backtrace>) {
            let mut backtrace = None;

            e = match e.downcast::<$error_name>() {
                Err(e) => {
                    e as Box<::std::error::Error + Send + 'static>
                }
                Ok(e) => {
                    #[derive(Debug)]
                    struct ChainedError($error_kind_name,
                                        Option<Box<::std::error::Error + Send>>);

                    impl ::std::error::Error for ChainedError {
                        fn description(&self) -> &str { self.0.description() }
                        fn cause(&self) -> Option<&::std::error::Error> {
                            match self.1 {
                                Some(ref c) => Some(&**c),
                                None => None
                            }
                        }
                    }

                    impl ::std::fmt::Display for ChainedError {
                        fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                            ::std::fmt::Display::fmt(&self.0, f)
                        }
                    }

                    let e = *e;
                    backtrace = Some(e.2);
                    let e2 = ChainedError(e.0, e.1);
                    Box::new(e2) as Box<::std::error::Error + Send + 'static>
                }
            };

            $(

                e = match e.downcast::<$link_error_path>() {
                    Err(e) => {
                        e as Box<::std::error::Error + Send + 'static>
                    }
                    Ok(e) => {
                        #[derive(Debug)]
                        struct ChainedError($link_kind_path,
                                            Option<Box<::std::error::Error + Send>>);

                        impl ::std::error::Error for ChainedError {
                            fn description(&self) -> &str { self.0.description() }
                            fn cause(&self) -> Option<&::std::error::Error> {
                                match self.1 {
                                    Some(ref c) => Some(&**c),
                                    None => None
                                }
                            }
                        }

                        impl ::std::fmt::Display for ChainedError {
                            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                                ::std::fmt::Display::fmt(&self.0, f)
                            }
                        }

                        let e = *e;
                        backtrace = Some(e.2);
                        let e2 = ChainedError(e.0, e.1);
                        Box::new(e2) as Box<::std::error::Error + Send + 'static>
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
