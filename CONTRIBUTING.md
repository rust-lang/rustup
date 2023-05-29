# Contributing to rustup

1. Fork it!
2. Create your feature branch: `git checkout -b my-new-feature`
3. Test it: `cargo test --features=test`
4. Lint it: `cargo +beta clippy --all --all-targets -- -D warnings`
> We use `cargo clippy` to ensure high-quality code and to enforce a set of best practices for Rust programming. However, not all lints provided by `cargo clippy` are relevant or applicable to our project.
> We may choose to ignore some lints if they are unstable, experimental, or specific to our project.
> If you are unsure about a lint, please ask us in the [rustup Discord channel](https://discord.com/channels/442252698964721669/463480252723888159).
5. Commit your changes: `git commit -am 'Add some feature'`
6. Push to the branch: `git push origin my-new-feature`
7. Submit a pull request :D

For developing on `rustup` itself, you may want to install into a temporary
directory, with a series of commands similar to this:

```bash
cargo build
mkdir home
RUSTUP_HOME=home CARGO_HOME=home target/debug/rustup-init --no-modify-path -y
```

You can then try out `rustup` with your changes by running `home/bin/rustup`, without
affecting any existing installation. Remember to keep those two environment variables
set when running your compiled `rustup-init` or the toolchains it installs, but _unset_
when rebuilding `rustup` itself.

If you wish to install your new build to try out longer term in your home directory
then you can run `cargo dev-install` which is an alias in `.cargo/config` which
runs `cargo run -- --no-modify-path -y` to install your build into your homedir.

We use `rustfmt` to keep our codebase consistently formatted. Please ensure that
you have correctly formatted your code (most editors will do this automatically
when saving) or it may not pass the CI tests.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as in the README, without any
additional terms or conditions.

## Non-machine-enforced coding standards

These are requirements we have that we have not yet lifted to the level of
automatic enforcement.

### Import grouping

In each file the imports should be grouped into at most 4 groups in the
following order:

1. stdlib
2. non-repository local crates
3. repository local other crates
4. this crate

Separate each group with a blank line, and rustfmt will sort into a canonical
order. Any file that is not grouped like this can be rearranged whenever the
file is touched - we're not precious about having it done in a separate commit,
though that is helpful.

### No direct use of process state outside rustup::currentprocess

The `rustup::currentprocess` module abstracts the global state that is
`std::env::args`, `std::env::vars`, `std::io::std*`, `std::process::id`,
`std::env::current_dir` and `std::process::exit` permitting threaded tests of
the CLI logic; use `process()` rather than those APIs directly.

### Clippy lints

We do not enforce lint status in the checks done by GitHub Actions, because
clippy is a moving target that can make it hard to merge for little benefit.

We do ask that contributors keep the clippy status clean themselves.

Minimally, run `cargo +beta clippy --all --all-targets -- -D warnings` before
submitting code.

If possible, adding `--all-features` to the command is useful, but will require
additional dependencies like `libcurl-dev`.

Regular contributors or contributors to particularly OS-specific code should
also make sure that their clippy checking is done on at least Linux and Windows,
as OS-conditional code is a common source of unused imports and other small
lints, which can build up over time.

For developers using BSD/Linux/Mac OS, there are Windows VM's suitable for such
development tasks for use with virtualbox and other hypervisors are downloadable
from
[Microsoft](https://developer.microsoft.com/en-us/windows/downloads/virtual-machines/).
Similarly, there are many Linux and Unix operating systems images available for
developers whose usual operating system is Windows. Currently Rustup has no Mac
OS specific code, so there should be no need to worry about Mac VM images.

Clippy is also run in GitHub Actions, in the `General Checks / Checks` build
task, but not currently run per-platform, which means there is no way to find
out the status of clippy per platform without running it on that platform as a
developer.

### import rustup-macros::{integration,unit}_test into test modules

These test helpers add pre-and-post logic to tests to enable the use of tracing
inside tests, which can be helpful for tracking down behaviours in larger tests.

## Version numbers

If you ever see a released version of rustup which has `::` in its version string
then something went wrong with the CI and that needs to be addressed.

We use `git-testament` to construct our version strings. This records, as a
struct, details of the git commit, tag description, and also an indication
of modifications to the working tree present when the binary was compiled.

During normal development you may get information from invoking `rustup --version`
which looks like `rustup-init 1.18.3+15 (a54051502 2019-05-26)` or even
`rustup-init 1.18.3+15 (a54051502 2019-05-26) dirty 1 modification`.

The first part is always the binary name as per `clap`'s normal operation. The
version number is a combination of the most recent tag in the git repo, and the
number of commits since that tag. The parenthesised information is, naturally,
the SHA of the most recent commit and the date of that commit. If the indication
of a dirty tree is present, the number of changes is indicated. This combines
adds, deletes, modifies, and unknown entries.

You can request further information of a `rustup` binary with the
`rustup dump-testament` hidden command. It produces output of the form:

```shell
$ rustup dump-testament
Rustup version renders as: 1.18.3+15 (a54051502 2019-05-26) dirty 1 modification
Current crate version: 1.18.3
Built from branch: kinnison/version-strings
Commit info: 1.18.3+15 (a54051502 2019-05-26)
Modified: CONTRIBUTING.md
```

This can be handy when you are testing development versions on your PC
and cannot remember exactly which version you had installed, or if you have given
a development copy (or instruction to build such) to a user, and wish to have them
confirm _exactly_ what they are using.

Finally, we tell `git-testament` that we trust the `stable` branch to carry
releases. If the build is being performed when not on the `stable` branch, and
the tag and `CARGO_PKG_VERSION` differ, then the short version string will include
both, in the form `rustup-init 1.18.3 :: 1.18.2+99 (a54051502 2019-05-26)` which
indicates the crate version before the rest of the commit.
On the other hand, if the build was on the `stable` branch then regardless
of the tag information, providing the commit was clean, the version is
always replaced by the crate version. The `dump-testament` hidden command can
reveal the truth however.

## Making a release

Before making a release, ensure that `rustup-init.sh` is behaving correctly,
and that you're satisfied that nothing in the ecosystem is breaking because
of the update. A useful set of things to check includes verifying that
real-world toolchains install okay, and that `rls-vscode` isn't broken by
the release. While it's not our responsibility if they depend on non-stable
APIs, we should behave well if we can.

Producing the final release artifacts is a bit involved because of the way
Rustup is distributed. The steps for a release are:

1. Update `Cargo.toml` and `download/Cargo.toml`to have the same new version
   (optionally make a commit)
2. Run `cargo build` and review `Cargo.lock` changes
   if all looks well, make a commit
3. Update `rustup-init.sh` version to match the commit
   details for `Cargo.lock`
4. Push this to the `stable` branch (git push origin HEAD:stable)
5. While you wait for green CI, double-check the `rustup-init.sh` functionality
   and `rustup-init` just in case.
6. Ensure all of CI is green on the `stable` branch.
   Once it is, check through a representative proportion of the builds looking
   for the reported version statements to ensure that we definitely built something
   cleanly which reports as the right version number when run `--version`.
7. Ping someone in the release team to perform the actual release.
   They can find instructions in `ci/sync-dist.py`
   Note: Some manual testing occurs here, so hopefully they'll catch
   anything egregious in which case abort the change and roll back.
8. Once the official release has happened, prepare and push a tag
   of that commit, and also push the content to master
   - `git tag -as $VER_NUM -m $VER_NUM` (optionally without -s if not GPG
     signing the tag)
   - `git push origin HEAD:master`
   - `git push origin $VER_NUM`

## Developer tips and tricks

### `RUSTUP_FORCE_ARG0`

The environment variable `RUSTUP_FORCE_ARG0` can be used to get rustup to think
it's a particular binary, rather than e.g. copying it, symlinking it or other
tricks with exec. This is handy when testing particular code paths from cargo
run.

```shell
RUSTUP_FORCE_ARG0=rustup cargo run -- uninstall nightly
```

### `RUSTUP_BACKTRACK_LIMIT`

If it's necessary to alter the backtracking limit from the default of half
a release cycle for some reason, you can set the `RUSTUP_BACKTRACK_LIMIT`
environment variable. If this is unparseable as an `i32` or if it's absent
then the default of 21 days (half a cycle) is used. If it parses and is less
than 1, it is clamped to 1 at minimum.

This is not meant for use by users, but can be suggested in diagnosing an issue
should one arise with the backtrack limits.

### `RUSTUP_MAX_RETRIES`

When downloading a file, rustup will retry the download a number of times. The
default is 3 times, but if this variable is set to a valid usize then it is the
max retry count. A value of `0` means no retries, thus the default of `3` will
mean a download is tried a total of four times before failing out.

### `RUSTUP_BACKTRACE`

By default while running tests, we unset some environment variables that will
break our testing (like `RUSTUP_TOOLCHAIN`, `SHELL`, `ZDOTDIR`, `RUST_BACKTRACE`).
But if you want to debug locally, you may need backtrace. `RUSTUP_BACKTRACE`
is used like `RUST_BACKTRACE` to enable backtraces of failed tests.

**NOTE**: This is a backtrace for the test, not for any subprocess invocation of
rustup process running in the test

```bash
$ RUSTUP_BACKTRACE=1 cargo test --release --test cli-v1 -- remove_toolchain_then_add_again
    Finished release [optimized] target(s) in 0.38s
     Running target\release\deps\cli_v1-1f29f824792f6dc1.exe

running 1 test
test remove_toolchain_then_add_again ... FAILED

failures:

---- remove_toolchain_then_add_again stdout ----
thread 'remove_toolchain_then_add_again' panicked at 'called `Result::unwrap()` on an `Err` value: Os { code: 1142, kind: Other, message: "An attempt was made to create more links on a file than the file system supports." }', src\libcore\result.rs:999:5
stack backtrace:
   0: backtrace::backtrace::trace_unsynchronized
             at C:\Users\appveyor\.cargo\registry\src\github.com-1ecc6299db9ec823\backtrace-0.3.29\src\backtrace\mod.rs:66
   1: std::sys_common::backtrace::_print
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libstd\sys_common\backtrace.rs:47
   2: std::sys_common::backtrace::print
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libstd\sys_common\backtrace.rs:36
   3: std::panicking::default_hook::{{closure}}
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libstd\panicking.rs:198
   4: std::panicking::default_hook
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libstd\panicking.rs:209
   5: std::panicking::rust_panic_with_hook
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libstd\panicking.rs:475
   6: std::panicking::continue_panic_fmt
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libstd\panicking.rs:382
   7: std::panicking::rust_begin_panic
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libstd\panicking.rs:309
   8: core::panicking::panic_fmt
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libcore\panicking.rs:85
   9: core::result::unwrap_failed
  10: cli_v1::mock::clitools::test
  11: alloc::boxed::{{impl}}::call_once<(),FnOnce<()>>
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\src\liballoc\boxed.rs:746
  12: panic_unwind::__rust_maybe_catch_panic
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libpanic_unwind\lib.rs:82
  13: std::panicking::try
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\src\libstd\panicking.rs:273
  14: std::panic::catch_unwind
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\src\libstd\panic.rs:388
  15: test::run_test::run_test_inner::{{closure}}
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libtest\lib.rs:1466
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.


failures:
    remove_toolchain_then_add_again

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 26 filtered out

error: test failed, to rerun pass '--test cli-v1'
```

## Tracing

The feature "otel" can be used when building rustup to turn on Opentelemetry
tracing with an OLTP GRPC exporter. This requires protoc installed, which can be
downloaded from GitHub or installed via  package manager.

The normal [OTLP environment
variables](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/protocol/exporter.md)
can be used to customise its behaviour, but often the simplest thing is to just
run a Jaeger docker container on the same host:

```sh
docker run -d --name jaeger   -e COLLECTOR_ZIPKIN_HOST_PORT=:9411   -e COLLECTOR_OTLP_ENABLED=true   -p 6831:6831/udp   -p 6832:6832/udp   -p 5778:5778   -p 16686:16686   -p 4317:4317   -p 4318:4318   -p 14250:14250   -p 14268:14268   -p 14269:14269   -p 9411:9411   jaegertracing/all-in-one:latest
```

Then build rustup-init with tracing:

```sh
cargo build --features=otel
```

Run the operation you want to analyze:

```sh
RUSTUP_FORCE_ARG0="rustup" ./target/debug/rustup-init show
```

And [look in Jaeger for a trace](http://localhost:16686/search?service=rustup).

### Tracing and tests

The custom macro `rustup_macros::test` adds a prelude and suffix to each test to
ensure that there is a tracing context setup, that the test function is a span,
and that the spans from the test are flushed. Build with features=otel,test to
use this feature.

### Adding instrumentation

The `otel` feature uses conditional compilation to only add function instrument
when enabled. Instrumenting a currently uninstrumented function is mostly simply
done like so:

```rust
#[cfg_attr(feature = "otel", tracing::instrument(err, skip_all))]
```

`skip_all` is not required, but some core structs don't implement Debug yet, and
others have a lot of output in Debug : tracing adds some overheads, so keeping
spans lightweight can help avoid frequency bias in the results - where
parameters with large debug in frequently called functions show up as much
slower than they are.

Some good general heuristics:

- Do instrument slow blocking functions
- Do instrument functions with many callers or that call many different things,
  as these tend to help figure the puzzle of what-is-happening
- Default to not instrumenting thin shim functions (or at least, only instrument
  them temporarily while figuring out the shape of a problem)
- Be way of debug build timing - release optimisations make a huge difference,
  though debug is a lot faster to iterate on. If something isn't a problem in
  release don't pay it too much heed in debug.

### Caveats

Cross-thread propogation isn't connected yet. This will cause instrumentation in
a thread to make a new root span until it is fixed. If any Tokio runtime-related
code gets added in those threads this will also cause a panic. We have a couple
of threadpools in use today; if you need to instrument within that context, use
a thunk to propogate the tokio runtime into those threads.