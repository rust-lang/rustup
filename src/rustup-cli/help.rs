pub static RUSTUP_HELP: &'static str =
r"
rustup installs The Rust Programming Language from the official
release channels, enabling you to easily switch between stable, beta,
and nightly compilers and keep them updated. It makes cross-compiling
simpler with binary builds of the standard library for common platforms.

If you are new to Rust consider running `rustup doc --book`
to learn Rust.";

pub static SHOW_HELP: &'static str =
r"
Shows the name of the active toolchain and the version of `rustc`.

If the active toolchain has installed support for additional
compilation targets, then they are listed as well.

If there are multiple toolchains installed then all installed
toolchains are listed as well.";

pub static UPDATE_HELP: &'static str =
r"
With no toolchain specified, the `update` command updates each of the
installed toolchains from the official release channels, then updates
rustup itself.

If given a toolchain argument then `update` updates that toolchain,
the same as `rustup toolchain install`.

'toolchain' specifies a toolchain name, such as 'stable', 'nightly',
or '1.8.0'. For more information see `rustup help toolchain`.";

pub static TOOLCHAIN_INSTALL_HELP: &'static str =
r"
Installs a specific rust toolchain.

The 'install' command is an alias for 'rustup update <toolchain>'.

'toolchain' specifies a toolchain name, such as 'stable', 'nightly',
or '1.8.0'. For more information see `rustup help toolchain`.";

pub static DEFAULT_HELP: &'static str =
r"
Sets the default toolchain to the one specified. If the toolchain is
not already installed then it is installed first.";

pub static TOOLCHAIN_HELP: &'static str =
r"
Many `rustup` commands deal with *toolchains*, a single installation
of the Rust compiler. `rustup` supports multiple types of
toolchains. The most basic track the official release channels:
'stable', 'beta' and 'nightly'; but `rustup` can also install
toolchains from the official archives, for alternate host platforms,
and from local builds.

Standard release channel toolchain names have the following form:

    <channel>[-<date>][-<host>]

    <channel>       = stable|beta|nightly|<version>
    <date>          = YYYY-MM-DD
    <host>          = <target-triple>

'channel' is either a named release channel or an explicit version
number, such as '1.8.0'. Channel names can be optionally appended with
an archive date, as in 'nightly-2014-12-18', in which case the
toolchain is downloaded from the archive for that date.

Finally, the host may be specified as a target triple. This is most
useful for installing a 32-bit compiler on a 64-bit platform, or for
installing the [MSVC-based toolchain] on Windows. For example:

    rustup toolchain install stable-x86_64-pc-windows-msvc

For convenience, elements of the target triple that are omitted will be
inferred, so the above could be written:

    $ rustup default stable-msvc

Toolchain names that don't name a channel instead can be used to name
custom toolchains with the `rustup toolchain link` command.";

pub static OVERRIDE_HELP: &'static str =
r"
Overrides configure rustup to use a specific toolchain when
running in a specific directory.

Directories can be assigned their own Rust toolchain with
`rustup override`. When a directory has an override then
any time `rustc` or `cargo` is run inside that directory,
or one of its child directories, the override toolchain
will be invoked.

To pin to a specific nightly:

    rustup override set nightly-2014-12-18

Or a specific stable release:

    rustup override set 1.0.0

To see the active toolchain use `rustup show`. To remove the override
and use the default toolchain again, `rustup override unset`.";

pub static OVERRIDE_UNSET_HELP: &'static str =
r"
If `--path` argument is present, removes the override toolchain for
the specified directory. If `--nonexistent` argument is present, removes
the override toolchain for all nonexistent directories. Otherwise,
removes the override toolchain for the current directory.";

pub static RUN_HELP: &'static str =
r"
Configures an environment to use the given toolchain and then runs
the specified program. The command may be any program, not just
rustc or cargo. This can be used for testing arbitrary toolchains
without setting an override.

Commands explicitly proxied by `rustup` (such as `rustc` and `cargo`)
also have a shorthand for this available. The toolchain can be set by
using `+toolchain` as the first argument. These are equivalent:

    cargo +nightly build

    rustup run nightly cargo build";

pub static DOC_HELP: &'static str =
r"
Opens the documentation for the currently active toolchain with the
default browser.

By default, it opens the documentation index. Use the various flags to
open specific pieces of documentation.";

pub static COMPLETIONS_HELP: &'static str =
r"
One can generate a completion script for `rustup` that is compatible with
a given shell. The script is output on `stdout` allowing one to re-direct
the output to the file of their choosing. Where you place the file will
depend on which shell, and which operating system you are using. Your
particular configuration may also determine where these scripts need
to be placed.

Here are some common set ups for the three supported shells under
Unix and similar operating systems (such as GNU/Linux).

BASH:

Completion files are commonly stored in `/etc/bash_completion.d/`

Run the command:

`rustup completions bash > /etc/bash_completion.d/rustup.bash-completion`

This installs the completion script. You may have to log out and log
back in to your shell session for the changes to take affect.

FISH:

Fish completion files are commonly stored in
`$HOME/.config/fish/completions`

Run the command:
`rustup completions fish > ~/.config/fish/completions/rustup.fish`

This installs the completion script. You may have to log out and log
back in to your shell session for the changes to take affect.

ZSH:

ZSH completions are commonly stored in any directory listed in your
`$fpath` variable. To use these completions, you must either add the
generated script to one of those directories, or add your own
to this list.

Adding a custom directory is often the safest best if you're unsure
of which directory to use. First create the directory, for this
example we'll create a hidden directory inside our `$HOME` directory

`mkdir ~/.zfunc`

Then add the following lines to your `.zshrc` just before `compinit`

`fpath+=~/.zfunc`

Now you can install the completions script using the following command

`rustup completions zsh > ~/.zfunc/_rustup`

You must then either log out and log back in, or simply run

`exec zsh`

For the new completions to take affect.

CUSTOM LOCATIONS:

Alternatively, you could save these files to the place of your choosing,
such as a custom directory inside your $HOME. Doing so will require you
to add the proper directives, such as `source`ing inside your login
script. Consult your shells documentation for how to add such directives.

POWERSHELL:

The powershell completion scripts require PowerShell v5.0+ (which comes
Windows 10, but can be downloaded separately for windows 7 or 8.1).

First, check if a profile has already been set

`PS C:\> Test-Path $profile`

If the above command returns `False` run the following

`PS C:\> New-Item -path $profile -type file --force`

Now open the file provided by `$profile` (if you used the `New-Item` command
it will be `%USERPROFILE%\Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1`

Next, we either save the completions file into our profile, or into a separate file
and source it inside our profile. To save the completions into our profile simply
use

`PS C:\> rustup completions powershell >> %USERPROFILE%\Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1`";
