use std::env::consts::EXE_SUFFIX;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::str::FromStr;

use anyhow::{anyhow, Error, Result};
use clap::{
    builder::{PossibleValue, PossibleValuesParser},
    Args, CommandFactory, Parser, Subcommand, ValueEnum,
};
use clap_complete::Shell;
use itertools::Itertools;

use crate::{
    cli::{
        common::{self, PackageUpdate},
        errors::CLIError,
        help::*,
        self_update::{self, check_rustup_update, SelfUpdateMode},
        topical_doc,
    },
    command,
    config::{new_toolchain_with_reason, ActiveReason, Cfg},
    currentprocess::{
        argsource::ArgSource,
        filesource::{StderrSource, StdoutSource},
        process,
        terminalsource::{self, ColorableTerminal},
    },
    dist::{
        dist::{PartialToolchainDesc, Profile, TargetTriple},
        manifest::{Component, ComponentStatus},
    },
    errors::RustupError,
    install::{InstallMethod, UpdateStatus},
    toolchain::{
        distributable::DistributableToolchain,
        names::{
            CustomToolchainName, LocalToolchainName, MaybeResolvableToolchainName,
            ResolvableLocalToolchainName, ResolvableToolchainName, ToolchainName,
        },
        toolchain::Toolchain,
    },
    utils::utils::{self, ExitCode},
};

const TOOLCHAIN_OVERRIDE_ERROR: &str =
    "To override the toolchain using the 'rustup +toolchain' syntax, \
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
    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Disable progress output
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
    use clap::{error::ErrorKind, Error};
    if let Some(stripped) = s.strip_prefix('+') {
        ResolvableToolchainName::try_from(stripped)
            .map_err(|e| Error::raw(ErrorKind::InvalidValue, e))
    } else {
        Err(Error::raw(ErrorKind::InvalidSubcommand, format!("\"{s}\" is not a valid subcommand, so it was interpreted as a toolchain name, but it is also invalid. {TOOLCHAIN_OVERRIDE_ERROR}")))
    }
}

#[derive(Debug, Subcommand)]
#[command(name = "rustup", bin_name = "rustup[EXE]")]
enum RustupSubcmd {
    /// Update Rust toolchains
    #[command(hide = true, after_help = INSTALL_HELP)]
    Install {
        #[command(flatten)]
        opts: UpdateOpts,
    },

    /// Uninstall Rust toolchains
    #[command(hide = true)]
    Uninstall {
        #[command(flatten)]
        opts: UninstallOpts,
    },

    /// Dump information about the build
    #[command(hide = true)]
    DumpTestament,

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
        #[arg(num_args = 1..)]
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
    Check,

    /// Set the default toolchain
    #[command(after_help = DEFAULT_HELP)]
    Default {
        #[arg(help = MAYBE_RESOLVABLE_TOOLCHAIN_ARG_HELP)]
        toolchain: Option<MaybeResolvableToolchainName>,
    },

    /// Modify or query the installed toolchains
    Toolchain {
        #[command(subcommand)]
        subcmd: ToolchainSubcmd,
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

        #[arg(required = true, num_args = 1.., use_value_delimiter = false)]
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
    },

    /// Install or update a given toolchain
    #[command(aliases = ["update", "add"] )]
    Install {
        #[command(flatten)]
        opts: UpdateOpts,
    },

    /// Uninstall a toolchain
    #[command(alias = "remove")]
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
struct UpdateOpts {
    #[arg(
        required = true,
        help = OFFICIAL_TOOLCHAIN_ARG_HELP,
        num_args = 1..,
    )]
    toolchain: Vec<PartialToolchainDesc>,

    #[arg(long, value_parser = PossibleValuesParser::new(Profile::names()))]
    profile: Option<String>,

    /// Add specific components on installation
    #[arg(short, long, value_delimiter = ',', num_args = 1..)]
    component: Vec<String>,

    /// Add specific targets on installation
    #[arg(short, long, value_delimiter = ',', num_args = 1..)]
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
    #[command(alias = "uninstall")]
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
    #[command(alias = "remove", after_help = OVERRIDE_UNSET_HELP)]
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
        #[arg(
            default_value = Profile::default_name(),
            value_parser = PossibleValuesParser::new(Profile::names()),
        )]
        profile_name: String,
    },

    /// The rustup auto self update mode
    AutoSelfUpdate {
        #[arg(
            default_value = SelfUpdateMode::default_mode(),
            value_parser = PossibleValuesParser::new(SelfUpdateMode::modes()),
        )]
        auto_self_update_mode: String,
    },
}

