use std::borrow::Cow;
use std::env::consts::EXE_SUFFIX;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::str::FromStr;

use anyhow::{Context, Error, Result, anyhow};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum, builder::PossibleValue};
use clap_complete::Shell;
use itertools::Itertools;
use tracing::{info, trace, warn};
use tracing_subscriber::{EnvFilter, Registry, reload::Handle};

use crate::dist::AutoInstallMode;
use crate::{
    cli::{
        common::{self, PackageUpdate, update_console_filter},
        errors::CLIError,
        help::*,
        self_update::{self, RustupUpdateAvailable, SelfUpdateMode, check_rustup_update},
        topical_doc,
    },
    command,
    config::{ActiveReason, Cfg},
    dist::{
        PartialToolchainDesc, Profile, TargetTriple,
        manifest::{Component, ComponentStatus},
    },
    errors::RustupError,
    install::{InstallMethod, UpdateStatus},
    process::{
        Process,
        terminalsource::{self, ColorableTerminal},
    },
    toolchain::{
        CustomToolchainName, DistributableToolchain, LocalToolchainName,
        MaybeResolvableToolchainName, ResolvableLocalToolchainName, ResolvableToolchainName,
        Toolchain, ToolchainName,
    },
    utils::{self, ExitCode},
};

const TOOLCHAIN_OVERRIDE_ERROR: &str = "To override the toolchain using the 'rustup +toolchain' syntax, \
                        make sure to prefix the toolchain override with a '+'";

fn handle_epipe(res: Result<utils::ExitCode>) -> Result<utils::ExitCode> {
    match res {
        Err(e) => {
            let root = e.root_cause();
            if let Some(io_err) = root.downcast_ref::<std::io::Error>() {
                if io_err.kind() == std::io::ErrorKind::BrokenPipe {
                    return Ok(utils::ExitCode(0));
                }
            }
            Err(e)
        }
        res => res,
    }
}

/// The Rust toolchain installer
#[derive(Debug, Parser)]
#[command(
    name = "rustup",
    bin_name = "rustup[EXE]",
    version = common::version(),
    before_help = format!("rustup {}", common::version()),
    after_help = RUSTUP_HELP,
)]
struct Rustup {
    /// Set log level to 'DEBUG' if 'RUSTUP_LOG' is unset
    #[arg(short, long, conflicts_with = "quiet")]
    verbose: bool,

    /// Disable progress output, set log level to 'WARN' if 'RUSTUP_LOG' is unset
    #[arg(short, long, conflicts_with = "verbose")]
    quiet: bool,

    /// Release channel (e.g. +stable) or custom toolchain to set override
    #[arg(
        name = "+toolchain",
        value_parser = plus_toolchain_value_parser,
    )]
    plus_toolchain: Option<ResolvableToolchainName>,

    #[command(subcommand)]
    subcmd: Option<RustupSubcmd>,
}

fn plus_toolchain_value_parser(s: &str) -> clap::error::Result<ResolvableToolchainName> {
    use clap::{Error, error::ErrorKind};
    if let Some(stripped) = s.strip_prefix('+') {
        ResolvableToolchainName::try_from(stripped)
            .map_err(|e| Error::raw(ErrorKind::InvalidValue, e))
    } else {
        Err(Error::raw(
            ErrorKind::InvalidSubcommand,
            format!(
                "\"{s}\" is not a valid subcommand, so it was interpreted as a toolchain name, but it is also invalid. {TOOLCHAIN_OVERRIDE_ERROR}"
            ),
        ))
    }
}

#[derive(Debug, Subcommand)]
#[command(name = "rustup", bin_name = "rustup[EXE]")]
enum RustupSubcmd {
    /// Install or update the given toolchains, or by default the active toolchain
    #[command(hide = true, after_help = INSTALL_HELP)]
    Install {
        #[command(flatten)]
        opts: UpdateOpts,
    },

    /// Uninstall the given toolchains
    #[command(hide = true)]
    Uninstall {
        #[command(flatten)]
        opts: UninstallOpts,
    },

    /// Dump information about the build
    #[command(hide = true)]
    DumpTestament,

    /// Install, uninstall, or list toolchains
    Toolchain {
        #[command(subcommand)]
        subcmd: ToolchainSubcmd,
    },

    /// Set the default toolchain
    #[command(after_help = DEFAULT_HELP)]
    Default {
        #[arg(help = MAYBE_RESOLVABLE_TOOLCHAIN_ARG_HELP)]
        toolchain: Option<MaybeResolvableToolchainName>,

        /// Install toolchains that require an emulator. See https://github.com/rust-lang/rustup/wiki/Non-host-toolchains
        #[arg(long)]
        force_non_host: bool,
    },

    /// Show the active and installed toolchains or profiles
    #[command(after_help = SHOW_HELP)]
    Show {
        /// Enable verbose output with rustc information for all installed toolchains
        #[arg(short, long)]
        verbose: bool,

        #[command(subcommand)]
        subcmd: Option<ShowSubcmd>,
    },

    /// Update Rust toolchains and rustup
    #[command(
        after_help = UPDATE_HELP,
        aliases = ["upgrade", "up"],
    )]
    Update {
        /// Toolchain name, such as 'stable', 'nightly', or '1.8.0'. For more information see `rustup help toolchain`
        #[arg(num_args = 1.., value_parser = update_toolchain_value_parser)]
        toolchain: Vec<PartialToolchainDesc>,

        /// Don't perform self update when running the `rustup update` command
        #[arg(long)]
        no_self_update: bool,

        /// Force an update, even if some components are missing
        #[arg(long)]
        force: bool,

        /// Install toolchains that require an emulator. See https://github.com/rust-lang/rustup/wiki/Non-host-toolchains
        #[arg(long)]
        force_non_host: bool,
    },

    /// Check for updates to Rust toolchains and rustup
    Check {
        #[command(flatten)]
        opts: CheckOpts,
    },

    /// Modify a toolchain's supported targets
    Target {
        #[command(subcommand)]
        subcmd: TargetSubcmd,
    },

    /// Modify a toolchain's installed components
    Component {
        #[command(subcommand)]
        subcmd: ComponentSubcmd,
    },

    /// Modify toolchain overrides for directories
    Override {
        #[command(subcommand)]
        subcmd: OverrideSubcmd,
    },

    /// Run a command with an environment configured for a given toolchain
    #[command(after_help = RUN_HELP, trailing_var_arg = true)]
    Run {
        #[arg(help = RESOLVABLE_LOCAL_TOOLCHAIN_ARG_HELP)]
        toolchain: ResolvableLocalToolchainName,

        #[arg(required = true, num_args = 1..)]
        command: Vec<String>,

        /// Install the requested toolchain if needed
        #[arg(long)]
        install: bool,
    },

    /// Display which binary will be run for a given command
    Which {
        command: String,

        #[arg(long, help = RESOLVABLE_TOOLCHAIN_ARG_HELP)]
        toolchain: Option<ResolvableToolchainName>,
    },

    /// Open the documentation for the current toolchain
    #[command(
        alias = "docs",
        after_help = DOC_HELP,
    )]
    Doc {
        /// Only print the path to the documentation
        #[arg(long)]
        path: bool,

        #[arg(long, help = OFFICIAL_TOOLCHAIN_ARG_HELP)]
        toolchain: Option<PartialToolchainDesc>,

        #[arg(help = TOPIC_ARG_HELP)]
        topic: Option<String>,

        #[command(flatten)]
        page: DocPage,
    },

    /// View the man page for a given command
    #[cfg(not(windows))]
    Man {
        command: String,

        #[arg(long, help = OFFICIAL_TOOLCHAIN_ARG_HELP)]
        toolchain: Option<PartialToolchainDesc>,
    },

    /// Modify the rustup installation
    Self_ {
        #[command(subcommand)]
        subcmd: SelfSubcmd,
    },

    /// Alter rustup settings
    Set {
        #[command(subcommand)]
        subcmd: SetSubcmd,
    },

    /// Generate tab-completion scripts for your shell
    #[command(after_help = COMPLETIONS_HELP, arg_required_else_help = true)]
    Completions {
        shell: Shell,

        #[arg(default_value = "rustup")]
        command: CompletionCommand,
    },
}

