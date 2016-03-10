
use clap::*;

pub fn get() -> App<'static, 'static, 'static, 'static, 'static, 'static> {
    App::new("multirust-rs")
        .version("0.0.5")
        .author("Diggory Blake")
        .about("Port of multirust to rust")
        .setting(AppSettings::VersionlessSubcommands)
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Enable verbose output")
        )
        .subcommand(
            SubCommand::with_name("default")
                .about("Set the default toolchain.")
                .after_help(
r"Sets the default toolchain to the one specified. If the toolchain is
not already installed then it is first installed.

If the toolchain is already installed then it is not reinstalled,
though if installing a custom toolchain with --copy-local,
--link-local, or --installer then the toolchain is always
reinstalled.
"
                )
                .arg(Arg::with_name("toolchain").required(true))
                .args(install_args())
                .arg_group(install_group())
        )
        .subcommand(
            SubCommand::with_name("override")
                .about("Set the toolchain override.")
                .after_help(
r"Sets the toolchain that will be used when working at or below the
current directory. If the toolchain is not already installed then it
is first installed.

If the toolchain is already installed then it is not reinstalled,
though if installing a custom toolchain with --copy-local,
--link-local, or --installer then the toolchain is always
reinstalled.

To remove an existing override use `multirust remove-override`.
"
                )
                .arg(Arg::with_name("toolchain").required(true))
                .args(install_args())
                .arg_group(install_group())
        )
        .subcommand(
            SubCommand::with_name("update")
                .about("Install or update a given toolchain.")
                .after_help(
r"With no toolchain specified, the update command updates each of the
stable, beta, and nightly toolchains from the official release
channels, plus any other installed toolchains.
"
                )
                .arg(Arg::with_name("toolchain").required(false))
                .args(install_args())
                .arg_group(install_group())
        )
        .subcommand(
            SubCommand::with_name("show-override")
                .about("Show information about the current override.")
        )
        .subcommand(
            SubCommand::with_name("show-default")
                .about("Show information about the current default.")
        )
        .subcommand(
            SubCommand::with_name("list-overrides")
                .about("List all overrides.")
        )
        .subcommand(
            SubCommand::with_name("list-toolchains")
                .about("List all installed toolchains.")
        )
        .subcommand(
            SubCommand::with_name("remove-override")
                .about("Remove an override.")
                .after_help(
r"Removes the override for the current directory, or the named
override if one is provided.
"
                )
                .arg(Arg::with_name("override").required(false))
        )
        .subcommand(
            SubCommand::with_name("remove-toolchain")
                .about("Uninstall a toolchain.")
                .after_help(
r"Uninstalls an installed toolchain.
"
                )
                .arg(Arg::with_name("toolchain").required(true))
        )
        .subcommand(
            SubCommand::with_name("list-targets")
                .about("List targets available to install")
                .after_help(
r"List the targets available to an installed toolchain.
"
                )
                .arg(Arg::with_name("toolchain").required(true))
        )
        .subcommand(
            SubCommand::with_name("add-target")
                .about("Add additional compilation targets to an existing toolchain")
                .after_help(
r"Adds the standard library for a given platform to an existing
installation.
"
                )
                .arg(Arg::with_name("toolchain").required(true))
                .arg(Arg::with_name("target").required(true))
        )
        .subcommand(
            SubCommand::with_name("remove-target")
                .about("Removes compilation targets from an existing toolchain")
                .after_help(
r"Removes the standard library for a given platform.
"
                )
                .arg(Arg::with_name("toolchain").required(true))
                .arg(Arg::with_name("target").required(true))
        )
        .subcommand(
            SubCommand::with_name("run")
                .setting(AppSettings::TrailingVarArg)
                .about("Run a command.")
                .after_help(
r"Configures an environment to use the given toolchain and then runs
the specified program. The command may be any program, not just
rustc or cargo. This can be used for testing arbitrary toolchains
without setting an override.
"
                )
                .arg(Arg::with_name("toolchain").required(true))
                .arg(Arg::with_name("command").required(true).multiple(true))
        )
        .subcommand(
            SubCommand::with_name("proxy")
                .setting(AppSettings::TrailingVarArg)
                .about("Proxy a command.")
                .after_help(
r"Configures an environment to use the correct toolchain for the
current directory, and then runs the specified program. The command
may be any program, not just rustc or cargo.
"
                )
                .arg(Arg::with_name("command").required(true).multiple(true))
        )
        .subcommand(
            SubCommand::with_name("delete-data")
                .about("Delete all user metadata.")
                .after_help(
r"Rremoves all installed toolchains, overrides, and aliases for the
current user.

Prompts for confirmation, unless disabled.

Does not uninstall multirust.
"
                )
                .arg(Arg::with_name("no-prompt").short("y").help("Disable confirmation prompt."))
        )
        .subcommand(
            SubCommand::with_name("upgrade-data")
                .about("Upgrade the ~/.multirust directory.")
                .after_help(
r"Upgrades the ~/.multirust directory from previous versions.
"
                )
        )
        .subcommand(
            SubCommand::with_name("self")
                .about("Commands for managing multirust itself.")
                .subcommand(
                    SubCommand::with_name("install")
                        .about("Installs multirust.")
                        .after_help(
r"Installs multirust for the current user.
"
                        )
                        .arg(Arg::with_name("add-to-path").short("a").long("add-to-path").help("Modifies .profile or the registry"))
                        .arg(Arg::with_name("move").short("m").long("move").help("Move self instead of copying"))
                )
                .subcommand(
                    SubCommand::with_name("uninstall")
                        .about("Uninstalls multirust.")
                        .arg(Arg::with_name("no-prompt").short("y").help("Disable confirmation prompt."))
                )
                .subcommand(
                    SubCommand::with_name("update")
                        .about("Updates multirust.")
                )
        )
        .subcommand(
            SubCommand::with_name("doc")
                .about("Open the documentation for the current toolchain.")
                .after_help(
r"Opens the documentation for the currently active toolchain with the
default browser.

By default, it opens the documentation index. Use the various flags to
open specific pieces of documentation.
"
                )
                .arg(Arg::with_name("book").long("book").help("The Rust Programming Language book"))
                .arg(Arg::with_name("reference").long("reference").help("Rust language reference"))
                .arg(Arg::with_name("std").long("std").help("Standard library API documentation"))
                .arg(Arg::with_name("nomicon").long("nomicon").help("The Rustonomicon book"))
                .arg(Arg::with_name("error-index").long("error-index").help("Compiler Error Index"))
                .arg_group(ArgGroup::with_name("page").add_all(&["book", "reference", "std", "nomicon", "error-index"]))
        )
        .subcommand(
            SubCommand::with_name("which")
                .about("Report location of the currently active Rust tool.")
                .arg(Arg::with_name("binary").required(true))
        )
}