#[cfg_attr(feature = "otel", tracing::instrument(fields(args = format!("{:?}", process().args_os().collect::<Vec<_>>()))))]
pub fn main() -> Result<utils::ExitCode> {
    self_update::cleanup_self_updater()?;

    use clap::error::ErrorKind::*;
    let matches = match Rustup::try_parse_from(process().args_os()) {
        Ok(matches) => Ok(matches),
        Err(err) if err.kind() == DisplayHelp => {
            write!(process().stdout().lock(), "{err}")?;
            return Ok(utils::ExitCode(0));
        }
        Err(err) if err.kind() == DisplayVersion => {
            write!(process().stdout().lock(), "{err}")?;
            info!("This is the version for the rustup toolchain manager, not the rustc compiler.");

            #[cfg_attr(feature = "otel", tracing::instrument)]
            fn rustc_version() -> std::result::Result<String, Box<dyn std::error::Error>> {
                let cfg = &mut common::set_globals(false, true)?;
                let cwd = std::env::current_dir()?;

                if let Some(t) = process().args().find(|x| x.starts_with('+')) {
                    debug!("Fetching rustc version from toolchain `{}`", t);
                    cfg.set_toolchain_override(&ResolvableToolchainName::try_from(&t[1..])?);
                }

                let toolchain = cfg.find_or_install_active_toolchain(&cwd)?.0;

                Ok(toolchain.rustc_version())
            }

            match rustc_version() {
                Ok(version) => info!("The currently active `rustc` version is `{}`", version),
                Err(err) => debug!("Wanted to tell you the current rustc version, too, but ran into this error: {}", err),
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
                write!(process().stdout().lock(), "{err}")?;
                return Ok(utils::ExitCode(1));
            }
            if err.kind() == ValueValidation && err.to_string().contains(TOOLCHAIN_OVERRIDE_ERROR) {
                write!(process().stderr().lock(), "{err}")?;
                return Ok(utils::ExitCode(1));
            }
            Err(err)
        }
    }?;
    let cfg = &mut common::set_globals(matches.verbose, matches.quiet)?;

    if let Some(t) = &matches.plus_toolchain {
        cfg.set_toolchain_override(t);
    }

    cfg.check_metadata_version()?;

    let Some(subcmd) = matches.subcmd else {
        eprintln!("{}", Rustup::command().render_long_help());
        return Ok(utils::ExitCode(1));
    };

    match subcmd {
        RustupSubcmd::DumpTestament => common::dump_testament(),
        RustupSubcmd::Install { opts } => update(cfg, opts),
        RustupSubcmd::Uninstall { opts } => toolchain_remove(cfg, opts),
        RustupSubcmd::Show { verbose, subcmd } => handle_epipe(match subcmd {
            None => show(cfg, verbose),
            Some(ShowSubcmd::ActiveToolchain { verbose }) => show_active_toolchain(cfg, verbose),
            Some(ShowSubcmd::Home) => show_rustup_home(cfg),
            Some(ShowSubcmd::Profile) => show_profile(cfg),
        }),
        RustupSubcmd::Update {
            toolchain,
            no_self_update,
            force,
            force_non_host,
        } => update(
            cfg,
            UpdateOpts {
                toolchain,
                no_self_update,
                force,
                force_non_host,
                ..UpdateOpts::default()
            },
        ),
        RustupSubcmd::Toolchain { subcmd } => match subcmd {
            ToolchainSubcmd::Install { opts } => update(cfg, opts),
            ToolchainSubcmd::List { verbose } => {
                handle_epipe(common::list_toolchains(cfg, verbose))
            }
            ToolchainSubcmd::Link { toolchain, path } => toolchain_link(cfg, &toolchain, &path),
            ToolchainSubcmd::Uninstall { opts } => toolchain_remove(cfg, opts),
        },
        RustupSubcmd::Check => check_updates(cfg),
        RustupSubcmd::Default { toolchain } => default_(cfg, toolchain),
        RustupSubcmd::Target { subcmd } => match subcmd {
            TargetSubcmd::List {
                toolchain,
                installed,
            } => handle_epipe(target_list(cfg, toolchain, installed)),
            TargetSubcmd::Add { target, toolchain } => target_add(cfg, target, toolchain),
            TargetSubcmd::Remove { target, toolchain } => target_remove(cfg, target, toolchain),
        },
        RustupSubcmd::Component { subcmd } => match subcmd {
            ComponentSubcmd::List {
                toolchain,
                installed,
            } => handle_epipe(component_list(cfg, toolchain, installed)),
            ComponentSubcmd::Add {
                component,
                toolchain,
                target,
            } => component_add(cfg, component, toolchain, target),
            ComponentSubcmd::Remove {
                component,
                toolchain,
                target,
            } => component_remove(cfg, component, toolchain, target),
        },
        RustupSubcmd::Override { subcmd } => match subcmd {
            OverrideSubcmd::List => handle_epipe(common::list_overrides(cfg)),
            OverrideSubcmd::Set { toolchain, path } => {
                override_add(cfg, toolchain, path.as_deref())
            }
            OverrideSubcmd::Unset { path, nonexistent } => {
                override_remove(cfg, path.as_deref(), nonexistent)
            }
        },
        RustupSubcmd::Run {
            toolchain,
            command,
            install,
        } => run(cfg, toolchain, command, install).map(ExitCode::from),
        RustupSubcmd::Which { command, toolchain } => which(cfg, &command, toolchain),
        RustupSubcmd::Doc {
            path,
            toolchain,
            topic,
            page,
        } => doc(cfg, path, toolchain, topic.as_deref(), &page),
        #[cfg(not(windows))]
        RustupSubcmd::Man { command, toolchain } => man(cfg, &command, toolchain),
        RustupSubcmd::Self_ { subcmd } => match subcmd {
            SelfSubcmd::Update => self_update::update(cfg),
            SelfSubcmd::Uninstall { no_prompt } => self_update::uninstall(no_prompt),
            SelfSubcmd::UpgradeData => cfg.upgrade_data().map(|_| ExitCode(0)),
        },
        RustupSubcmd::Set { subcmd } => match subcmd {
            SetSubcmd::DefaultHost { host_triple } => cfg
                .set_default_host_triple(host_triple)
                .map(|_| utils::ExitCode(0)),
            SetSubcmd::Profile { profile_name } => set_profile(cfg, &profile_name),
            SetSubcmd::AutoSelfUpdate {
                auto_self_update_mode,
            } => set_auto_self_update(cfg, &auto_self_update_mode),
        },
        RustupSubcmd::Completions { shell, command } => output_completion_script(shell, command),
    }
}

