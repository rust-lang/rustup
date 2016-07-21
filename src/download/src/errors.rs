error_chain! {
    links { }

    foreign_links { }

    errors {
        HttpStatus(e: u32) {
            description("http request returned an unsuccessful status code")
            display("http request returned an unsuccessful status code: {}", e)
        }
        FileNotFound {
            description("file not found")
        }
        BackendUnavailable(be: &'static str) {
            description("download backend unavailable")
            display("download backend '{}' unavailable", be)
        }
    }
}
