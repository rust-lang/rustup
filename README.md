# multirust-rs

Multirust-rs is a reimplementation of multirust in rust. It provides both a command line interface, and a rust library, so it's trivial to integrate it with external tools.

## Documentation

- [multirust](http://diggsey.github.io/multirust-rs/multirust/index.html)
- [rust-install](http://diggsey.github.io/multirust-rs/rust_install/index.html)


## Installation

### Installing from binaries

- [Windows GNU 64-bit installer](https://github.com/Diggsey/multirust-rs-binaries/raw/master/x86_64-pc-windows-gnu/multirust-rs.exe)
- [Windows MSVC 64-bit installer](https://github.com/Diggsey/multirust-rs-binaries/raw/master/x86_64-pc-windows-msvc/multirust-rs.exe)
- [Windows GNU 32-bit installer](https://github.com/Diggsey/multirust-rs-binaries/raw/master/i686-pc-windows-gnu/multirust-rs.exe)
- [Windows MSVC 32-bit installer](https://github.com/Diggsey/multirust-rs-binaries/raw/master/i686-pc-windows-msvc/multirust-rs.exe)
- [Linux 64-bit installer](https://github.com/Diggsey/multirust-rs-binaries/raw/master/x86_64-unknown-linux-gnu/multirust-rs)

Binaries for other platforms are not yet available. Follow the instructions below for installing from source.


### Installing from source

Run this command in a writable directory:
```
git clone --depth 1 https://github.com/Diggsey/multirust-rs.git multirust-rs && cd multirust-rs && cargo run --release install [-a]
```

Passing `-a` will attempt to automatically add `~/.multirust/bin` to your PATH.

On linux, this is done by appending to `~/.profile`.
On windows, this is done by modifying the registry entry `HKCU\Environment\PATH`.

The changes to PATH will not take effect immediately within the same terminal.

The `multirust-rs` directory which is created is no longer required once installation has completed, but keeping it around will make future updates much faster:

```
cd multirust-rs && git pull && cargo run --release install
```


## Usage

```
USAGE:
        multirust [FLAGS] [SUBCOMMAND]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Enable verbose output

SUBCOMMANDS:
    ctl
    default             Set the default toolchain.
    delete-data         Delete all user metadata.
    doc                 Open the documentation for the current toolchain.
    help                Prints this message
    install             Installs multirust.
    list-overrides      List all overrides.
    list-toolchains     List all installed toolchains.
    override            Set the toolchain override.
    remove-override     Remove an override.
    remove-toolchain    Uninstall a toolchain.
    run                 Run a command.
    show-default        Show information about the current default.
    show-override       Show information about the current override.
    uninstall           Uninstalls multirust.
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