fn update_toolchain_value_parser(s: &str) -> Result<PartialToolchainDesc> {
    PartialToolchainDesc::from_str(s).inspect_err(|_| {
        if s == "self" {
            info!("if you meant to update rustup itself, use `rustup self update`");
        }
    })
}

#[derive(Debug, Subcommand)]
enum ShowSubcmd {
    /// Show the active toolchain
    #[command(after_help = SHOW_ACTIVE_TOOLCHAIN_HELP)]
    ActiveToolchain {
        /// Enable verbose output with rustc information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Display the computed value of RUSTUP_HOME
    Home,

    /// Show the default profile used for the `rustup install` command
    Profile,
}

#[derive(Debug, Subcommand)]
#[command(
    arg_required_else_help = true,
    subcommand_required = true,
    after_help = TOOLCHAIN_HELP,
)]
enum ToolchainSubcmd {
    /// List installed toolchains
    List {
        /// Enable verbose output with toolchain information
        #[arg(short, long)]
        verbose: bool,

        /// Force the output to be a single column
        #[arg(short, long, conflicts_with = "verbose")]
        quiet: bool,
    },

    /// Install or update the given toolchains, or by default the active toolchain
    #[command(aliases = ["update", "add"] )]
    Install {
        #[command(flatten)]
        opts: UpdateOpts,
    },

    /// Uninstall the given toolchains
    #[command(aliases = ["remove", "rm", "delete", "del"])]
    Uninstall {
        #[command(flatten)]
        opts: UninstallOpts,
    },

    /// Create a custom toolchain by symlinking to a directory
    #[command(after_help = TOOLCHAIN_LINK_HELP)]
    Link {
        /// Custom toolchain name
        toolchain: CustomToolchainName,

        /// Path to the directory
        path: PathBuf,
    },
}

#[derive(Debug, Default, Args)]
struct CheckOpts {
    /// Don't check for self update when running the `rustup check` command
    #[arg(long)]
    no_self_update: bool,
}

#[derive(Debug, Default, Args)]
struct UpdateOpts {
    #[arg(
        help = OFFICIAL_TOOLCHAIN_ARG_HELP,
        num_args = 1..,
    )]
    toolchain: Vec<PartialToolchainDesc>,

    #[arg(long, value_enum)]
    profile: Option<Profile>,

    /// Comma-separated list of components to be added on installation
    #[arg(short, long, value_delimiter = ',')]
    component: Vec<String>,

    /// Comma-separated list of targets to be added on installation
    #[arg(short, long, value_delimiter = ',')]
    target: Vec<String>,

    /// Don't perform self update when running the `rustup toolchain install` command
    #[arg(long)]
    no_self_update: bool,

    /// Force an update, even if some components are missing
    #[arg(long)]
    force: bool,

    /// Allow rustup to downgrade the toolchain to satisfy your component choice
    #[arg(long)]
    allow_downgrade: bool,

    /// Install toolchains that require an emulator. See https://github.com/rust-lang/rustup/wiki/Non-host-toolchains
    #[arg(long)]
    force_non_host: bool,
}

#[derive(Debug, Default, Args)]
struct UninstallOpts {
    #[arg(
        help = RESOLVABLE_TOOLCHAIN_ARG_HELP,
        required = true,
        num_args = 1..,
    )]
    toolchain: Vec<ResolvableToolchainName>,
}

#[derive(Debug, Subcommand)]
#[command(arg_required_else_help = true, subcommand_required = true)]
enum TargetSubcmd {
    /// List installed and available targets
    List {
        #[arg(
            long,
            help = OFFICIAL_TOOLCHAIN_ARG_HELP,
        )]
        toolchain: Option<PartialToolchainDesc>,

        /// List only installed targets
        #[arg(long)]
        installed: bool,

        /// Force the output to be a single column
        #[arg(long, short)]
        quiet: bool,
    },

    /// Add a target to a Rust toolchain
    #[command(alias = "install")]
    Add {
        /// List of targets to install; "all" installs all available targets
        #[arg(required = true, num_args = 1..)]
        target: Vec<String>,

        #[arg(long, help = OFFICIAL_TOOLCHAIN_ARG_HELP)]
        toolchain: Option<PartialToolchainDesc>,
    },

    /// Remove a target from a Rust toolchain
    #[command(aliases = ["uninstall", "rm", "delete", "del"])]
    Remove {
        /// List of targets to uninstall
        #[arg(required = true, num_args = 1..)]
        target: Vec<String>,

        #[arg(long, help = OFFICIAL_TOOLCHAIN_ARG_HELP)]
        toolchain: Option<PartialToolchainDesc>,
    },
}

#[derive(Debug, Subcommand)]
#[command(arg_required_else_help = true, subcommand_required = true)]
enum ComponentSubcmd {
    /// List installed and available components
    List {
        #[arg(long, help = OFFICIAL_TOOLCHAIN_ARG_HELP)]
        toolchain: Option<PartialToolchainDesc>,

        /// List only installed components
        #[arg(long)]
        installed: bool,

        /// Force the output to be a single column
        #[arg(long, short)]
        quiet: bool,
    },

    /// Add a component to a Rust toolchain
    Add {
        #[arg(required = true, num_args = 1..)]
        component: Vec<String>,

        #[arg(long, help = OFFICIAL_TOOLCHAIN_ARG_HELP)]
        toolchain: Option<PartialToolchainDesc>,

        #[arg(long)]
        target: Option<String>,
    },

