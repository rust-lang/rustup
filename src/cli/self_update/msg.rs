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

In category home mode this can be modified with RUSTUP_BIN_HOME;
otherwise it follows CARGO_HOME.

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

#[cfg(not(windows))]
macro_rules! post_install_msg_unix {
    () => {
        r"# Rust is installed now. Great!

To get started you may need to restart your current shell.
This would reload your `PATH` environment variable to include
Rustup's bin directory ({rustup_bin_home}).

To configure your current shell, run the right command below
(note the leading DOT):
{source_env_lines}"
    };
}

#[cfg(windows)]
macro_rules! post_install_msg_win {
    () => {
        r"# Rust is installed now. Great!


To get started you may need to restart your current shell.
This would reload its `PATH` environment variable to include
Rustup's bin directory ({rustup_bin_home}).
"
    };
}

#[cfg(not(windows))]
macro_rules! post_install_msg_unix_no_modify_path {
    () => {
        r"# Rust is installed now. Great!

To get started you need Rustup's bin directory ({rustup_bin_home}) in your
`PATH` environment variable. This has not been done automatically.

To configure your current shell, run the right command below
(note the leading DOT):
{source_env_lines}"
    };
}

#[cfg(windows)]
macro_rules! post_install_msg_win_no_modify_path {
    () => {
        r"# Rust is installed now. Great!

To get started you need Rustup's bin directory ({rustup_bin_home}) in your
`PATH` environment variable. This has not been done automatically.
"
    };
}

macro_rules! pre_uninstall_msg {
    () => {
        r"# Thanks for hacking in Rust!

This will uninstall all Rust toolchains and data, and remove
`{cargo_home}/bin` from your `PATH` environment variable.

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
