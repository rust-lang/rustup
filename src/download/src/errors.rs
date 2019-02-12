use error_chain::error_chain;
use error_chain::error_chain_processing;
use error_chain::{impl_error_chain_kind, impl_error_chain_processed, impl_extract_backtrace};

error_chain! {
    links { }

    foreign_links {
        Io(::std::io::Error);
        Reqwest(::reqwest::Error) #[cfg(feature = "reqwest-backend")];
    }

    errors {
        HttpStatus(e: u32) {
            description("http request returned an unsuccessful status code")
            display("http request returned an unsuccessful status code: {}", e)
        }
        FileNotFound {
            description("file not found")
        }
    }
}
