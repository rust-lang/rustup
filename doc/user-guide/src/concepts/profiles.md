# Profiles

`rustup` has the concept of "profiles". They are groups of [components] you
can choose to download while installing a new Rust toolchain. The profiles
available at this time are `minimal`, `default`, and `complete`:

* The **minimal** profile includes as few components as possible to get a
  working compiler (`rustc`, `rust-std`, and `cargo`). It's recommended to use
  this component on Windows systems if you don't use local documentation (the
  large number of files can cause issues with some Antivirus systems), and in
  CI.
* The **default** profile includes all of components in the **minimal**
  profile, and adds `rust-docs`, `rustfmt`, and `clippy`. This profile will be
  used by `rustup` by default, and it's the one recommended for general use.
* The **complete** profile includes all the components available through
  `rustup`. This should never be used, as it includes *every* component ever
  included in the metadata and thus will almost always fail. If you are
  looking for a way to install devtools such as `miri` or IDE integration
  tools (`rust-analyzer`), you should use the `default` profile and
  install the needed additional components manually, either by using `rustup
  component add` or by using `-c` when installing the toolchain.

To change the profile `rustup install` uses by default, you can use the
`rustup set profile` command.
For example, to select the minimal profile you can use:

```console
rustup set profile minimal
```

You can also directly select the profile used when installing a toolchain with:

```console
rustup install --profile <name>
```

It's also possible to choose the default profile when installing `rustup` for
the first time, either interactively by choosing the "Customize installation"
option or programmatically by passing the `--profile=<name>` flag. Profiles
will only affect newly installed toolchains: as usual it will be possible to
install individual components later with: `rustup component add`.

[components]: components.md
