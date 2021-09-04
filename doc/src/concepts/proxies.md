# Proxies

`rustup` provides a number of wrappers for common Rust tools.
These are called _proxies_ and represent commands which are
provided by the various [components].

The list of proxies is currently static in `rustup` and is as follows:

[components]: components.md

- `rustc` is the compiler for the Rust programming language, provided by the project itself and comes from the `rustc` component.

- `rustdoc` is a tool distributed in the `rustc` component which helps you to generate documentation for Rust projects.

- `cargo` is the Rust package manager which downloads your Rust package’s dependencies, compiles your packages, makes distributable packages, and uploads them to crates.io (the Rust community’s package registry). It comes from the `cargo` component.

- `rust-lldb`, `rust-gdb`, and `rust-gdbgui` are simple wrappers around the `lldb`, `gdb`, and `gdbgui` debuggers respectively. The wrappers enable some pretty-printing of Rust values and add some convenience features to the debuggers by means of their scripting interfaces.

- `rls` is part of the Rust IDE integration tooling. It implements the language-server protocol to permit IDEs and editors such as Visual Studio Code, ViM, or Emacs, access to the semantics of the Rust code you are editing. It comes from the `rls` component.

- `cargo-clippy` and `clippy-driver` are related to the `clippy` linting tool which provides extra checks for common mistakes and stylistic choices and it comes from the `clippy` component.

- `cargo-miri` is an experimental interpreter for Rust's mid-level intermediate representation (MIR) and it comes from the `miri` component.
