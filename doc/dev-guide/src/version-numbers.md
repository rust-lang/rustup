# Version numbers

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
