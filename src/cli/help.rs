use anstyle::Style;
use clap_cargo::style::{HEADER, LITERAL, PLACEHOLDER};

pub(crate) fn rustup_help() -> String {
    format!(
        r"{HEADER}Discussion:{HEADER:#}
  Rustup installs The Rust Programming Language from the official
  release channels, enabling you to easily switch between stable,
  beta, and nightly compilers and keep them updated. It makes
  cross-compiling simpler with binary builds of the standard library
  for common platforms.

  If you are new to Rust consider running `rustup doc --book` to
  learn Rust."
    )
}

pub(crate) fn show_help() -> String {
    format!(
        r"{HEADER}Discussion:{HEADER:#}
  Shows the name of the active toolchain and the version of `rustc`.

  If the active toolchain has installed support for additional
  compilation targets, then they are listed as well.

  If there are multiple toolchains installed then all installed
  toolchains are listed as well."
    )
}

pub(crate) fn show_active_toolchain_help() -> String {
    format!(
        r"{HEADER}Discussion:{HEADER:#}
  Shows the name of the active toolchain.

  This is useful for figuring out the active tool chain from
  scripts.

  You should use `rustc --print sysroot` to get the sysroot, or
  `rustc --version` to get the toolchain version."
    )
}

pub(crate) fn update_help() -> String {
    format!(
        r"{HEADER}Discussion:{HEADER:#}
  With no toolchain specified, the `update` command updates each of
  the installed toolchains from the official release channels, then
  updates rustup itself.

  If given a toolchain argument then `update` updates that
  toolchain, the same as `rustup toolchain install`.

{TOOLCHAIN_INSTALL_HINT}"
    )
}

pub(crate) fn install_help() -> String {
    format!(
        r"{HEADER}Discussion:{HEADER:#}
  The 'install' command is an alias for 'rustup toolchain install'."
    )
}

pub(crate) fn default_help() -> String {
    format!(
        r"{HEADER}Discussion:{HEADER:#}
  Sets the default toolchain to the one specified. If the toolchain
  is not already installed then it is installed first."
    )
}