fn default_(cfg: &Cfg, toolchain: Option<MaybeResolvableToolchainName>) -> Result<utils::ExitCode> {
    common::warn_if_host_is_emulated();

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
                let status = DistributableToolchain::install_if_not_installed(cfg, &desc)?;

                cfg.set_default(Some(&(&desc).into()))?;

                writeln!(process().stdout().lock())?;

                common::show_channel_update(cfg, PackageUpdate::Toolchain(desc), Ok(status))?;
            }
        };

        let cwd = utils::current_dir()?;
        if let Some((toolchain, reason)) = cfg.find_active_toolchain(&cwd)? {
            if !matches!(reason, ActiveReason::Default) {
                info!("note that the toolchain '{toolchain}' is currently in use ({reason})");
            }
        }
    } else {
        match cfg.get_default()? {
            Some(default_toolchain) => {
                writeln!(process().stdout().lock(), "{default_toolchain} (default)")?;
            }
            None => writeln!(
                process().stdout().lock(),
                "no default toolchain is configured"
            )?,
        }
    }

    Ok(utils::ExitCode(0))
}

fn check_updates(cfg: &Cfg) -> Result<utils::ExitCode> {
    let mut t = process().stdout().terminal();
    let channels = cfg.list_channels()?;

    for channel in channels {
        let (name, distributable) = channel;
        let current_version = distributable.show_version()?;
        let dist_version = distributable.show_dist_version()?;
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
                let _ = t.fg(terminalsource::Color::Yellow);
                write!(t.lock(), "Update available")?;
                let _ = t.reset();
                writeln!(t.lock(), " : {cv} -> {dv}")?;
            }
            (None, Some(dv)) => {
                let _ = t.fg(terminalsource::Color::Yellow);
                write!(t.lock(), "Update available")?;
                let _ = t.reset();
                writeln!(t.lock(), " : (Unknown version) -> {dv}")?;
            }
        }
    }

    check_rustup_update()?;

    Ok(utils::ExitCode(0))
}

