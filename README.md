# multirust-rs

Multirust-rs is a reimplementation of multirust in rust. It provides both a command line interface, and a rust library, so it's trivial to integrate it with external tools.

## Documentation

- [multirust](http://diggsey.github.io/multirust-rs/multirust/index.html)
- [rust-install](http://diggsey.github.io/multirust-rs/rust_install/index.html)


## Installation

Currently an installer is the missing piece. Manual installation is described below:

- Run `cargo build --release` to build the multirust-rs binary
- Add the binary to your PATH
- Add symlinks for `rustc`, `cargo`, `rustdoc`, and optionally `rust-lldb` and `rust-gdb`
- All symlinks should point to the same `multirust-rs` binary
- Done!

## Usage

```
USAGE:
        multirust-rs [FLAGS] [SUBCOMMAND]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    ctl
    default             Set the default toolchain.
    delete-data         Delete all user metadata.
    doc                 Open the documentation for the current toolchain.
    help                Prints this message
    list-overrides      List all overrides.
    list-toolchains     List all installed toolchains.
    override            Set the toolchain override.
    remove-override     Remove an override.
    remove-toolchain    Uninstall a toolchain.
    run                 Run a command.
    show-default        Show information about the current default.
    show-override       Show information about the current override.
    update              Install or update a given toolchain.
    upgrade-data        Upgrade the ~/.multirust directory.
    which               Report location of the currently active Rust tool.
```

## Contributing

1. Fork it!
2. Create your feature branch: `git checkout -b my-new-feature`
3. Commit your changes: `git commit -am 'Add some feature'`
4. Push to the branch: `git push origin my-new-feature`
5. Submit a pull request :D

## License

MIT