    /// Remove a component from a Rust toolchain
    #[command(aliases = ["uninstall", "rm", "delete", "del"])]
    Remove {
        #[arg(required = true, num_args = 1..)]
        component: Vec<String>,

        #[arg(long, help = OFFICIAL_TOOLCHAIN_ARG_HELP)]
        toolchain: Option<PartialToolchainDesc>,

        #[arg(long)]
        target: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
#[command(
    after_help = OVERRIDE_HELP,
    arg_required_else_help = true,
    subcommand_required = true,
)]
enum OverrideSubcmd {
    /// List directory toolchain overrides
    List,

    /// Set the override toolchain for a directory
    #[command(alias = "add")]
    Set {
        #[arg(help = RESOLVABLE_TOOLCHAIN_ARG_HELP)]
        toolchain: ResolvableToolchainName,

        /// Path to the directory
        #[arg(long)]
        path: Option<PathBuf>,
    },

    /// Remove the override toolchain for a directory
    #[command(aliases = ["remove", "rm", "delete", "del"], after_help = OVERRIDE_UNSET_HELP)]
    Unset {
        /// Path to the directory
        #[arg(long)]
        path: Option<PathBuf>,

        /// Remove override toolchain for all nonexistent directories
        #[arg(long)]
        nonexistent: bool,
    },
}

#[derive(Debug, Subcommand)]
#[command(
    name = "self",
    arg_required_else_help = true,
    subcommand_required = true
)]
enum SelfSubcmd {
    /// Download and install updates to rustup
    Update,

    /// Uninstall rustup
    Uninstall {
        #[arg(short = 'y')]
        no_prompt: bool,
    },

    /// Upgrade the internal data format
    UpgradeData,
}

#[derive(Debug, Subcommand)]
#[command(arg_required_else_help = true, subcommand_required = true)]
enum SetSubcmd {
    /// The triple used to identify toolchains when not specified
    DefaultHost { host_triple: String },

    /// The default components installed with a toolchain
    Profile {
        #[arg(value_enum, default_value_t)]
        profile_name: Profile,
    },

    /// The rustup auto self update mode
    AutoSelfUpdate {
        #[arg(value_enum, default_value_t)]
        auto_self_update_mode: SelfUpdateMode,
    },

    /// The auto toolchain install mode
    AutoInstall {
        #[arg(value_enum, default_value_t)]
        auto_install_mode: AutoInstallMode,
    },
}

#[tracing::instrument(level = "trace", fields(args = format!("{:?}", process.args_os().collect::<Vec<_>>())), skip(process, console_filter))]
pub async fn main(
    current_dir: PathBuf,
    process: &Process,
    console_filter: Handle<EnvFilter, Registry>,
) -> Result<utils::ExitCode> {
    self_update::cleanup_self_updater(process)?;

    use clap::error::ErrorKind::*;
    let matches = match Rustup::try_parse_from(process.args_os()) {
        Ok(matches) => matches,
        Err(err) if err.kind() == DisplayHelp => {
            write!(process.stdout().lock(), "{err}")?;
            return Ok(utils::ExitCode(0));
        }
        Err(err) if err.kind() == DisplayVersion => {
            write!(process.stdout().lock(), "{err}")?;
            info!("This is the version for the rustup toolchain manager, not the rustc compiler.");
            let mut cfg = common::set_globals(current_dir, true, process)?;
            match cfg.active_rustc_version().await {
                Ok(Some(version)) => info!("The currently active `rustc` version is `{version}`"),
                Ok(None) => info!("No `rustc` is currently active"),
                Err(err) => trace!("Failed to display the current `rustc` version: {err}"),
            }
            return Ok(utils::ExitCode(0));
        }

        Err(err) => {
            if [
                InvalidSubcommand,
                UnknownArgument,
                DisplayHelpOnMissingArgumentOrSubcommand,
            ]
            .contains(&err.kind())
            {
                write!(process.stdout().lock(), "{err}")?;
            } else {
                write!(process.stderr().lock(), "{err}")?;
            }
            return Ok(utils::ExitCode(1));
        }
    };

    update_console_filter(process, &console_filter, matches.quiet, matches.verbose);

    let cfg = &mut common::set_globals(current_dir, matches.quiet, process)?;

    if let Some(t) = &matches.plus_toolchain {
        cfg.set_toolchain_override(t);
    }

    let Some(subcmd) = matches.subcmd else {
        let help = Rustup::command().render_long_help();
        writeln!(process.stderr().lock(), "{help}")?;
        return Ok(utils::ExitCode(1));
    };

    match subcmd {
        RustupSubcmd::DumpTestament => common::dump_testament(process),
        RustupSubcmd::Install { opts } => update(cfg, opts, true).await,
        RustupSubcmd::Uninstall { opts } => toolchain_remove(cfg, opts).await,
        RustupSubcmd::Show { verbose, subcmd } => handle_epipe(match subcmd {
            None => show(cfg, verbose).await,
            Some(ShowSubcmd::ActiveToolchain { verbose }) => {
                show_active_toolchain(cfg, verbose).await
            }
            Some(ShowSubcmd::Home) => show_rustup_home(cfg),
            Some(ShowSubcmd::Profile) => {
                writeln!(process.stdout().lock(), "{}", cfg.get_profile()?)?;
                Ok(ExitCode(0))
            }
        }),
        RustupSubcmd::Update {
            toolchain,
            no_self_update,
            force,
            force_non_host,
        } => {
            update(
                cfg,
                UpdateOpts {
                    toolchain,
                    no_self_update,
                    force,
                    force_non_host,
                    ..UpdateOpts::default()
                },
                false,
            )
            .await
        }
        RustupSubcmd::Toolchain { subcmd } => match subcmd {
            ToolchainSubcmd::Install { opts } => update(cfg, opts, true).await,
            ToolchainSubcmd::List { verbose, quiet } => {
                handle_epipe(common::list_toolchains(cfg, verbose, quiet).await)
            }
            ToolchainSubcmd::Link { toolchain, path } => {
                toolchain_link(cfg, &toolchain, &path).await
            }
            ToolchainSubcmd::Uninstall { opts } => toolchain_remove(cfg, opts).await,
        },
        RustupSubcmd::Check { opts } => check_updates(cfg, opts).await,
        RustupSubcmd::Default {
            toolchain,
            force_non_host,
        } => default_(cfg, toolchain, force_non_host).await,
        RustupSubcmd::Target { subcmd } => match subcmd {
            TargetSubcmd::List {
                toolchain,
                installed,
                quiet,
            } => handle_epipe(target_list(cfg, toolchain, installed, quiet).await),
            TargetSubcmd::Add { target, toolchain } => target_add(cfg, target, toolchain).await,
            TargetSubcmd::Remove { target, toolchain } => {
                target_remove(cfg, target, toolchain).await
            }
        },
        RustupSubcmd::Component { subcmd } => match subcmd {
            ComponentSubcmd::List {
                toolchain,
                installed,
                quiet,
            } => handle_epipe(component_list(cfg, toolchain, installed, quiet).await),
            ComponentSubcmd::Add {
                component,
                toolchain,
                target,
            } => component_add(cfg, component, toolchain, target).await,
            ComponentSubcmd::Remove {
                component,
                toolchain,
                target,
            } => component_remove(cfg, component, toolchain, target).await,
        },
        RustupSubcmd::Override { subcmd } => match subcmd {
            OverrideSubcmd::List => handle_epipe(common::list_overrides(cfg)),
            OverrideSubcmd::Set { toolchain, path } => {
                override_add(cfg, toolchain, path.as_deref()).await
            }
            OverrideSubcmd::Unset { path, nonexistent } => {
                override_remove(cfg, path.as_deref(), nonexistent)
            }
        },
        RustupSubcmd::Run {
            toolchain,
            command,
            install,
        } => run(cfg, toolchain, command, install)
            .await
            .map(ExitCode::from),
        RustupSubcmd::Which { command, toolchain } => which(cfg, &command, toolchain).await,
        RustupSubcmd::Doc {
            path,
            toolchain,
            topic,
            page,
        } => doc(cfg, path, toolchain, topic.as_deref(), &page).await,
        #[cfg(not(windows))]
        RustupSubcmd::Man { command, toolchain } => man(cfg, &command, toolchain).await,
        RustupSubcmd::Self_ { subcmd } => match subcmd {
            SelfSubcmd::Update => self_update::update(cfg).await,
            SelfSubcmd::Uninstall { no_prompt } => self_update::uninstall(no_prompt, process),
            SelfSubcmd::UpgradeData => cfg.upgrade_data().map(|_| ExitCode(0)),
        },
        RustupSubcmd::Set { subcmd } => match subcmd {
            SetSubcmd::DefaultHost { host_triple } => cfg
                .set_default_host_triple(host_triple)
                .map(|_| utils::ExitCode(0)),
            SetSubcmd::Profile { profile_name } => {
                cfg.set_profile(profile_name).map(|_| utils::ExitCode(0))
            }
            SetSubcmd::AutoSelfUpdate {
                auto_self_update_mode,
            } => set_auto_self_update(cfg, auto_self_update_mode),
            SetSubcmd::AutoInstall { auto_install_mode } => cfg
                .set_auto_install(auto_install_mode)
                .map(|_| utils::ExitCode(0)),
        },
        RustupSubcmd::Completions { shell, command } => {
            output_completion_script(shell, command, process)
        }
    }
}