fn update(cfg: &mut Cfg, opts: UpdateOpts) -> Result<utils::ExitCode> {
    common::warn_if_host_is_emulated();
    let self_update_mode = cfg.get_self_update_mode()?;
    // Priority: no-self-update feature > self_update_mode > no-self-update args.
    // Update only if rustup does **not** have the no-self-update feature,
    // and auto-self-update is configured to **enable**
    // and has **no** no-self-update parameter.
    let self_update = !self_update::NEVER_SELF_UPDATE
        && self_update_mode == SelfUpdateMode::Enable
        && !opts.no_self_update;
    let forced = opts.force_non_host;
    if let Some(p) = &opts.profile {
        let p = Profile::from_str(p)?;
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
                let host_arch = TargetTriple::from_host_or_build();

                let target_triple = name.clone().resolve(&host_arch)?.target;
                if !forced && !host_arch.can_run(&target_triple)? {
                    err!("DEPRECATED: future versions of rustup will require --force-non-host to install a non-host toolchain.");
                    warn!("toolchain '{name}' may not be able to run on this system.");
                    warn!(
                            "If you meant to build software to target that platform, perhaps try `rustup target add {}` instead?",
                            target_triple.to_string()
                        );
                }
            }
            let desc = name.resolve(&cfg.get_default_host_triple()?)?;

            let components = opts.component.iter().map(|s| &**s).collect::<Vec<_>>();
            let targets = opts.target.iter().map(|s| &**s).collect::<Vec<_>>();

            let force = opts.force;
            let allow_downgrade = opts.allow_downgrade;
            let profile = cfg.get_profile()?;
            let status = match crate::toolchain::distributable::DistributableToolchain::new(
                cfg,
                desc.clone(),
            ) {
                Ok(mut d) => {
                    d.update_extra(&components, &targets, profile, force, allow_downgrade)?
                }
                Err(RustupError::ToolchainNotInstalled(_)) => {
                    crate::toolchain::distributable::DistributableToolchain::install(
                        cfg,
                        &desc,
                        &components,
                        &targets,
                        profile,
                        force,
                    )?
                    .0
                }
                Err(e) => Err(e)?,
            };

            writeln!(process().stdout().lock())?;
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
            common::self_update(|| Ok(utils::ExitCode(0)))?;
        }
    } else {
        common::update_all_channels(cfg, self_update, opts.force)?;
        info!("cleaning up downloads & tmp directories");
        utils::delete_dir_contents_following_links(&cfg.download_dir);
        cfg.tmp_cx.clean();
    }

    if !self_update::NEVER_SELF_UPDATE && self_update_mode == SelfUpdateMode::CheckOnly {
        check_rustup_update()?;
    }

    if self_update::NEVER_SELF_UPDATE {
        info!("self-update is disabled for this build of rustup");
        info!("any updates to rustup will need to be fetched with your system package manager")
    }

    Ok(utils::ExitCode(0))
}

