# FAQ

### Is this an official Rust project?

Yes. rustup is an official Rust project. It is the recommended way to install
Rust at https://www.rust-lang.org.

### How is this related to multirust?

rustup is the successor to [multirust]. rustup began as multirust-rs, a
rewrite of multirust from shell script to Rust, by [Diggory Blake], and is now
maintained by The Rust Project.

[multirust]: https://github.com/brson/multirust
[Diggory Blake]: https://github.com/Diggsey

### Can rustup download the Rust source code?

The Rust source can be obtained by running `rustup component add rust-src`. It
will be downloaded to the `<toolchain root>/lib/rustlib/src/rust` directory of
the current toolchain.

### rustup fails with Windows error 32

If `rustup` fails with Windows error 32, it may be due to antivirus scanning
in the background. Disable antivirus scanner and try again.

### I get "error: could not remove 'rustup-bin' file: 'C:\Users\USER\\.cargo\bin\rustup.exe'"

If `rustup` fails to self-update in this way it's usually because RLS is
running (your editor is open and running RLS). The solution is to stop RLS (by
closing your editor) and try again.

### rustup exited successfully but I can't run `rustc --version`

To get started you need Cargo's bin directory ({cargo_home}/bin) in your
`PATH` environment variable. This should be done by rustup. Next time you log
in this should have been done automatically.
