# 0.1.11

* [Actually dispatch the `rustup install` command](https://github.com/rust-lang-nursery/rustup.rs/pull/444).
* [Migrate to libcurl instead of hyper](https://github.com/rust-lang-nursery/rustup.rs/pull/443).
* [Add error for downloading bogus versions](https://github.com/rust-lang-nursery/rustup.rs/pull/428).

# 0.1.10

* [Multiple cli improvements](https://github.com/rust-lang-nursery/rustup.rs/pull/419).
* [Support HTTP protocol again](https://github.com/rust-lang-nursery/rustup.rs/pull/431).
* [Improvements to welcome screen](https://github.com/rust-lang-nursery/rustup.rs/pull/418).
* [Don't try to update non-tracking channels](https://github.com/rust-lang-nursery/rustup.rs/pull/425).
* [Don't panic when NativeSslStream lock is poisoned](https://github.com/rust-lang-nursery/rustup.rs/pull/429).
* [Fix multiple issues in schannel bindings](https://github.com/sfackler/schannel-rs/pull/1)

# 0.1.9

* [Do TLS hostname verification](https://github.com/rust-lang-nursery/rustup.rs/pull/400).
* [Expand `rustup show`](https://github.com/rust-lang-nursery/rustup.rs/pull/406).
* [Add `rustup doc`](https://github.com/rust-lang-nursery/rustup.rs/pull/403).
* [Refuse to install if it looks like other Rust installations are present](https://github.com/rust-lang-nursery/rustup.rs/pull/408).
* [Update www platform detection for FreeBSD](https://github.com/rust-lang-nursery/rustup.rs/pull/399).
* [Fix color display during telemetry capture](https://github.com/rust-lang-nursery/rustup.rs/pull/394).
* [Make it less of an error for the self-update hash to be wrong](https://github.com/rust-lang-nursery/rustup.rs/pull/372).

# 0.1.8

* [Initial telemetry implementation (disabled)](https://github.com/rust-lang-nursery/rustup.rs/pull/289)
* [Add hash to `--version`](https://github.com/rust-lang-nursery/rustup.rs/pull/347)
* [Improve download progress](https://github.com/rust-lang-nursery/rustup.rs/pull/355)
* [Completely overhaul error handling](https://github.com/rust-lang-nursery/rustup.rs/pull/358)
* [Add armv7l support to www](https://github.com/rust-lang-nursery/rustup.rs/pull/359)
* [Overhaul website](https://github.com/rust-lang-nursery/rustup.rs/pull/363)

# 0.1.7

* [Fix overrides for Windows root directories](https://github.com/rust-lang-nursery/rustup.rs/pull/317).
* [Remove 'multirust' binary and rename crates](https://github.com/rust-lang-nursery/rustup.rs/pull/312).
* [Pass rustup-setup.sh arguments to rustup-setup](https://github.com/rust-lang-nursery/rustup.rs/pull/325).
* [Don't open /dev/tty if passed -y](https://github.com/rust-lang-nursery/rustup.rs/pull/334).
* [Add interactive install, `--default-toolchain` argument](https://github.com/rust-lang-nursery/rustup.rs/pull/293).
* [Rename rustup-setup to rustu-init](https://github.com/rust-lang-nursery/rustup.rs/pull/303).