fn run(
    cfg: &Cfg,
    toolchain: ResolvableLocalToolchainName,
    command: Vec<String>,
    install: bool,
) -> Result<ExitStatus> {
    let toolchain = toolchain.resolve(&cfg.get_default_host_triple()?)?;
    let cmd = cfg.create_command_for_toolchain(&toolchain, install, &command[0])?;
    command::run_command_for_dir(cmd, &command[0], &command[1..])
}

fn which(
    cfg: &Cfg,
    binary: &str,
    toolchain: Option<ResolvableToolchainName>,
) -> Result<utils::ExitCode> {
    let binary_path = if let Some(toolchain) = toolchain {
        let desc = toolchain.resolve(&cfg.get_default_host_triple()?)?;
        Toolchain::new(cfg, desc.into())?.binary_file(binary)
    } else {
        cfg.which_binary(&utils::current_dir()?, binary)?
    };

    utils::assert_is_file(&binary_path)?;

    writeln!(process().stdout().lock(), "{}", binary_path.display())?;
    Ok(utils::ExitCode(0))
}

#[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
fn show(cfg: &Cfg, verbose: bool) -> Result<utils::ExitCode> {
    common::warn_if_host_is_emulated();

    // Print host triple
    {
        let mut t = process().stdout().terminal();
        t.attr(terminalsource::Attr::Bold)?;
        write!(t.lock(), "Default host: ")?;
        t.reset()?;
        writeln!(t.lock(), "{}", cfg.get_default_host_triple()?)?;
    }

    // Print rustup home directory
    {
        let mut t = process().stdout().terminal();
        t.attr(terminalsource::Attr::Bold)?;
        write!(t.lock(), "rustup home:  ")?;
        t.reset()?;
        writeln!(t.lock(), "{}", cfg.rustup_dir.display())?;
        writeln!(t.lock())?;
    }

    let cwd = utils::current_dir()?;
    let installed_toolchains = cfg.list_toolchains()?;
    let active_toolchain_and_reason: Option<(ToolchainName, ActiveReason)> =
        if let Ok(Some((LocalToolchainName::Named(toolchain_name), reason))) =
            cfg.find_active_toolchain(&cwd)
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
            ToolchainName::Official(desc) => DistributableToolchain::new(cfg, desc.clone()).ok(),
            // So far, it is not possible to list targets for a custom toolchain.
            ToolchainName::Custom(_) => None,
        })
        .and_then(|distributable| distributable.components().ok())
        .map(|cs_vec| {
            cs_vec
                .into_iter()
                .filter(|c| c.installed && c.component.short_name_in_manifest() == "rust-std")
                .map(|c| c.component.target.expect("rust-std should have a target"))
                .collect()
        })
        .unwrap_or_default();

    // show installed toolchains
    {
        let mut t = process().stdout().terminal();

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
                writeln!(process().stdout().lock(), "  {}", toolchain.rustc_version())?;
                // To make it easy to see which rustc belongs to which
                // toolchain, we separate each pair with an extra newline.
                if n != last_index {
                    writeln!(process().stdout().lock())?;
                }
            }
        }
    }

    // show active toolchain
    {
        let mut t = process().stdout().terminal();

        writeln!(t.lock())?;

        print_header::<Error>(&mut t, "active toolchain")?;

        match active_toolchain_and_reason {
            Some((active_toolchain_name, active_reason)) => {
                let active_toolchain = new_toolchain_with_reason(
                    cfg,
                    active_toolchain_name.clone().into(),
                    &active_reason,
                )?;
                writeln!(t.lock(), "name: {}", active_toolchain.name())?;
                writeln!(t.lock(), "compiler: {}", active_toolchain.rustc_version())?;
                writeln!(t.lock(), "active because: {}", active_reason)?;

                // show installed targets for the active toolchain
                writeln!(t.lock(), "installed targets:")?;

                for target in active_toolchain_targets {
                    writeln!(t.lock(), "  {}", target)?;
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

#[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
fn show_active_toolchain(cfg: &Cfg, verbose: bool) -> Result<utils::ExitCode> {
    let cwd = utils::current_dir()?;
    match cfg.find_active_toolchain(&cwd)? {
        Some((toolchain_name, reason)) => {
            let toolchain = new_toolchain_with_reason(cfg, toolchain_name.clone(), &reason)?;
            writeln!(
                process().stdout().lock(),
                "{}\nactive because: {}",
                toolchain.name(),
                reason
            )?;
            if verbose {
                writeln!(
                    process().stdout().lock(),
                    "compiler: {}",
                    toolchain.rustc_version()
                )?;
            }
        }
        None => writeln!(process().stdout().lock(), "There isn't an active toolchain")?,
    }
    Ok(utils::ExitCode(0))
}

#[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
fn show_rustup_home(cfg: &Cfg) -> Result<utils::ExitCode> {
    writeln!(process().stdout().lock(), "{}", cfg.rustup_dir.display())?;
    Ok(utils::ExitCode(0))
}

fn target_list(
    cfg: &Cfg,
    toolchain: Option<PartialToolchainDesc>,
    installed_only: bool,
) -> Result<utils::ExitCode> {
    // downcasting required because the toolchain files can name any toolchain
    let distributable = DistributableToolchain::from_partial(toolchain, cfg)?;
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
    )
}

fn target_add(
    cfg: &Cfg,
    mut targets: Vec<String>,
    toolchain: Option<PartialToolchainDesc>,
) -> Result<utils::ExitCode> {
    // XXX: long term move this error to cli ? the normal .into doesn't work
    // because Result here is the wrong sort and expression type ascription
    // isn't a feature yet.
    // list_components *and* add_component would both be inappropriate for
    // custom toolchains.
    let distributable = DistributableToolchain::from_partial(toolchain, cfg)?;
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
        distributable.add_component(new_component)?;
    }

    Ok(utils::ExitCode(0))
}

fn target_remove(
    cfg: &Cfg,
    targets: Vec<String>,
    toolchain: Option<PartialToolchainDesc>,
) -> Result<utils::ExitCode> {
    let distributable = DistributableToolchain::from_partial(toolchain, cfg)?;

    for target in targets {
        let target = TargetTriple::new(target);
        let default_target = cfg.get_default_host_triple()?;
        if target == default_target {
            warn!("after removing the default host target, proc-macros and build scripts might no longer build");
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
            warn!("after removing the last target, no build targets will be available");
        }
        let new_component = Component::new("rust-std".to_string(), Some(target), false);
        distributable.remove_component(new_component)?;
    }

    Ok(utils::ExitCode(0))
}

fn component_list(
    cfg: &Cfg,
    toolchain: Option<PartialToolchainDesc>,
    installed_only: bool,
) -> Result<utils::ExitCode> {
    // downcasting required because the toolchain files can name any toolchain
    let distributable = DistributableToolchain::from_partial(toolchain, cfg)?;
    common::list_items(distributable, |c| Some(&c.name), installed_only)?;
    Ok(utils::ExitCode(0))
}

fn component_add(
    cfg: &Cfg,
    components: Vec<String>,
    toolchain: Option<PartialToolchainDesc>,
    target: Option<String>,
) -> Result<utils::ExitCode> {
    let distributable = DistributableToolchain::from_partial(toolchain, cfg)?;
    let target = get_target(target, &distributable);

    for component in &components {
        let new_component = Component::try_new(component, &distributable, target.as_ref())?;
        distributable.add_component(new_component)?;
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

fn component_remove(
    cfg: &Cfg,
    components: Vec<String>,
    toolchain: Option<PartialToolchainDesc>,
    target: Option<String>,
) -> Result<utils::ExitCode> {
    let distributable = DistributableToolchain::from_partial(toolchain, cfg)?;
    let target = get_target(target, &distributable);

    for component in &components {
        let new_component = Component::try_new(component, &distributable, target.as_ref())?;
        distributable.remove_component(new_component)?;
    }

    Ok(utils::ExitCode(0))
}

fn toolchain_link(cfg: &Cfg, dest: &CustomToolchainName, src: &Path) -> Result<utils::ExitCode> {
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
            src: &utils::to_absolute(src)?,
            dest,
            cfg,
        }
        .install()?;
    } else {
        InstallMethod::Copy { src, dest, cfg }.install()?;
    }

    Ok(utils::ExitCode(0))
}

fn toolchain_remove(cfg: &mut Cfg, opts: UninstallOpts) -> Result<utils::ExitCode> {
    for toolchain_name in &opts.toolchain {
        let toolchain_name = toolchain_name.resolve(&cfg.get_default_host_triple()?)?;
        Toolchain::ensure_removed(cfg, (&toolchain_name).into())?;
    }
    Ok(utils::ExitCode(0))
}

fn override_add(
    cfg: &Cfg,
    toolchain: ResolvableToolchainName,
    path: Option<&Path>,
) -> Result<utils::ExitCode> {
    let toolchain_name = toolchain.resolve(&cfg.get_default_host_triple()?)?;

    let path = if let Some(path) = path {
        PathBuf::from(path)
    } else {
        utils::current_dir()?
    };

    match Toolchain::new(cfg, (&toolchain_name).into()) {
        Ok(_) => {}
        Err(e @ RustupError::ToolchainNotInstalled(_)) => match &toolchain_name {
            ToolchainName::Custom(_) => Err(e)?,
            ToolchainName::Official(desc) => {
                let status = DistributableToolchain::install(
                    cfg,
                    desc,
                    &[],
                    &[],
                    cfg.get_profile()?,
                    false,
                )?
                .0;
                writeln!(process().stdout().lock())?;
                common::show_channel_update(
                    cfg,
                    PackageUpdate::Toolchain(desc.clone()),
                    Ok(status),
                )?;
            }
        },
        Err(e) => Err(e)?,
    }

    cfg.make_override(&path, &toolchain_name)?;
    Ok(utils::ExitCode(0))
}

fn override_remove(cfg: &Cfg, path: Option<&Path>, nonexistent: bool) -> Result<utils::ExitCode> {
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
        vec![utils::current_dir()?]
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
            fn name(&self) -> Option<&'static str> {
                Some(self.path()?.rsplit_once('/')?.0)
            }

            fn path(&self) -> Option<&'static str> {
                $( if self.$ident { return Some($path); } )+
                None
            }
        }
    };
}

