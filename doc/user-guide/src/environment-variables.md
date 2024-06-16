# Environment variables

- `RUSTUP_LOG` (default: none). Enables Rustup's "custom logging mode". In this mode,
  the verbosity of Rustup's log lines can be specified with `tracing_subscriber`'s
  [directive syntax]. For example, set `RUSTUP_LOG=rustup=DEBUG` to receive log lines
  from `rustup` itself with a maximal verbosity of `DEBUG`.

- `RUSTUP_HOME` (default: `~/.rustup` or `%USERPROFILE%/.rustup`). Sets the
  root `rustup` folder, used for storing installed toolchains and
  configuration options.

- `RUSTUP_TOOLCHAIN` (default: none). If set, will [override] the toolchain used
  for all rust tool invocations. A toolchain with this name should be installed,
  or invocations will fail. This can specify custom toolchains, installable
  toolchains, or the absolute path to a toolchain.

- `RUSTUP_DIST_SERVER` (default: `https://static.rust-lang.org`). Sets the root
  URL for downloading static resources related to Rust. You can change this to
  instead use a local mirror, or to test the binaries from the staging
  directory.

- ~~`RUSTUP_DIST_ROOT`~~ *deprecated* (default: `https://static.rust-lang.org/dist`).
  Use `RUSTUP_DIST_SERVER` instead.

- `RUSTUP_UPDATE_ROOT` (default `https://static.rust-lang.org/rustup`). Sets
  the root URL for downloading self-update.

- `RUSTUP_IO_THREADS` *unstable* (defaults to reported cpu count). Sets the
  number of threads to perform close IO in. Set to `1` to force
  single-threaded IO for troubleshooting, or an arbitrary number to override
  automatic detection.

- `RUSTUP_TRACE_DIR` *unstable* (default: no tracing). Enables tracing and
  determines the directory that traces will be written too. Traces are of the
  form PID.trace. Traces can be read by the Catapult project [tracing viewer].

- `RUSTUP_TERM_COLOR` (default: `auto`). Controls whether colored output is used in the terminal.
  Set to `auto` to use colors only in tty streams, to `always` to always enable colors,
  or to `never` to disable colors.

- `RUSTUP_UNPACK_RAM` *unstable* (default free memory or 500MiB if unable to tell, min 210MiB). Caps the amount of
  RAM `rustup` will use for IO tasks while unpacking.

- `RUSTUP_NO_BACKTRACE`. Disables backtraces on non-panic errors even when
  `RUST_BACKTRACE` is set.

- `RUSTUP_PERMIT_COPY_RENAME` *unstable*. When set, allows rustup to fall-back
  to copying files if attempts to `rename` result in cross-device link
  errors. These errors occur on OverlayFS, which is used by [Docker][dc]. This
  feature sacrifices some transactions protections and may be removed at any
  point. Linux only.

[directive syntax]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives
[dc]: https://docs.docker.com/storage/storagedriver/overlayfs-driver/#modifying-files-or-directories
[override]: overrides.md
[tracing viewer]: https://github.com/catapult-project/catapult/blob/master/tracing/README.md