fn install_args() -> Vec<Arg<'static, 'static, 'static, 'static, 'static, 'static>> {
    vec![
        Arg::with_name("copy-local")
            .long("copy-local")
            .help(r"
             Install by copying a local toolchain. This will
             copy the toolchain from a local directory to a
             directory in multirust's home.
             ")
            .takes_value(true)
            .value_name("toolchain-path")
            .number_of_values(1),
        Arg::with_name("link-local")
            .long("link-local")
            .help(r"
             Install by linking to a local toolchain. This
             will create a soft link to the local toolchain
             in multirust's home.
             ")
            .takes_value(true)
            .value_name("toolchain-path")
            .number_of_values(1),
        Arg::with_name("installer")
            .long("installer")
            .help(r"
             Allows arbitrary builds of rust to be installed
             from a custom-built installer, either from the
             local filesystem or the network. Custom
             installers are neither checksum nor
             signature-verified.

             If multiple installers are specified then they
             are all installed to the same location. This can
             make installing cargo easier since otherwise it
             would need to be combined with rustc into a
             single installer through the rust-packaging
             project.
             ")
            .takes_value(true)
            .value_name("toolchain-path")
            .min_values(1),
    ]
}

fn install_group() -> ArgGroup<'static, 'static> {
    ArgGroup::with_name("toolchain-source")
        .add("copy-local")
        .add("link-local")
        .add("installer")
        .requires("toolchain")
}