docs_data![
    // flags can be used to open specific documents, e.g. `rustup doc --nomicon`
    // tuple elements: document name used as flag, help message, document index path
    (alloc, "The Rust core allocation and collections library", "alloc/index.html"),
    (book, "The Rust Programming Language book", "book/index.html"),
    (cargo, "The Cargo Book", "cargo/index.html"),
    (core, "The Rust Core Library", "core/index.html"),
    (edition_guide, "The Rust Edition Guide", "edition-guide/index.html"),
    (nomicon, "The Dark Arts of Advanced and Unsafe Rust Programming", "nomicon/index.html"),

    #[arg(long = "proc_macro")]
    (proc_macro, "A support library for macro authors when defining new macros", "proc_macro/index.html"),

    (reference, "The Rust Reference", "reference/index.html"),
    (rust_by_example, "A collection of runnable examples that illustrate various Rust concepts and standard libraries", "rust-by-example/index.html"),
    (rustc, "The compiler for the Rust programming language", "rustc/index.html"),
    (rustdoc, "Documentation generator for Rust projects", "rustdoc/index.html"),
    (std, "Standard library API documentation", "std/index.html"),
    (test, "Support code for rustc's built in unit-test and micro-benchmarking framework", "test/index.html"),
    (unstable_book, "The Unstable Book", "unstable-book/index.html"),
    (embedded_book, "The Embedded Rust Book", "embedded-book/index.html"),
];

