# Security

`rustup` is secure enough for most people, but it [still needs work][s].
`rustup` performs all downloads over HTTPS, but does not yet validate
signatures of downloads.

[s]: https://github.com/rust-lang/rustup/issues?q=is%3Aopen+is%3Aissue+label%3Asecurity

File modes on installation honor umask as of 1.18.4, use umask if very tight
controls are desired.

If you wish to report a security issue, please follow the [Rust security
policy].

[Rust security policy]: https://www.rust-lang.org/policies/security