async fn default_(
    cfg: &Cfg<'_>,
    toolchain: Option<MaybeResolvableToolchainName>,
    force_non_host: bool,
) -> Result<utils::ExitCode> {
    common::warn_if_host_is_emulated(cfg.process);

    if let Some(toolchain) = toolchain {
        match toolchain.to_owned() {
            MaybeResolvableToolchainName::None => {
                cfg.set_default(None)?;
            }
            MaybeResolvableToolchainName::Some(ResolvableToolchainName::Custom(toolchain_name)) => {
                Toolchain::new(cfg, (&toolchain_name).into())?;
                cfg.set_default(Some(&toolchain_name.into()))?;
            }
            MaybeResolvableToolchainName::Some(ResolvableToolchainName::Official(toolchain)) => {
                let desc = toolchain.resolve(&cfg.get_default_host_triple()?)?;
                let status = cfg
                    .ensure_installed(&desc, vec![], vec![], None, force_non_host, true)
                    .await?
                    .0;

                cfg.set_default(Some(&(&desc).into()))?;

                writeln!(cfg.process.stdout().lock())?;

                common::show_channel_update(cfg, PackageUpdate::Toolchain(desc), Ok(status))?;
            }
        };

        if let Some((toolchain, reason)) = cfg.active_toolchain()? {
            if !matches!(reason, ActiveReason::Default) {
                info!("note that the toolchain '{toolchain}' is currently in use ({reason})");
            }
        }
    } else {
        let default_toolchain = cfg
            .get_default()?
            .ok_or_else(|| anyhow!("no default toolchain is configured"))?;
        writeln!(cfg.process.stdout().lock(), "{default_toolchain} (default)")?;
    }

    Ok(utils::ExitCode(0))
}

async fn check_updates(cfg: &Cfg<'_>, opts: CheckOpts) -> Result<utils::ExitCode> {
    let mut update_available = false;

    let mut t = cfg.process.stdout().terminal(cfg.process);
    let channels = cfg.list_channels()?;

    for channel in channels {
        let (name, distributable) = channel;
        let current_version = distributable.show_version()?;
        let dist_version = distributable.show_dist_version().await?;
        let _ = t.attr(terminalsource::Attr::Bold);
        write!(t.lock(), "{name} - ")?;
        match (current_version, dist_version) {
            (None, None) => {
                let _ = t.fg(terminalsource::Color::Red);
                writeln!(t.lock(), "Cannot identify installed or update versions")?;
            }
            (Some(cv), None) => {
                let _ = t.fg(terminalsource::Color::Green);
                write!(t.lock(), "Up to date")?;
                let _ = t.reset();
                writeln!(t.lock(), " : {cv}")?;
            }
            (Some(cv), Some(dv)) => {
                update_available = true;
                let _ = t.fg(terminalsource::Color::Yellow);
                write!(t.lock(), "Update available")?;
                let _ = t.reset();
                writeln!(t.lock(), " : {cv} -> {dv}")?;
            }
            (None, Some(dv)) => {
                update_available = true;
                let _ = t.fg(terminalsource::Color::Yellow);
                write!(t.lock(), "Update available")?;
                let _ = t.reset();
                writeln!(t.lock(), " : (Unknown version) -> {dv}")?;
            }
        }
    }

    let self_update_mode = cfg.get_self_update_mode()?;
    // Priority: no-self-update feature > self_update_mode > no-self-update args.
    // Check for update only if rustup does **not** have the no-self-update feature,
    // and auto-self-update is configured to **enable**
    // and has **no** no-self-update parameter.
    let self_update = !self_update::NEVER_SELF_UPDATE
        && self_update_mode == SelfUpdateMode::Enable
        && !opts.no_self_update;

    if self_update
        && matches!(
            check_rustup_update(cfg.process).await?,
            RustupUpdateAvailable::True
        )
    {
        update_available = true;
    }

    let exit_status = if update_available { 0 } else { 1 };
    Ok(utils::ExitCode(exit_status))
}