fn doc(
    cfg: &Cfg,
    path_only: bool,
    toolchain: Option<PartialToolchainDesc>,
    mut topic: Option<&str>,
    doc_page: &DocPage,
) -> Result<utils::ExitCode> {
    let toolchain = Toolchain::from_partial(toolchain, cfg)?;

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

    let topical_path: PathBuf;

    let doc_url = if let Some(topic) = topic {
        topical_path = topical_doc::local_path(&toolchain.doc_path("").unwrap(), topic)?;
        topical_path.to_str().unwrap()
    } else {
        topic = doc_page.name();
        doc_page.path().unwrap_or("index.html")
    };

    if path_only {
        let doc_path = toolchain.doc_path(doc_url)?;
        writeln!(process().stdout().lock(), "{}", doc_path.display())?;
        Ok(utils::ExitCode(0))
    } else {
        if let Some(name) = topic {
            writeln!(
                process().stderr().lock(),
                "Opening docs named `{name}` in your browser"
            )?;
        } else {
            writeln!(process().stderr().lock(), "Opening docs in your browser")?;
        }
        toolchain.open_docs(doc_url)?;
        Ok(utils::ExitCode(0))
    }
}

#[cfg(not(windows))]
fn man(
    cfg: &Cfg,
    command: &str,
    toolchain: Option<PartialToolchainDesc>,
) -> Result<utils::ExitCode> {
    use crate::currentprocess::varsource::VarSource;

    let toolchain = Toolchain::from_partial(toolchain, cfg)?;
    let mut path = toolchain.path().to_path_buf();
    path.push("share");
    path.push("man");
    utils::assert_is_directory(&path)?;

    let mut manpaths = std::ffi::OsString::from(path);
    manpaths.push(":"); // prepend to the default MANPATH list
    if let Some(path) = process().var_os("MANPATH") {
        manpaths.push(path);
    }
    std::process::Command::new("man")
        .env("MANPATH", manpaths)
        .arg(command)
        .status()
        .expect("failed to open man page");
    Ok(utils::ExitCode(0))
}

