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

## Writing tests

Rustup provides a number of test helpers in the `rustup::test` module
which is conditionally enabled with the `test` feature.

The existing tests under `tests/suite` provide good examples of how to use these
helpers, but you might also find it useful to look at the documentation for
particular APIs in the `rustup::test` module.

For example, for more information regarding end-to-end tests with the `.expect()`
APIs, you can refer to the documentation of the [`Assert`] type.

[`Assert`]: https://github.com/search?q=repo%3Arust-lang%2Frustup+symbol%3A%2F%28%3F-i%29Assert%2F&type=code

## Clippy lints

At the time of writing, rustup's CI pipeline runs clippy on both Windows and
Linux, but contributors to particularly OS-specific code should also make
sure that their clippy checking is done on that particular platform, as
OS-conditional code is a common source of unused imports and other small lints,
which can build up over time.

## Writing platform-specific code

For developers using BSD/Linux/Mac OS, there are Windows VM's suitable for such
development tasks for use with virtualbox and other hypervisors are downloadable
from
[Microsoft](https://developer.microsoft.com/en-us/windows/downloads/virtual-machines/).
Similarly, there are many Linux and Unix operating systems images available for
developers whose usual operating system is Windows. Currently Rustup has no Mac
OS specific code, so there should be no need to worry about Mac VM images.