async fn update(
    cfg: &mut Cfg<'_>,
    opts: UpdateOpts,
    ensure_active_toolchain: bool,
) -> Result<utils::ExitCode> {
    let mut exit_code = utils::ExitCode(0);

    common::warn_if_host_is_emulated(cfg.process);
    let self_update_mode = cfg.get_self_update_mode()?;
    // Priority: no-self-update feature > self_update_mode > no-self-update args.
    // Update only if rustup does **not** have the no-self-update feature,
    // and auto-self-update is configured to **enable**
    // and has **no** no-self-update parameter.
    let self_update = !self_update::NEVER_SELF_UPDATE
        && self_update_mode == SelfUpdateMode::Enable
        && !opts.no_self_update;
    let force_non_host = opts.force_non_host;
    if let Some(p) = opts.profile {
        cfg.set_profile_override(p);
    }
    let cfg = &cfg;
    if cfg.get_profile()? == Profile::Complete {
        warn!("{}", common::WARN_COMPLETE_PROFILE);
    }
    let names = opts.toolchain;
    if !names.is_empty() {
        for name in names {
            // This needs another pass to fix it all up
            if name.has_triple() {
                let host_arch = TargetTriple::from_host_or_build(cfg.process);
                let target_triple = name.clone().resolve(&host_arch)?.target;
                common::check_non_host_toolchain(
                    name.to_string(),
                    &host_arch,
                    &target_triple,
                    force_non_host,
                )?;
            }
            let desc = name.resolve(&cfg.get_default_host_triple()?)?;

            let components = opts.component.iter().map(|s| &**s).collect::<Vec<_>>();
            let targets = opts.target.iter().map(|s| &**s).collect::<Vec<_>>();

            let force = opts.force;
            let allow_downgrade = opts.allow_downgrade;
            let profile = cfg.get_profile()?;
            let status = match DistributableToolchain::new(cfg, desc.clone()) {
                Ok(mut d) => {
                    d.update_extra(&components, &targets, profile, force, allow_downgrade)
                        .await?
                }
                Err(RustupError::ToolchainNotInstalled { .. }) => {
                    DistributableToolchain::install(
                        cfg,
                        &desc,
                        &components,
                        &targets,
                        profile,
                        force,
                    )
                    .await?
                    .0
                }
                Err(e) => Err(e)?,
            };

            writeln!(cfg.process.stdout().lock())?;
            common::show_channel_update(
                cfg,
                PackageUpdate::Toolchain(desc.clone()),
                Ok(status.clone()),
            )?;
            if cfg.get_default()?.is_none() && matches!(status, UpdateStatus::Installed) {
                cfg.set_default(Some(&desc.into()))?;
            }
        }
        if self_update {
            exit_code &= common::self_update(|| Ok(()), cfg.process).await?;
        }
    } else if ensure_active_toolchain {
        let (toolchain, reason) = cfg.ensure_active_toolchain(force_non_host, true).await?;
        info!("the active toolchain `{toolchain}` has been installed");
        info!("it's active because: {reason}");
    } else {
        exit_code &= common::update_all_channels(cfg, self_update, opts.force).await?;
        info!("cleaning up downloads & tmp directories");
        utils::delete_dir_contents_following_links(&cfg.download_dir);
        cfg.tmp_cx.clean();
    }

    if !self_update::NEVER_SELF_UPDATE && self_update_mode == SelfUpdateMode::CheckOnly {
        check_rustup_update(cfg.process).await?;
    }

    if self_update::NEVER_SELF_UPDATE {
        info!("self-update is disabled for this build of rustup");
        info!("any updates to rustup will need to be fetched with your system package manager")
    }

    Ok(exit_code)
}

async fn run(
    cfg: &Cfg<'_>,
    toolchain: ResolvableLocalToolchainName,
    command: Vec<String>,
    install: bool,
) -> Result<ExitStatus> {
    let toolchain = toolchain.resolve(&cfg.get_default_host_triple()?)?;
    let toolchain = Toolchain::from_local(toolchain, install, cfg).await?;
    let cmd = toolchain.command(&command[0])?;
    command::run_command_for_dir(cmd, &command[0], &command[1..])
}

async fn which(
    cfg: &Cfg<'_>,
    binary: &str,
    toolchain: Option<ResolvableToolchainName>,
) -> Result<utils::ExitCode> {
    let binary_path = cfg.resolve_toolchain(toolchain).await?.binary_file(binary);

    utils::assert_is_file(&binary_path)?;

    writeln!(cfg.process.stdout().lock(), "{}", binary_path.display())?;
    Ok(utils::ExitCode(0))
}

#[tracing::instrument(level = "trace", skip_all)]
async fn show(cfg: &Cfg<'_>, verbose: bool) -> Result<utils::ExitCode> {
    common::warn_if_host_is_emulated(cfg.process);

    // Print host triple
    {
        let mut t = cfg.process.stdout().terminal(cfg.process);
        t.attr(terminalsource::Attr::Bold)?;
        write!(t.lock(), "Default host: ")?;
        t.reset()?;
        writeln!(t.lock(), "{}", cfg.get_default_host_triple()?)?;
    }

    // Print rustup home directory
    {
        let mut t = cfg.process.stdout().terminal(cfg.process);
        t.attr(terminalsource::Attr::Bold)?;
        write!(t.lock(), "rustup home:  ")?;
        t.reset()?;
        writeln!(t.lock(), "{}", cfg.rustup_dir.display())?;
        writeln!(t.lock())?;
    }

    let installed_toolchains = cfg.list_toolchains()?;
    let active_toolchain_and_reason: Option<(ToolchainName, ActiveReason)> =
        if let Ok(Some((LocalToolchainName::Named(toolchain_name), reason))) =
            cfg.maybe_ensure_active_toolchain(None).await
        {
            Some((toolchain_name, reason))
        } else {
            None
        };

    let (active_toolchain_name, _active_reason) = active_toolchain_and_reason
        .as_ref()
        .map(|atar| (&atar.0, &atar.1))
        .unzip();

    let active_toolchain_targets: Vec<TargetTriple> = active_toolchain_name
        .and_then(|atn| match atn {
            ToolchainName::Official(desc) => DistributableToolchain::new(cfg, desc.clone())
                .ok()
                .and_then(|distributable| distributable.components().ok())
                .map(|cs_vec| {
                    cs_vec
                        .into_iter()
                        .filter(|c| {
                            c.installed && c.component.short_name_in_manifest() == "rust-std"
                        })
                        .map(|c| c.component.target.expect("rust-std should have a target"))
                        .collect()
                }),
            ToolchainName::Custom(name) => {
                Toolchain::new(cfg, LocalToolchainName::Named(name.into()))
                    .ok()?
                    .installed_targets()
                    .ok()
            }
        })
        .unwrap_or_default();

    // show installed toolchains
    {
        let mut t = cfg.process.stdout().terminal(cfg.process);

        print_header::<Error>(&mut t, "installed toolchains")?;

        let default_toolchain_name = cfg.get_default()?;
        let last_index = installed_toolchains.len().wrapping_sub(1);
        for (n, toolchain_name) in installed_toolchains.into_iter().enumerate() {
            let is_default_toolchain = default_toolchain_name.as_ref() == Some(&toolchain_name);
            let is_active_toolchain = active_toolchain_name == Some(&toolchain_name);

            let status_str = match (is_active_toolchain, is_default_toolchain) {
                (true, true) => " (active, default)",
                (true, false) => " (active)",
                (false, true) => " (default)",
                (false, false) => "",
            };

            writeln!(t.lock(), "{toolchain_name}{status_str}")?;

            if verbose {
                let toolchain = Toolchain::new(cfg, toolchain_name.into())?;
                writeln!(
                    cfg.process.stdout().lock(),
                    "  {}\n  path: {}",
                    toolchain.rustc_version(),
                    toolchain.path().display()
                )?;
                if n != last_index {
                    writeln!(cfg.process.stdout().lock())?;
                }
            }
        }
    }

    // show active toolchain
    {
        let mut t = cfg.process.stdout().terminal(cfg.process);

        writeln!(t.lock())?;

        print_header::<Error>(&mut t, "active toolchain")?;

        match active_toolchain_and_reason {
            Some((active_toolchain_name, active_reason)) => {
                let active_toolchain = Toolchain::with_reason(
                    cfg,
                    active_toolchain_name.clone().into(),
                    &active_reason,
                )?;
                writeln!(t.lock(), "name: {}", active_toolchain.name())?;
                writeln!(t.lock(), "active because: {active_reason}")?;
                if verbose {
                    writeln!(t.lock(), "compiler: {}", active_toolchain.rustc_version())?;
                    writeln!(t.lock(), "path: {}", active_toolchain.path().display())?;
                }

                // show installed targets for the active toolchain
                writeln!(t.lock(), "installed targets:")?;

                for target in active_toolchain_targets {
                    writeln!(t.lock(), "  {target}")?;
                }
            }
            None => {
                writeln!(t.lock(), "no active toolchain")?;
            }
        }
    }

    fn print_header<E>(t: &mut ColorableTerminal, s: &str) -> std::result::Result<(), E>
    where
        E: From<std::io::Error>,
    {
        t.attr(terminalsource::Attr::Bold)?;
        {
            let mut term_lock = t.lock();
            writeln!(term_lock, "{s}")?;
            writeln!(term_lock, "{}", "-".repeat(s.len()))?;
        } // drop the term_lock
        t.reset()?;
        Ok(())
    }

    Ok(utils::ExitCode(0))
}