fn set_profile(cfg: &mut Cfg, profile: &str) -> Result<utils::ExitCode> {
    cfg.set_profile(profile)?;
    Ok(utils::ExitCode(0))
}

fn set_auto_self_update(cfg: &mut Cfg, auto_self_update_mode: &str) -> Result<utils::ExitCode> {
    if self_update::NEVER_SELF_UPDATE {
        let mut args = process().args_os();
        let arg0 = args.next().map(PathBuf::from);
        let arg0 = arg0
            .as_ref()
            .and_then(|a| a.to_str())
            .ok_or(CLIError::NoExeName)?;
        warn!("{} is built with the no-self-update feature: setting auto-self-update will not have any effect.",arg0);
    }
    cfg.set_auto_self_update(auto_self_update_mode)?;
    Ok(utils::ExitCode(0))
}

#[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
fn show_profile(cfg: &Cfg) -> Result<utils::ExitCode> {
    writeln!(process().stdout().lock(), "{}", cfg.get_profile()?)?;
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

fn output_completion_script(shell: Shell, command: CompletionCommand) -> Result<utils::ExitCode> {
    match command {
        CompletionCommand::Rustup => {
            clap_complete::generate(
                shell,
                &mut Rustup::command(),
                "rustup",
                &mut process().stdout().lock(),
            );
        }
        CompletionCommand::Cargo => {
            if let Shell::Zsh = shell {
                writeln!(process().stdout().lock(), "#compdef cargo")?;
            }

            let script = match shell {
                Shell::Bash => "/etc/bash_completion.d/cargo",
                Shell::Zsh => "/share/zsh/site-functions/_cargo",
                _ => {
                    return Err(anyhow!(
                        "{} does not currently support completions for {}",
                        command,
                        shell
                    ))
                }
            };

            writeln!(
                process().stdout().lock(),
                "if command -v rustc >/dev/null 2>&1; then\n\
                    \tsource \"$(rustc --print sysroot)\"{script}\n\
                 fi",
            )?;
        }
    }

    Ok(utils::ExitCode(0))
}
