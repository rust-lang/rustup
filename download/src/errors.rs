use error_chain::error_chain;

error_chain! {
    links { }

    foreign_links {
        Io(std::io::Error);
        Reqwest(::reqwest::Error);
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