#[tracing::instrument(level = "trace", skip_all)]
async fn show_active_toolchain(cfg: &Cfg<'_>, verbose: bool) -> Result<utils::ExitCode> {
    match cfg.maybe_ensure_active_toolchain(None).await? {
        Some((toolchain_name, reason)) => {
            let toolchain = Toolchain::with_reason(cfg, toolchain_name.clone(), &reason)?;
            if verbose {
                writeln!(
                    cfg.process.stdout().lock(),
                    "{}\nactive because: {}\ncompiler: {}\npath: {}",
                    toolchain.name(),
                    reason,
                    toolchain.rustc_version(),
                    toolchain.path().display()
                )?;
            } else {
                writeln!(
                    cfg.process.stdout().lock(),
                    "{} ({})",
                    toolchain.name(),
                    match reason {
                        ActiveReason::Default => &"default" as &dyn fmt::Display,
                        _ => &reason,
                    }
                )?;
            }
        }
        None => return Err(anyhow!("no active toolchain")),
    }
    Ok(utils::ExitCode(0))
}

#[tracing::instrument(level = "trace", skip_all)]
fn show_rustup_home(cfg: &Cfg<'_>) -> Result<utils::ExitCode> {
    writeln!(cfg.process.stdout().lock(), "{}", cfg.rustup_dir.display())?;
    Ok(utils::ExitCode(0))
}

async fn target_list(
    cfg: &Cfg<'_>,
    toolchain: Option<PartialToolchainDesc>,
    installed_only: bool,
    quiet: bool,
) -> Result<utils::ExitCode> {
    // downcasting required because the toolchain files can name any toolchain
    let distributable = DistributableToolchain::from_partial(toolchain, cfg).await?;
    common::list_items(
        distributable,
        |c| {
            (c.component.short_name_in_manifest() == "rust-std").then(|| {
                c.component
                    .target
                    .as_deref()
                    .expect("rust-std should have a target")
            })
        },
        installed_only,
        quiet,
        cfg.process,
    )
}

async fn target_add(
    cfg: &Cfg<'_>,
    mut targets: Vec<String>,
    toolchain: Option<PartialToolchainDesc>,
) -> Result<utils::ExitCode> {
    // XXX: long term move this error to cli ? the normal .into doesn't work
    // because Result here is the wrong sort and expression type ascription
    // isn't a feature yet.
    // list_components *and* add_component would both be inappropriate for
    // custom toolchains.
    let distributable = DistributableToolchain::from_partial(toolchain, cfg).await?;
    let components = distributable.components()?;

    if targets.contains(&"all".to_string()) {
        if targets.len() != 1 {
            return Err(anyhow!(
                "`rustup target add {}` includes `all`",
                targets.join(" ")
            ));
        }

        targets.clear();
        for component in components {
            if component.component.short_name_in_manifest() == "rust-std"
                && component.available
                && !component.installed
            {
                let target = component
                    .component
                    .target
                    .as_ref()
                    .expect("rust-std should have a target");
                targets.push(target.to_string());
            }
        }
    }

    for target in targets {
        let new_component = Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new(target)),
            false,
        );
        distributable.add_component(new_component).await?;
    }

    Ok(utils::ExitCode(0))
}

async fn target_remove(
    cfg: &Cfg<'_>,
    targets: Vec<String>,
    toolchain: Option<PartialToolchainDesc>,
) -> Result<utils::ExitCode> {
    let distributable = DistributableToolchain::from_partial(toolchain, cfg).await?;

    for target in targets {
        let target = TargetTriple::new(target);
        let default_target = cfg.get_default_host_triple()?;
        if target == default_target {
            warn!(
                "removing the default host target; proc-macros and build scripts might no longer build"
            );
        }
        // Whether we have at most 1 component target that is not `None` (wildcard).
        let has_at_most_one_target = distributable
            .components()?
            .into_iter()
            .filter_map(|c| match (c.installed, c.component.target) {
                (true, Some(t)) => Some(t),
                _ => None,
            })
            .unique()
            .at_most_one()
            .is_ok();
        if has_at_most_one_target {
            warn!("removing the last target; no build targets will be available");
        }
        let new_component = Component::new("rust-std".to_string(), Some(target), false);
        distributable.remove_component(new_component).await?;
    }

    Ok(utils::ExitCode(0))
}

async fn component_list(
    cfg: &Cfg<'_>,
    toolchain: Option<PartialToolchainDesc>,
    installed_only: bool,
    quiet: bool,
) -> Result<utils::ExitCode> {
    // downcasting required because the toolchain files can name any toolchain
    let distributable = DistributableToolchain::from_partial(toolchain, cfg).await?;
    common::list_items(
        distributable,
        |c| Some(&c.name),
        installed_only,
        quiet,
        cfg.process,
    )
}

async fn component_add(
    cfg: &Cfg<'_>,
    components: Vec<String>,
    toolchain: Option<PartialToolchainDesc>,
    target: Option<String>,
) -> Result<utils::ExitCode> {
    let distributable = DistributableToolchain::from_partial(toolchain, cfg).await?;
    let target = get_target(target, &distributable);

    for component in &components {
        let new_component = Component::try_new(component, &distributable, target.as_ref())?;
        distributable.add_component(new_component).await?;
    }

    Ok(utils::ExitCode(0))
}

