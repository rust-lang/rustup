extern crate curl;

error_chain! {
    types {
        Error, ErrorKind, ChainErr, Result;
    }

    links { }

    foreign_links { }

    errors {
        HttpError(e: curl::Error) {
            description("http request did not succeed")
            display("http request returned failure: {}", e)
        }
        HttpStatus(e: u32) {
            description("http request returned an unsuccessful status code")
            display("http request returned an unsuccessful status code: {}", e)
        }
    }
}