pub(crate) fn toolchain_help() -> String {
    format!(
        r"{HEADER}Discussion:{HEADER:#}
  Many `rustup` commands deal with *toolchains*, a single
  installation of the Rust compiler. `rustup` supports multiple
  types of toolchains. The most basic track the official release
  channels: 'stable', 'beta' and 'nightly'; but `rustup` can also
  install specific toolchains from the official archives, toolchains for
  alternate host platforms, and from local builds ('custom toolchains').

  Standard release channel toolchain names have the following form:

    {PLACEHOLDER}<channel>[-<date>][-<host>]{PLACEHOLDER:#}

    {PLACEHOLDER}<channel>       = stable|beta|nightly|<versioned>[-<prerelease>]{PLACEHOLDER:#}
    {PLACEHOLDER}<versioned>     = <major.minor>|<major.minor.patch>{PLACEHOLDER:#}
    {PLACEHOLDER}<prerelease>    = beta[.<number>]{PLACEHOLDER:#}
    {PLACEHOLDER}<date>          = YYYY-MM-DD{PLACEHOLDER:#}
    {PLACEHOLDER}<host>          = <target-triple>{PLACEHOLDER:#}

  'channel' is a named release channel, a major and minor version
  number such as `1.42`, or a fully specified version number, such
  as `1.42.0`. Channel names can be optionally appended with an
  archive date, as in `nightly-2014-12-18`, in which case the
  toolchain is downloaded from the archive for that date.

  The host may be specified as a target triple. This is most useful
  for installing a 32-bit compiler on a 64-bit platform, or for
  installing the [MSVC-based toolchain] on Windows. For example:

    {LITERAL}$ rustup toolchain install stable-x86_64-pc-windows-msvc{LITERAL:#}

  For convenience, omitted elements of the target triple will be
  inferred, so the above could be written:

    {LITERAL}$ rustup toolchain install stable-msvc{LITERAL:#}

  The `rustup default` command may be used to both install and set
  the desired toolchain as default in a single command:

    {LITERAL}$ rustup default stable-msvc{LITERAL:#}

  rustup can also manage symlinked local toolchain builds, which are
  often used for developing Rust itself. For more information see
  `rustup toolchain help link`."
    )
}

pub(crate) fn toolchain_install_help() -> String {
    format!(
        r"{HEADER}Discussion:{HEADER:#}
{TOOLCHAIN_INSTALL_HINT}"
    )
}

pub(crate) fn toolchain_link_help() -> String {
    format!(
        r"{HEADER}Discussion:{HEADER:#}
  'toolchain' is the custom name to be assigned to the new toolchain.
  Any name is permitted as long as:
  - it does not include '/' or '\' except as the last character
  - it is not equal to 'none'
  - it does not fully match an initialsubstring of a standard release channel.
  For example, you can use the names 'latest' or '2017-04-01' but you cannot
  use 'stable' or 'beta-i686' or 'nightly-x86_64-unknown-linux-gnu'.

  'path' specifies the directory where the binaries and libraries for
  the custom toolchain can be found. For example, when used for
  development of Rust itself, toolchains can be linked directly out of
  the build directory. After building, you can test out different
  compiler versions as follows:

    {LITERAL}$ rustup toolchain link latest-stage1 build/x86_64-unknown-linux-gnu/stage1{LITERAL:#}
    {LITERAL}$ rustup override set latest-stage1{LITERAL:#}

  If you now compile a crate in the current directory, the custom
  toolchain 'latest-stage1' will be used."
    )
}

pub(crate) fn override_help() -> String {
    format!(
        r"{HEADER}Discussion:{HEADER:#}
  Overrides configure Rustup to use a specific toolchain when
  running in a specific directory.

  Directories can be assigned their own Rust toolchain with `rustup
  override`. When a directory has an override then any time `rustc`
  or `cargo` is run inside that directory, or one of its child
  directories, the override toolchain will be invoked.

  To pin to a specific nightly:

    {LITERAL}$ rustup override set nightly-2014-12-18{LITERAL:#}

  Or a specific stable release:

    {LITERAL}$ rustup override set 1.0.0{LITERAL:#}

  To see the active toolchain use `rustup show`. To remove the
  override and use the default toolchain again, `rustup override
  unset`."
    )
}

pub(crate) fn override_unset_help() -> String {
    format!(
        r"{HEADER}Discussion:{HEADER:#}
  If `--path` argument is present, removes the override toolchain
  for the specified directory. If `--nonexistent` argument is
  present, removes the override toolchain for all nonexistent
  directories. Otherwise, removes the override toolchain for the
  current directory."
    )
}

pub(crate) fn run_help() -> String {
    format!(
        r"{HEADER}Discussion:{HEADER:#}
  Configures an environment to use the given toolchain and then runs
  the specified program. The command may be any program, not just
  rustc or cargo. This can be used for testing arbitrary toolchains
  without setting an override.

  Commands explicitly proxied by `rustup` (such as `rustc` and
  `cargo`) also have a shorthand for this available. The toolchain
  can be set by using `+toolchain` as the first argument. These are
  equivalent:

    {LITERAL}$ cargo +nightly build{LITERAL:#}

    {LITERAL}$ rustup run nightly cargo build{LITERAL:#}"
    )
}

pub(crate) fn doc_help() -> String {
    format!(
        r"{HEADER}Discussion:{HEADER:#}
  Opens the documentation for the currently active toolchain with
  the default browser.

  By default, it opens the documentation index. Use the various
  flags to open specific pieces of documentation."
    )
}

pub(crate) fn completions_help() -> String {
    format!(
        r"{HEADER}Discussion:{HEADER:#}
  Enable tab completion for Bash, Fish, Zsh, or PowerShell
  The script is output on `stdout`, allowing one to re-direct the
  output to the file of their choosing. Where you place the file
  will depend on which shell, and which operating system you are
  using. Your particular configuration may also determine where
  these scripts need to be placed.

  Here are some common set ups for the three supported shells under
  Unix and similar operating systems (such as GNU/Linux).

  {SUBHEADER}Bash:{SUBHEADER:#}

  Completion files are commonly stored in `/etc/bash_completion.d/` for
  system-wide commands, but can be stored in
  `~/.local/share/bash-completion/completions` for user-specific commands.
  Run the command:

    {LITERAL}$ mkdir -p ~/.local/share/bash-completion/completions{LITERAL:#}
    {LITERAL}$ rustup completions bash > ~/.local/share/bash-completion/completions/rustup{LITERAL:#}

  This installs the completion script. You may have to log out and
  log back in to your shell session for the changes to take effect.

  {SUBHEADER}Bash (macOS/Homebrew):{SUBHEADER:#}

  Homebrew stores bash completion files within the Homebrew directory.
  With the `bash-completion` brew formula installed, run the command:

    {LITERAL}$ mkdir -p $(brew --prefix)/etc/bash_completion.d{LITERAL:#}
    {LITERAL}$ rustup completions bash > $(brew --prefix)/etc/bash_completion.d/rustup.bash-completion{LITERAL:#}

  {SUBHEADER}Fish:{SUBHEADER:#}

  Fish completion files are commonly stored in
  `$HOME/.config/fish/completions`. Run the command:

    {LITERAL}$ mkdir -p ~/.config/fish/completions{LITERAL:#}
    {LITERAL}$ rustup completions fish > ~/.config/fish/completions/rustup.fish{LITERAL:#}

  This installs the completion script. You may have to log out and
  log back in to your shell session for the changes to take effect.

  {SUBHEADER}Xonsh:{SUBHEADER:#}

  In Xonsh you can reuse Fish completion by installing `xontrib-fish-completer`.

  {SUBHEADER}Zsh:{SUBHEADER:#}

  ZSH completions are commonly stored in any directory listed in
  your `$fpath` variable. To use these completions, you must either
  add the generated script to one of those directories, or add your
  own to this list.

  Adding a custom directory is often the safest bet if you are
  unsure of which directory to use. First create the directory; for
  this example we'll create a hidden directory inside our `$HOME`
  directory:

    {LITERAL}$ mkdir ~/.zfunc{LITERAL:#}

  Then add the following lines to your `.zshrc` just before
  `compinit`:

    {LITERAL}fpath+=~/.zfunc{LITERAL:#}

  Now you can install the completions script using the following
  command:

    {LITERAL}$ rustup completions zsh > ~/.zfunc/_rustup{LITERAL:#}

  You must then either log out and log back in, or simply run

    {LITERAL}$ exec zsh{LITERAL:#}

  for the new completions to take effect.

  {SUBHEADER}Custom locations:{SUBHEADER:#}

  Alternatively, you could save these files to the place of your
  choosing, such as a custom directory inside your $HOME. Doing so
  will require you to add the proper directives, such as `source`ing
  inside your login script. Consult your shells documentation for
  how to add such directives.

  {SUBHEADER}PowerShell:{SUBHEADER:#}

  The powershell completion scripts require PowerShell v5.0+ (which
  comes with Windows 10, but can be downloaded separately for windows 7
  or 8.1).

  First, check if a profile has already been set

    {LITERAL}PS C:\> Test-Path $profile{LITERAL:#}

  If the above command returns `False` run the following

    {LITERAL}PS C:\> New-Item -path $profile -type file -force{LITERAL:#}

  Now open the file provided by `$profile` (if you used the
  `New-Item` command it will be
  `${{env:USERPROFILE}}\Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1`

  Next, we either save the completions file into our profile, or
  into a separate file and source it inside our profile. To save the
  completions into our profile simply use

    {LITERAL}PS C:\> rustup completions powershell >> ${{env:USERPROFILE}}\Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1{LITERAL:#}

  {SUBHEADER}Cargo:{SUBHEADER:#}

  Rustup can also generate a completion script for `cargo`. The script output
  by `rustup` will source the completion script distributed with your default
  toolchain. Not all shells are currently supported. Here are examples for
  the currently supported shells.

  {SUBHEADER}Bash:{SUBHEADER:#}

    {LITERAL}$ rustup completions bash cargo >> ~/.local/share/bash-completion/completions/cargo{LITERAL:#}

  {SUBHEADER}Zsh:{SUBHEADER:#}

    {LITERAL}$ rustup completions zsh cargo > ~/.zfunc/_cargo{LITERAL:#}"
    )
}

pub(crate) fn official_toolchain_arg_help() -> &'static str {
    "Toolchain name, such as 'stable', 'nightly', \
                                       or '1.8.0'. For more information see `rustup \
                                       help toolchain`"
}
pub(crate) fn resolvable_local_toolchain_arg_help() -> &'static str {
    "Toolchain name, such as 'stable', 'nightly', \
                                       '1.8.0', or a custom toolchain name, or an absolute path. For more \
                                       information see `rustup help toolchain`"
}
pub(crate) fn resolvable_toolchain_arg_help() -> &'static str {
    "Toolchain name, such as 'stable', 'nightly', \
                                       '1.8.0', or a custom toolchain name. For more information see `rustup \
                                       help toolchain`"
}
pub(crate) fn maybe_resolvable_toolchain_arg_help() -> &'static str {
    "'none', a toolchain name, such as 'stable', 'nightly', \
                                       '1.8.0', or a custom toolchain name. For more information see `rustup \
                                       help toolchain`"
}

pub(crate) fn topic_arg_help() -> &'static str {
    "Topic such as 'core', 'fn', 'usize', 'eprintln!', \
                                   'core::arch', 'alloc::format!', 'std::fs', \
                                   'std::fs::read_dir', 'std::io::Bytes', \
                                   'std::iter::Sum', 'std::io::error::Result' etc..."
}

const SUBHEADER: Style = Style::new().bold();

const TOOLCHAIN_INSTALL_HINT: &str =
    "  Some environment variables allow you to customize certain parameters
  in toolchain installation, including:

  - `RUSTUP_CONCURRENT_DOWNLOADS`: the number of concurrent downloads.
  - `RUSTUP_DOWNLOAD_TIMEOUT`: the download timeout in seconds.

  See <https://rust-lang.github.io/rustup/devel/environment-variables.html>
  for more info.";