fn get_target(
    target: Option<String>,
    distributable: &DistributableToolchain<'_>,
) -> Option<TargetTriple> {
    target
        .map(TargetTriple::new)
        .or_else(|| Some(distributable.desc().target.clone()))
}

async fn component_remove(
    cfg: &Cfg<'_>,
    components: Vec<String>,
    toolchain: Option<PartialToolchainDesc>,
    target: Option<String>,
) -> Result<utils::ExitCode> {
    let distributable = DistributableToolchain::from_partial(toolchain, cfg).await?;
    let target = get_target(target, &distributable);

    for component in &components {
        let new_component = Component::try_new(component, &distributable, target.as_ref())?;
        distributable.remove_component(new_component).await?;
    }

    Ok(utils::ExitCode(0))
}

async fn toolchain_link(
    cfg: &Cfg<'_>,
    dest: &CustomToolchainName,
    src: &Path,
) -> Result<utils::ExitCode> {
    cfg.ensure_toolchains_dir()?;
    let mut pathbuf = PathBuf::from(src);

    pathbuf.push("lib");
    utils::assert_is_directory(&pathbuf)?;
    pathbuf.pop();
    pathbuf.push("bin");
    utils::assert_is_directory(&pathbuf)?;
    pathbuf.push(format!("rustc{EXE_SUFFIX}"));
    utils::assert_is_file(&pathbuf)?;

    if true {
        InstallMethod::Link {
            src: &cfg.current_dir.join(src),
            dest,
            cfg,
        }
        .install()
        .await?;
    } else {
        InstallMethod::Copy { src, dest, cfg }.install().await?;
    }

    Ok(utils::ExitCode(0))
}

async fn toolchain_remove(cfg: &mut Cfg<'_>, opts: UninstallOpts) -> Result<utils::ExitCode> {
    let default_toolchain = cfg.get_default().ok().flatten();
    let active_toolchain = cfg
        .maybe_ensure_active_toolchain(Some(false))
        .await
        .ok()
        .flatten()
        .map(|(it, _)| it);

    for toolchain_name in &opts.toolchain {
        let toolchain_name = toolchain_name.resolve(&cfg.get_default_host_triple()?)?;

        if active_toolchain
            .as_ref()
            .is_some_and(|n| n == &toolchain_name)
        {
            warn!(
                "removing the active toolchain; a toolchain override will be required for running Rust tools"
            );
        }
        if default_toolchain
            .as_ref()
            .is_some_and(|n| n == &toolchain_name)
        {
            warn!(
                "removing the default toolchain; proc-macros and build scripts might no longer build"
            );
        }

        Toolchain::ensure_removed(cfg, (&toolchain_name).into())?;
    }
    Ok(utils::ExitCode(0))
}

async fn override_add(
    cfg: &Cfg<'_>,
    toolchain: ResolvableToolchainName,
    path: Option<&Path>,
) -> Result<utils::ExitCode> {
    let toolchain_name = toolchain.resolve(&cfg.get_default_host_triple()?)?;
    match Toolchain::new(cfg, (&toolchain_name).into()) {
        Ok(_) => {}
        Err(e @ RustupError::ToolchainNotInstalled { .. }) => match &toolchain_name {
            ToolchainName::Custom(_) => Err(e)?,
            ToolchainName::Official(desc) => {
                let status =
                    DistributableToolchain::install(cfg, desc, &[], &[], cfg.get_profile()?, false)
                        .await?
                        .0;
                writeln!(cfg.process.stdout().lock())?;
                common::show_channel_update(
                    cfg,
                    PackageUpdate::Toolchain(desc.clone()),
                    Ok(status),
                )?;
            }
        },
        Err(e) => Err(e)?,
    }

    cfg.make_override(path.unwrap_or(&cfg.current_dir), &toolchain_name)?;
    Ok(utils::ExitCode(0))
}

fn override_remove(
    cfg: &Cfg<'_>,
    path: Option<&Path>,
    nonexistent: bool,
) -> Result<utils::ExitCode> {
    let paths = if nonexistent {
        let list: Vec<_> = cfg.settings_file.with(|s| {
            Ok(s.overrides
                .iter()
                .filter_map(|(k, _)| {
                    let path = Path::new(k);
                    (!path.is_dir()).then(|| path.to_owned())
                })
                .collect())
        })?;
        if list.is_empty() {
            info!("no nonexistent paths detected");
        }
        list
    } else if let Some(path) = path {
        vec![path.to_owned()]
    } else {
        vec![cfg.current_dir.clone()]
    };

    for p in &paths {
        if cfg
            .settings_file
            .with_mut(|s| Ok(s.remove_override(p, cfg.notify_handler.as_ref())))?
        {
            info!("override toolchain for '{}' removed", p.display());
        } else {
            info!("no override toolchain for '{}'", p.display());
            if path.is_none() && !nonexistent {
                info!(
                    "you may use `--path <path>` option to remove override toolchain \
                     for a specific path"
                );
            }
        }
    }
    Ok(utils::ExitCode(0))
}

macro_rules! docs_data {
    (
        $(
            $( #[$meta:meta] )*
            ($ident:ident, $help:expr, $path:expr $(,)?)
        ),+ $(,)?
    ) => {
        #[derive(Debug, Args)]
        struct DocPage {
            $(
                #[doc = $help]
                #[arg(long, group = "page")]
                $( #[$meta] )*
                $ident: bool,
            )+
        }

        impl DocPage {
            fn path_str(&self) -> Option<&'static str> {
                $( if self.$ident { return Some($path); } )+
                None
            }
        }
    };
}

docs_data![
    // flags can be used to open specific documents, e.g. `rustup doc --nomicon`
    // tuple elements: document name used as flag, help message, document index path
    (
        alloc,
        "The Rust core allocation and collections library",
        "alloc/index.html"
    ),
    (
        book,
        "The Rust Programming Language book",
        "book/index.html"
    ),
    (cargo, "The Cargo Book", "cargo/index.html"),
    (clippy, "The Clippy Documentation", "clippy/index.html"),
    (core, "The Rust Core Library", "core/index.html"),
    (
        edition_guide,
        "The Rust Edition Guide",
        "edition-guide/index.html"
    ),
    (
        embedded_book,
        "The Embedded Rust Book",
        "embedded-book/index.html"
    ),
    (
        error_codes,
        "The Rust Error Codes Index",
        "error_codes/index.html"
    ),
    (
        nomicon,
        "The Dark Arts of Advanced and Unsafe Rust Programming",
        "nomicon/index.html"
    ),
    #[arg(long = "proc_macro")]
    (
        proc_macro,
        "A support library for macro authors when defining new macros",
        "proc_macro/index.html"
    ),
    (reference, "The Rust Reference", "reference/index.html"),
    (
        rust_by_example,
        "A collection of runnable examples that illustrate various Rust concepts and standard libraries",
        "rust-by-example/index.html"
    ),
    (
        rustc,
        "The compiler for the Rust programming language",
        "rustc/index.html"
    ),
    (
        rustdoc,
        "Documentation generator for Rust projects",
        "rustdoc/index.html"
    ),
    (std, "Standard library API documentation", "std/index.html"),
    (
        style_guide,
        "The Rust Style Guide",
        "style-guide/index.html"
    ),
    (
        test,
        "Support code for rustc's built in unit-test and micro-benchmarking framework",
        "test/index.html"
    ),
    (
        unstable_book,
        "The Unstable Book",
        "unstable-book/index.html"
    ),
];

impl DocPage {
    fn path(&self) -> Option<&'static Path> {
        self.path_str().map(Path::new)
    }

    fn name(&self) -> Option<&'static str> {
        Some(self.path_str()?.rsplit_once('/')?.0)
    }

    fn resolve<'t>(&self, root: &Path, topic: &'t str) -> Option<(PathBuf, Option<&'t str>)> {
        // Use `.parent()` to chop off the default top-level `index.html`.
        let mut base = root.join(Path::new(self.path()?).parent()?);
        base.extend(topic.split("::"));
        let base_index_html = base.join("index.html");

        if base_index_html.is_file() {
            return Some((base_index_html, None));
        }

        let base_html = base.with_extension("html");
        if base_html.is_file() {
            return Some((base_html, None));
        }

        let parent_html = base.parent()?.with_extension("html");
        if parent_html.is_file() {
            return Some((parent_html, topic.rsplit_once("::").map(|(_, s)| s)));
        }

        None
    }
}

