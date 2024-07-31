# Coding standards

Generally we just follow good sensible Rust practices, clippy and so forth.
However there are some practices we've agreed on that are not machine-enforced;
meeting those requirements in a PR will make it easier to merge.

## Atomic commits

We use atomic commits across the repo. Each commit should represent a single unit of change.
You can read more about atomic commits [here](https://www.aleksandrhovhannisyan.com/blog/atomic-git-commits).

## Import grouping

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

## No direct use of process state outside rustup::process

The `rustup::process` module abstracts the global state that is
`std::env::args`, `std::env::vars`, `std::io::std*` and `std::env::current_dir`
permitting threaded tests of the CLI logic; use the relevant methods of the
`rustup::process::Process` type rather than those APIs directly.
Usually, a `process: &Process` variable will be available to you in the current context.
For example, it could be in the form of a parameter of the current function,
or a field of a `Cfg` instance, etc.

## Clippy lints

We do not enforce lint status in the checks done by GitHub Actions, because
clippy is a moving target that can make it hard to merge for little benefit.

We do ask that contributors keep the clippy status clean themselves.

Minimally, run `cargo clippy --all --all-targets --features test -- -D warnings` before
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
