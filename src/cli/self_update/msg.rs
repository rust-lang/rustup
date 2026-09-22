//! The big installation messages. These are macros because the first
//! argument of format! needs to be a literal.

macro_rules! pre_install_msg_template {
    ($platform_msg:literal) => {
        concat!(
            r"
# Welcome to Rust!

This will download and install the official compiler for the Rust
programming language, and its package manager, Cargo.

{rustup_home_message}

The `cargo`, `rustc`, `rustup` and other commands will be added to
Rustup's bin directory, located at:

    {rustup_bin_home}

This can be modified with `CARGO_HOME`, or overridden in category
home mode with `RUSTUP_BIN_HOME`.

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
Rustup's bin directory (`{rustup_bin_home}`).
"
    };
}

macro_rules! post_install_msg_no_modify_path {
    () => {
        r"# Rust is installed now. Great!

To get started you need Rustup's bin directory (`{rustup_bin_home}`) in your `PATH`
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

macro_rules! pre_uninstall_msg {
    () => {
        r"# Thanks for hacking in Rust!

This will uninstall all Rust toolchains and data, and remove
`{cargo_bin_dir}` from your `PATH` environment variable.

"
    };
}

macro_rules! pre_uninstall_msg_no_modify_path {
    () => {
        r"# Thanks for hacking in Rust!

This will uninstall all Rust toolchains and data.
Your `PATH` environment variable will not be touched.

"
    };
}
