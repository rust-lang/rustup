//! Installation and uninstallation messages. These are macros because the
//! first argument of format! needs to be a literal.

macro_rules! pre_install_msg_template {
    ($platform_msg:literal) => {
        concat!(
            r"
# Welcome to Rust!

This will download and install the official compiler for the Rust
programming language, and its package manager, Cargo.

{rustup_home_message}

The Cargo home directory is located at:

    {cargo_home}

This can be modified with the `CARGO_HOME` environment variable.

The `cargo`, `rustc`, `rustup` and other commands will be added to
the bin home directory, located at:

    {cargo_bin_dir}

",
            $platform_msg,
            r#"

You can uninstall at any time with `rustup self uninstall` and
these changes will be reverted.
"#
        )
    };
}

#[cfg(not(windows))]
macro_rules! pre_install_msg_unix {
    () => {
        pre_install_msg_template!(
            "This path will then be added to your `PATH` environment variable by
modifying the profile file{plural} located at:

{rcfiles}"
        )
    };
}

#[cfg(windows)]
macro_rules! pre_install_msg_win {
    () => {
        pre_install_msg_template!(
            r#"This path will then be added to your `PATH` environment variable by
modifying the `PATH` registry key at `HKEY_CURRENT_USER\Environment`."#
        )
    };
}

macro_rules! pre_install_msg_no_modify_path {
    () => {
        pre_install_msg_template!(
            "This path needs to be in your `PATH` environment variable,
but will not be added automatically."
        )
    };
}

macro_rules! post_install_msg {
    () => {
        r"# Rust is installed now. Great!

To get started you may need to restart your current shell.
This would reload your `PATH` environment variable to include
the bin home directory (`{cargo_bin_dir}`).
"
    };
}

macro_rules! post_install_msg_no_modify_path {
    () => {
        r"# Rust is installed now. Great!

To get started you need the bin home directory (`{cargo_bin_dir}`) in your `PATH`
environment variable. This has not been done automatically.
"
    };
}

#[cfg(not(windows))]
macro_rules! post_install_msg_unix {
    () => {
        r"
To configure your current shell, you need to source the
corresponding `env` file under `{env_dir}`.

Consider running the right command for your shell (note the leading DOT):

```
{source_env_lines}```
"
    };
}

// `md` wraps paragraphs at a fixed width and also keeps their source line
// breaks, so home paths go in indented code blocks, and paragraphs containing a
// path are kept on one source line.
macro_rules! pre_uninstall_msg_category_mode {
    () => {
        r"# Thanks for hacking in Rust!

This will uninstall all Rust toolchains and clean these homes:

      config: {config_home}
      state:  {state_home}
      data:   {data_home}
      cache:  {cache_home}
      rustup: {rustup_home}

Rustup and its proxies will be removed from {bin_dirs}.

Other programs there will be kept.

{path_message}"
    };
}

macro_rules! pre_uninstall_msg_legacy {
    () => {
        r"# Thanks for hacking in Rust!

This will uninstall all Rust toolchains and clean these homes:

      rustup: {rustup_home}
      cargo:  {cargo_home}

Rustup and its proxies will be removed from {bin_dirs}.

Other programs there will be kept.

{path_message}"
    };
}
