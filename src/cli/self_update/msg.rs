//! The big installation messages. These are macros because the first
//! argument of format! needs to be a literal.

macro_rules! pre_install_msg_template {
    ($platform_msg:literal) => {
        concat!(
            r"
# Welcome to Rust!

This will download and install the official compiler for the Rust
programming language, and its package manager, Cargo.

Rustup metadata and toolchains will be installed into the Rustup
home directory, located at:

    {rustup_home}

This can be modified with the RUSTUP_HOME environment variable.

The Cargo home directory is located at:

    {cargo_home}

This can be modified with the CARGO_HOME environment variable.

The `cargo`, `rustc`, `rustup` and other commands will be added to
Cargo's bin directory, located at:

    {cargo_home_bin}

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
Cargo's bin directory ({cargo_home}/bin).

To configure your current shell, you need to source the
corresponding `env` file under {cargo_home}.

Consider running the right command for your shell (note the leading DOT):
{source_env_lines}"
    };
}

#[cfg(windows)]
macro_rules! post_install_msg_win {
    () => {
        r"# Rust is installed now. Great!


To get started you may need to restart your current shell.
This would reload its `PATH` environment variable to include
Cargo's bin directory ({cargo_home}\\bin).
"
    };
}

#[cfg(not(windows))]
macro_rules! post_install_msg_unix_no_modify_path {
    () => {
        r"# Rust is installed now. Great!

To get started you need Cargo's bin directory ({cargo_home}/bin) in your `PATH`
environment variable. This has not been done automatically.

To configure your current shell, you need to source
the corresponding `env` file under {cargo_home}.

Consider running the right command for your shell (note the leading DOT):
{source_env_lines}"
    };
}

#[cfg(windows)]
macro_rules! post_install_msg_win_no_modify_path {
    () => {
        r"# Rust is installed now. Great!

To get started you need Cargo's bin directory ({cargo_home}\\bin) in your `PATH`
environment variable. This has not been done automatically.
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
