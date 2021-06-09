pub(crate) fn stderr_isatty() -> bool {
    atty::is(atty::Stream::Stderr)
}

pub(crate) fn stdout_isatty() -> bool {
    atty::is(atty::Stream::Stdout)
}