async fn doc(
    cfg: &Cfg<'_>,
    path_only: bool,
    toolchain: Option<PartialToolchainDesc>,
    mut topic: Option<&str>,
    doc_page: &DocPage,
) -> Result<utils::ExitCode> {
    let toolchain = cfg.toolchain_from_partial(toolchain).await?;

    if let Ok(distributable) = DistributableToolchain::try_from(&toolchain) {
        if let [_] = distributable
            .components()?
            .into_iter()
            .filter(|cstatus| {
                cstatus.component.short_name_in_manifest() == "rust-docs" && !cstatus.installed
            })
            .take(1)
            .collect::<Vec<ComponentStatus>>()
            .as_slice()
        {
            info!(
                "`rust-docs` not installed in toolchain `{}`",
                distributable.desc()
            );
            info!(
                "To install, try `rustup component add --toolchain {} rust-docs`",
                distributable.desc()
            );
            return Err(anyhow!(
                "unable to view documentation which is not installed"
            ));
        }
    };

    let (doc_path, fragment) = match (topic, doc_page.name()) {
        (Some(topic), Some(name)) => {
            let (doc_path, fragment) = doc_page
                .resolve(&toolchain.doc_path("")?, topic)
                .context(format!("no document for {name} on {topic}"))?;
            (Cow::Owned(doc_path), fragment)
        }
        (Some(topic), None) => {
            let doc_path = topical_doc::local_path(&toolchain.doc_path("").unwrap(), topic)?;
            (Cow::Owned(doc_path), None)
        }
        (None, name) => {
            topic = name;
            let doc_path = doc_page.path().unwrap_or(Path::new("index.html"));
            (Cow::Borrowed(doc_path), None)
        }
    };

    if path_only {
        let doc_path = toolchain.doc_path(&doc_path)?;
        writeln!(cfg.process.stdout().lock(), "{}", doc_path.display())?;
        return Ok(utils::ExitCode(0));
    }

    if let Some(name) = topic {
        writeln!(
            cfg.process.stderr().lock(),
            "Opening docs named `{name}` in your browser"
        )?;
    } else {
        writeln!(cfg.process.stderr().lock(), "Opening docs in your browser")?;
    }
    toolchain.open_docs(&doc_path, fragment)?;
    Ok(utils::ExitCode(0))
}

#[cfg(not(windows))]
async fn man(
    cfg: &Cfg<'_>,
    command: &str,
    toolchain: Option<PartialToolchainDesc>,
) -> Result<utils::ExitCode> {
    let toolchain = cfg.toolchain_from_partial(toolchain).await?;
    let path = toolchain.man_path();
    utils::assert_is_directory(&path)?;

    let mut manpaths = std::ffi::OsString::from(path);
    manpaths.push(":"); // prepend to the default MANPATH list
    if let Some(path) = cfg.process.var_os("MANPATH") {
        manpaths.push(path);
    }
    std::process::Command::new("man")
        .env("MANPATH", manpaths)
        .arg(command)
        .status()
        .expect("failed to open man page");
    Ok(utils::ExitCode(0))
}

fn set_auto_self_update(
    cfg: &mut Cfg<'_>,
    auto_self_update_mode: SelfUpdateMode,
) -> Result<utils::ExitCode> {
    if self_update::NEVER_SELF_UPDATE {
        let mut args = cfg.process.args_os();
        let arg0 = args.next().map(PathBuf::from);
        let arg0 = arg0
            .as_ref()
            .and_then(|a| a.to_str())
            .ok_or(CLIError::NoExeName)?;
        warn!(
            "{} is built with the no-self-update feature: setting auto-self-update will not have any effect.",
            arg0
        );
    }
    cfg.set_auto_self_update(auto_self_update_mode)?;
    Ok(utils::ExitCode(0))
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum CompletionCommand {
    Rustup,
    Cargo,
}

impl clap::ValueEnum for CompletionCommand {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Rustup, Self::Cargo]
    }

    fn to_possible_value<'a>(&self) -> Option<PossibleValue> {
        Some(match self {
            CompletionCommand::Rustup => PossibleValue::new("rustup"),
            CompletionCommand::Cargo => PossibleValue::new("cargo"),
        })
    }
}

impl fmt::Display for CompletionCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_possible_value() {
            Some(v) => write!(f, "{}", v.get_name()),
            None => unreachable!(),
        }
    }
}

fn output_completion_script(
    shell: Shell,
    command: CompletionCommand,
    process: &Process,
) -> Result<utils::ExitCode> {
    match command {
        CompletionCommand::Rustup => {
            clap_complete::generate(
                shell,
                &mut Rustup::command(),
                "rustup",
                &mut process.stdout().lock(),
            );
        }
        CompletionCommand::Cargo => {
            if let Shell::Zsh = shell {
                writeln!(process.stdout().lock(), "#compdef cargo")?;
            }

            let script = match shell {
                Shell::Bash => "/etc/bash_completion.d/cargo",
                Shell::Zsh => "/share/zsh/site-functions/_cargo",
                _ => {
                    return Err(anyhow!(
                        "{} does not currently support completions for {}",
                        command,
                        shell
                    ));
                }
            };

            writeln!(
                process.stdout().lock(),
                "if command -v rustc >/dev/null 2>&1; then\n\
                    \tsource \"$(rustc --print sysroot)\"{script}\n\
                 fi",
            )?;
        }
    }

    Ok(utils::ExitCode(0))
}
