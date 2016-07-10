extern crate curl;

use std::error::Error as StdError;

error_chain! {
    types {
        Error, ErrorKind, ChainErr, Result;
    }

    links { }

    foreign_links { }

    errors {
        DownloadError(e: Box<StdError + Send + 'static>) {
            description("http request did not succeed")
            display("http request returned failure: {}", e)
        }
        HttpStatus(e: u32) {
            description("http request returned an unsuccessful status code")
            display("http request returned an unsuccessful status code: {}", e)
        }
        FileNotFound {
            description("file not found")
        }
    }
}
