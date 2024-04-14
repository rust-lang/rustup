# Changelog

## [1.27.1] - 2024-04-14

This new Rustup release involves some minor bug fixes.

The headlines for this release are:

1. Prebuilt Rustup binaries should be working on older macOS versions again.
2. `rustup-init` will no longer fail when `fish` is installed but `~/.config/fish/conf.d` hasn't been created.
3. Regressions regarding symlinked `RUSTUP_HOME/(toolchains|downloads|tmp)` have been addressed.

Since the release of 1.27.0, the project has welcomed a total of 7 new contributors.
Many thanks for your hard work, and we hope to see you again!

### Added

- Added the logging of `RUSTUP_UPDATE_ROOT` and `RUSTUP_DIST_(SERVER|ROOT)` when `RUSTUP_DEBUG` is set [pr#3722]

### Changed

- Ensured that CI builds target macOS 10.12+ on x64 and macOS 11.0+ on arm64 [pr#3710]
- Fixed an error when writing to rcfiles by ensuring the creation of their parent dir(s) first [pr#3712]
- Fixed unexpected errors when `RUSTUP_HOME/(toolchains|downloads|tmp)` is symlinked [pr#3742] [pr#3754]
- Fixed an unexpected error when removing a component by component name with explicit target triple [pr#3601]
- Changed `RUSTUP_WINDOWS_PATH_ADD_BIN` to be false by default [pr#3703]
- Fixed incorrect color state after `ColorableTerminal::reset` [pr#3711]
- Replaced `.` with `source` in fish shell's `source_string` [pr#3715]
- Fixed "component add" error message format [pr#3724]
- Fixed file paths in CI-generated `*.sha256` files on *nix [pr#3730]
- Removed an unnecessary debug print [pr#3734]
- Disabled the "doc opening" output on `rustup doc --path` [pr#3748]
- Fixed the update of `DisplayVersion` in the Windows registry on `rustup self update` [pr#3770]
- Bumped a lot of dependencies to their latest versions [pr#renovate-bot]

Thanks go to:

- Anas (0x61nas)
- cuiyourong (cuiyourong)
- Dirkjan Ochtman (djc)
- Eric Huss (ehuss)
- eth3lbert (eth3lbert)
- hev (heiher)
- klensy (klensy)
- Chih Wang (ongchi)
- Adam (pie-flavor)
- rami3l (rami3l)
- Robert (rben01)
- Robert Collins (rbtcollins)
- Sun Bin (shandongbinzhou)
- Samuel Moelius (smoelius)
- vpochapuis (vpochapuis)
- Renovate Bot (renovate)

**Full Changelog**: https://github.com/rust-lang/rustup/compare/1.27.0...1.27.1

[1.27.1]: https://github.com/rust-lang/rustup/releases/tag/1.27.1
[pr#3601]: https://github.com/rust-lang/rustup/pull/3601
[pr#3703]: https://github.com/rust-lang/rustup/pull/3703
[pr#3711]: https://github.com/rust-lang/rustup/pull/3711
[pr#3715]: https://github.com/rust-lang/rustup/pull/3715
[pr#3710]: https://github.com/rust-lang/rustup/pull/3710
[pr#3712]: https://github.com/rust-lang/rustup/pull/3712
[pr#3722]: https://github.com/rust-lang/rustup/pull/3722
[pr#3724]: https://github.com/rust-lang/rustup/pull/3724
[pr#3730]: https://github.com/rust-lang/rustup/pull/3730
[pr#3734]: https://github.com/rust-lang/rustup/pull/3734
[pr#3748]: https://github.com/rust-lang/rustup/pull/3748
[pr#3742]: https://github.com/rust-lang/rustup/pull/3742
[pr#3754]: https://github.com/rust-lang/rustup/pull/3754
[pr#3770]: https://github.com/rust-lang/rustup/pull/3770

## [1.27.0] - 2024-03-08

This long-awaited Rustup release has gathered all the new features and fixes since April 2023.
These changes include improvements in Rustup's maintainability, user experience, compatibility and documentation quality.

The headlines of this release are:
- Basic support for `fish` shell has been added.
- Support for the `loongarch64-unknown-linux-gnu` host platform has been added.

Also, it's worth mentioning that Dirkjan Ochtman and rami3l have joined the team and are coordinating this new release.

Finally, the project seems to have attracted a total of 23 new contributors within this release cycle. Looking forward to seeing you again in the future!

### Added

- Add basic support for `fish` shell [pr#3108] 
- Add the `RUSTUP_TERM_COLOR` environment variable to force the use of colored output [pr#3435]
- Improve `rustup-init.sh`'s compatibility with `ksh` and `zsh` [pr#3475]
- Add a warning when running under Rosetta 2 [pr#3068]
- Add browser detection for RISC-V 64 platform [pr#3642]
- Add a warning when removing the last/host target for a toolchain [pr#3637]

### Changed

- Upgrade `clap` to v4 [pr#3444]
- Fix incorrect platform detection on macOS aarch64 due to Rosetta 2 [pr#3438]
- Fix incorrect platform detection on 32-bit Linux userland with a 64-bit kernel [pr#3488] [pr#3490]
- Improve Windows system32 DLL loading mechanism [pr#3493]
- Improve suggestions about missing components [pr#3453]
- Fix handling of toolchain names with special characters [pr#3518]
- Fix panic in `component list --toolchain stable` [pr#3548]
- Rename `llvm-tools-preview` component to `llvm-tools` [pr#3578]
- Try using symlinks if possible on Windows [pr#3687]
- Bump a lot of dependencies to their latest versions [pr#renovate-bot]

Thanks go to:

- Anthony Perkins (acperkins)
- Tianqi (airstone42)
- Alex Gaynor (alex)
- Alex Hudspith (alexhudspith)
- Alan Somers (asomers)
- Brett (brettearle)
- Burak Emir (burakemir)
- Chris Denton (ChrisDenton)
- cui fliter (cuishuang)
- Dirkjan Ochtman (djc)
- Dezhi Wu (dzvon)
- Eric Swanson (ericswanson-dfinity)
- Prikshit Gautam (gautamprikshit1)
- hev (heiher)
- 二手掉包工程师 (hi-rustin)
- Kamila Borowska (KamilaBorowska)
- klensy (klensy)
- Jakub Beránek (Kobzol)
- Kornel (kornelski)
- Matt Harding (majaha)
- Mathias Brossard (mbrossard)
- Christian Thackston (nan60)
- Ruohui Wang (noirgif)
- Olivier Lemasle (olivierlemasle)
- Chih Wang (ongchi)
- Pavel Roskin (proski)
- rami3l (rami3l)
- Robert Collins (rbtcollins)
- Sandesh  Pyakurel (Sandesh-Pyakurel)
- Waffle Maybe (WaffleLapkin)
- Jubilee (workingjubilee)
- WÁNG Xuěruì (xen0n)
- Yerkebulan Tulibergenov (yerke)
- Renovate Bot (renovate)

**Full Changelog**: https://github.com/rust-lang/rustup/compare/1.26.0...1.27.0

[1.27.0]: https://github.com/rust-lang/rustup/releases/tag/1.27.0
[pr#3108]: https://github.com/rust-lang/rustup/pull/3108
[pr#3435]: https://github.com/rust-lang/rustup/pull/3435
[pr#3475]: https://github.com/rust-lang/rustup/pull/3475
[pr#3068]: https://github.com/rust-lang/rustup/pull/3068
[pr#3642]: https://github.com/rust-lang/rustup/pull/3642
[pr#3637]: https://github.com/rust-lang/rustup/pull/3637
[pr#3444]: https://github.com/rust-lang/rustup/pull/3444
[pr#3438]: https://github.com/rust-lang/rustup/pull/3438
[pr#3488]: https://github.com/rust-lang/rustup/pull/3488
[pr#3490]: https://github.com/rust-lang/rustup/pull/3490
[pr#3493]: https://github.com/rust-lang/rustup/pull/3493
[pr#3453]: https://github.com/rust-lang/rustup/pull/3453
[pr#3518]: https://github.com/rust-lang/rustup/pull/3518
[pr#3548]: https://github.com/rust-lang/rustup/pull/3548
[pr#3578]: https://github.com/rust-lang/rustup/pull/3578
[pr#3687]: https://github.com/rust-lang/rustup/pull/3687

## [1.26.0] - 2023-04-05

This version of Rustup involves a significant number of internal refactors, both in terms
of the Rustup code and its tests.

The headlines for this release are:

1. Add [rust-analyzer] as a [proxy] of rustup. Now you can call `rust-analyzer` and it will
   be proxied to the rust-analyzer component for the current toolchain.
2. Bump the [clap] dependency from 2.x to 3.x. It's a major version bump, so there are some
   help text changes, but the command line interface is unchanged.
3. Remove experimental GPG signature validation and the `rustup show keys` command. Due to its
   experimental status, validating the integrity of downloaded binaries did not rely on it, and there was no option to abort the installation if a signature mismatch happened.
   Multiple problems with its implementation were discovered in the recent months, which led to the decision to remove the experimental code. The team is working on the design of a new signature validation scheme, which will be implemented in the future.

In addition to a lot of work on the codebase itself, due to the length of time since the last
release this one has a record number of contributors and we thank you all for your efforts and time.

Rather than list every single merged PR since the last release, we have pulled out
a number of highlights to include in this changelog entry. For everything else,
please review the repository.

### Added

- Added `rust-analyzer` as a proxy of rustup [pr#3022]
- Added DisplayVersion for rustup to registry on Windows [pr#3047]
- Build Rustup for Windows arm64 on stable [pr#3232]
- Added `up` as an alias of the `update` command [pr#3044]
- Added details of each setting in the toolchain file in the documentation [pr#3067]
- Added automatic resume flag when retrying download with curl [pr#3089]
- Added UI tests for rustup [pr#3209]

### Changed

- Bump the `clap` dependency from 2.x to 3.x [pr#3064]
- Remove GPG signature support [pr#3277]
- Don't add toolchain bin to PATH on Windows [pr#3178]
- Remove use of hard links to symlinks on macOS [pr#3137]
- Avoid deduplicate PATH entries added during build [pr#2848]
- The toolchain name cannot be left blank [pr#2993]
- Correctly propagate subshell failures in rustup-init.sh [pr#3012]
- Enhanced warning message for Rust installation already present [pr#3038]
- Improved error message when there is an error caused by override file [pr#3041]
- Explain [proxy] in terminology documentation [pr#3091]
- Recommend tracking `Cargo.lock` with rust-toolchain file [pr#3054]
- Fix RUSTUP_PERMIT_COPY_RENAME condition so it is actually used [pr#3292]
- Bump a lot of dependencies to their latest versions [pr#renovate-bot]

[1.26.0]: https://github.com/rust-lang/rustup/releases/tag/1.26.0
[rust-analyzer]: https://github.com/rust-lang/rust-analyzer
[proxy]: https://rust-lang.github.io/rustup/concepts/proxies.html
[clap]: https://crates.io/crates/clap
[pr#3022]: https://github.com/rust-lang/rustup/pull/3022
[pr#3047]: https://github.com/rust-lang/rustup/pull/3047
[pr#3232]: https://github.com/rust-lang/rustup/pull/3232
[pr#3044]: https://github.com/rust-lang/rustup/pull/3044
[pr#3067]: https://github.com/rust-lang/rustup/pull/3067
[pr#3089]: https://github.com/rust-lang/rustup/pull/3089
[pr#3209]: https://github.com/rust-lang/rustup/pull/3209
[pr#3064]: https://github.com/rust-lang/rustup/pull/3064
[pr#3277]: https://github.com/rust-lang/rustup/pull/3277
[pr#3178]: https://github.com/rust-lang/rustup/pull/3178
[pr#3137]: https://github.com/rust-lang/rustup/pull/3137
[pr#2848]: https://github.com/rust-lang/rustup/pull/2848
[pr#2993]: https://github.com/rust-lang/rustup/pull/2993
[pr#3012]: https://github.com/rust-lang/rustup/pull/3012
[pr#3038]: https://github.com/rust-lang/rustup/pull/3038
[pr#3041]: https://github.com/rust-lang/rustup/pull/3041
[pr#3091]: https://github.com/rust-lang/rustup/pull/3091
[pr#3054]: https://github.com/rust-lang/rustup/pull/3054
[pr#3292]: https://github.com/rust-lang/rustup/pull/3292
[pr#renovate-bot]: https://github.com/rust-lang/rustup/pulls?q=is:pr+author:app/renovate+is:merged

Thanks go to:

- Daniel Silverstone (kinnison)
- Sabrina Jewson (SabrinaJewson)
- Robert Collins (rbtcollins)
- chansuke (chansuke)
- Shamil (shamilsan)
- Oli Lalonde (olalonde)
- 二手掉包工程师 (hi-rustin)
- Eric Huss (ehuss)
- J Balint BIRO (jbalintbiro)
- Easton Pillay (jedieaston)
- zhaixiaojuan (zhaixiaojuan)
- Chris Denton (ChrisDenton)
- Martin Geisler (mgeisler)
- Lucio Franco (LucioFranco)
- Nicholas Bishop (nicholasbishop)
- SADIK KUZU (sadikkuzu)
- darkyshiny (darkyshiny)
- René Dudfield (illume)
- Noritada Kobayashi (noritada)
- Mohammad AlSaleh (MoSal)
- Dustin Martin (dmartin)
- Ville Skyttä (scop)
- Tshepang Mbambo (tshepang)
- Illia Bobyr (ilya-bobyr)
- Vincent Rischmann (vrischmann)
- Alexander (Alovchin91)
- Daniel Brotsky (brotskydotcom)
- zohnannor (zohnannor)
- Joshua Nelson (jyn514)
- Prikshit Gautam (gautamprikshit1)
- Dylan Thacker-Smith (dylanahsmith)
- Jan David (jdno)
- Aurora (lilith13666)
- Pietro Albini (pietroalbini)
- Renovate Bot (renovate-bot)

## [1.25.2] - 2023-02-01

This version of Rustup changes the signature verification code to continue
accepting Rust's release signature key, which previously caused warnings due to
a time-based check.

Note that signature verification in Rustup is still an experimental feature,
and there is intentionally no way to enforce signature verification due to the
feature being incomplete.

Thanks go to:

- Pietro Albini
- Daniel Silverstone

[1.25.2]: https://github.com/rust-lang/rustup/releases/tag/1.25.2

## [1.25.1] - 2022-07-12

This version of Rustup reverts a single PR from 1.25.1 and tidies a couple of internal
bits of code.

In brief, it turns out that our optimisation for `RUSTC` and `RUSTDOC` cause problems
with some tooling which runs under one `cargo` invocation, but expects to invoke either
`cargo` or `rustc` without resetting the environment completely. As such, some particularly
confusing error messages ensued, and we decided to revert this one optimisation while we
wait to correct things in a future release.

Thanks go to:

- Joshua Nelson
- Manish Goregaokar
- Robert Collins

[1.25.1]: https://github.com/rust-lang/rustup/releases/tag/1.25.1

## [1.25.0] - 2022-07-11

This version of Rustup involves a significant number of internal cleanups, both in terms
of the Rustup code and its documentation. In addition to a lot of work on the codebase
itself, due to the length of time since the last release this one has a record number
of contributors and we thank you all for your efforts and time.

Rather than list every single merged PR since the last release, we have pulled out
a number of highlights to include in this changelog entry. For everything else,
please review the repository.

### Added

- Added `rust-gdbgui` to the proxy list [pr#2811]
- Support `rustup default none` as a way to unset the default toolchain [pr#2831]
- Build Rustup for Windows arm64 [pr#2835]
- Support Illumos/OpenIndiana platform check on website [pr#2839]
- Add info message if self-update is disabled during update [pr#2845]
- Added `RUSTC` and `RUSTDOC` environment variables for proxied child processes [pr#2958]
- Added offer to auto-install VS 2022 [pr#2954]
- Added `--verbose` mode for `rustup show` [pr#2992]
- Added support for `--force-non-host` to more subcommands [pr#2968]

### Changed

- Updated the `opener` crate used for `rustup-doc` [pr#2792]
- Changed the recursion limit for tool/proxy invocation to 20 [pr#2812]
- Updated to newer `effective-limits` crate to reduce "sysinfo not supported" errors [pr#2817]
- Handle `-y` more robustly in `rustup-init.sh` [pr#2815]
- Fix infinite recursion in bash completion when rustc not on PATH [pr#2833]
- Update macOS aarch64 CI to newer xcode [pr#2877]
- Update website to load TTF fonts more effectively [pr#2862]
- Retry curl invocations in `rustup-init.sh` [pr#2869]
- Better handle busybox's wget in `rustup-init.sh` [pr#2885]
- Improve target matching to reduce spurious deprecation warnings [pr#2854]
- Parse channel manifest only once to improve performance [pr#2898]
- Remove trailing slashes from toolchain names [pr#2897]
- Migrate OpenPGP support to Sequoia PGP [pr#2847]
- We now send a user agent on http requests to improve compatibility with proxies [pr#2953]
- We won't prepend `${CARGO_HOME}/bin` to `PATH` unless it's missing [pr#2978]

Thanks go to:

- 二手掉包工程师 (hi-rustin)
- Brian Bowman (Seeker14491)
- Jon Gjengset (jonho)
- pierwill
- Daniel Silverstone (kinnison)
- Robert Collins (rbtcollins)
- Alan Somers (asomers)
- Brennan Vincent (umanwizard)
- Joshua Nelson (jyn514)
- Eric Huss (ehuss)
- Will Bush (willbush)
- Thad Guidry (thadguidry)
- Alexander Lovchin (alovchin91)
- zoodirector
- Takayuki Nakata (giraffate)
- Yusuke Abe (chansuke)
- Wyatt Carss (wcarss)
- Sondre Aasemoen (sondr3)
- facklambda
- Chad Dougherty (crd477)
- Noritada Kobayashi (noritada)
- Milan (mdaverde)
- Pat Sier (pjsier)
- Matt Keeter (mkeeter)
- Alex Macleod (alexendoo)
- Sathwik Matsa (sathwikmatsa)
- Kushal Das (kushaldas)
- Justus Winter (teythoon)
- k900
- Nicolas Ambram (nico-abram)
- Connor Slade (basicprogrammer10)
- Yerkebulan Tulibergenov (yerke)
- Caleb Cartwright (calebcartwright)
- Matthias Beyer (matthiasbeyer)
- spacemaniac
- Alex Touchet (atouchet)
- Guillaume Gomez (guillaumegomez)
- Chris Denton (chrisdenton)
- Thomas Orozco (krallin)
- cui fliter (cuishuang)
- Martin Nordholts (enselic)
- Emil Gardström (emilgardis)
- Arlo Siemsen (arlosi)

[1.25.0]: https://github.com/rust-lang/rustup/releases/tag/1.25.0
[pr#2968]: https://github.com/rust-lang/rustup/pull/2968
[pr#2992]: https://github.com/rust-lang/rustup/pull/2992
[pr#2978]: https://github.com/rust-lang/rustup/pull/2978
[pr#2954]: https://github.com/rust-lang/rustup/pull/2954
[pr#2958]: https://github.com/rust-lang/rustup/pull/2958
[pr#2953]: https://github.com/rust-lang/rustup/pull/2953
[pr#2847]: https://github.com/rust-lang/rustup/pull/2847
[pr#2845]: https://github.com/rust-lang/rustup/pull/2845
[pr#2897]: https://github.com/rust-lang/rustup/pull/2897
[pr#2898]: https://github.com/rust-lang/rustup/pull/2898
[pr#2854]: https://github.com/rust-lang/rustup/pull/2854
[pr#2839]: https://github.com/rust-lang/rustup/pull/2839
[pr#2885]: https://github.com/rust-lang/rustup/pull/2885
[pr#2869]: https://github.com/rust-lang/rustup/pull/2869
[pr#2862]: https://github.com/rust-lang/rustup/pull/2862
[pr#2877]: https://github.com/rust-lang/rustup/pull/2877
[pr#2835]: https://github.com/rust-lang/rustup/pull/2835
[pr#2831]: https://github.com/rust-lang/rustup/pull/2831
[pr#2811]: https://github.com/rust-lang/rustup/pull/2811
[pr#2833]: https://github.com/rust-lang/rustup/pull/2833
[pr#2815]: https://github.com/rust-lang/rustup/pull/2815
[pr#2817]: https://github.com/rust-lang/rustup/pull/2817
[pr#2812]: https://github.com/rust-lang/rustup/pull/2812
[pr#2792]: https://github.com/rust-lang/rustup/pull/2792

## [1.24.3] - 2021-05-31

This patch release focuses around resolving some regressions in behaviour in
the 1.24.x series. One problem, related to accounting for the release of data
blocks in the unpack slab allocator, fixed in [pr#2779], would manifest in the
installer [hanging during installation][issue#2774]. A second, fixed in
[pr#2781], manifested in very early Rust versions (1.0 through 1.7) [repeatedly
having their checksums fetched][issue#2777] despite already being installed.
Finally the heuristic which started warning that toolchains being installed may
not work on the given host was improved in [pr#2782] to reduce false-positive
rate and reduce worry among Windows users in particular.

### Added

- Added the ability to configure the auto-self-update functionality. This will
  be of most use when people are testing unreleased versions of Rustup and wish
  to ensure they don't accidentally lose the test version, without having to
  remember to run with `--no-self-update` all the time. [pr#2763]

### Changed

- We no longer delete the top level of `$RUSTUP_HOME/tmp` and
  `$RUSTUP_HOME/download` meaning that if you have these set up as symlinks to
  another place, or bind mounts, etc. things should work. [pr#2433]
- We more gracefully handle outlier situations with unpack-RAM, panicing less
  often, clamping settings into viable ranges and warning instead. [pr#2780]

Thanks go to:

- Ian Jackson
- Alexander (asv7c2)
- pierwill
- 二手掉包工程师 (hi-rustin)
- Robert Collins
- Daniel Silverstone

[1.24.3]: https://github.com/rust-lang/rustup/releases/tag/1.24.3
[issue#2777]: https://github.com/rust-lang/rustup/issues/2777
[issue#2774]: https://github.com/rust-lang/rustup/issues/2774
[pr#2782]: https://github.com/rust-lang/rustup/pull/2782
[pr#2780]: https://github.com/rust-lang/rustup/pull/2780
[pr#2781]: https://github.com/rust-lang/rustup/pull/2781
[pr#2779]: https://github.com/rust-lang/rustup/pull/2779
[pr#2763]: https://github.com/rust-lang/rustup/pull/2763
[pr#2433]: https://github.com/rust-lang/rustup/pull/2433

## [1.24.2] - 2021-05-05

This patch release primarily exists to work around a
[problem discovered][issue#2748] on some Windows (and potentially other) systems
where a combination of factors, including suspected allocator behaviour, led to
Rustup failing to install certain toolchains. The symptom users observed was a
failure to allocate 1677732 bytes: a chunk used for unpacking very large files.
We hope this is fixed in a combination of [pr#2750][] and [pr#2756][].

In addition to that, we also:

### Added

- SHA256 links on the download page so that you can verify your downloads if you
  want to be certain. [pr#2719][]
- Added `--verbose` to `rustup show active-toolchain` to also display the version
  of the compiler for the toolchain. [pr#2710]
- We now support `1.x` installation channel names for versions 1.0 through 1.8
  by hardcoding `1.x.0` since they lack patch releases. [pr#2758][]

### Changed

- Amended the behaviour of the 'missing components' code so that if the problem
  exists when _installing_ a toolchain (rather than updating it) the message
  is different and leads you to other remediations. [pr#2709][]
- Amended the error message for a missing component so that when you're using
  a nightly toolchain and `rust-std` is missing for a given target, we lead you
  to `cargo build -Z build-std` as a remediation. [pr#2732][]
- Improved the documentation around `settings.toml` locations. [pr#2698][]
- Internal improvements around retrying removal of files. [pr#2752][]

Thanks go to:

- 二手掉包工程师 (hi-rustin)
- Robert Collins
- Daniel Silverstone
- Joshua Nelson
- João Marcos Bezerra
- Carol (Nichols || Goulding)
- Josh Rotenberg
- Martijn Gribnau
- pierwill

[issue#2748]: https://github.com/rust-lang/rustup/issues/2748
[pr#2753]: https://github.com/rust-lang/rustup/pull/2753
[pr#2756]: https://github.com/rust-lang/rustup/pull/2756
[pr#2752]: https://github.com/rust-lang/rustup/pull/2752
[pr#2758]: https://github.com/rust-lang/rustup/pull/2758
[pr#2698]: https://github.com/rust-lang/rustup/pull/2698
[pr#2750]: https://github.com/rust-lang/rustup/pull/2750
[pr#2732]: https://github.com/rust-lang/rustup/pull/2732
[pr#2710]: https://github.com/rust-lang/rustup/pull/2710
[pr#2709]: https://github.com/rust-lang/rustup/pull/2709
[pr#2719]: https://github.com/rust-lang/rustup/pull/2719
[1.24.2]: https://github.com/rust-lang/rustup/releases/tag/1.24.2

## [1.24.1] - 2021-04-27

This bugfix release [corrects an oversight][pr#2738] in the code we introduced to check for
unknown proxy names. The original change accidentally omitted the `rustfmt` and
`cargo-fmt` proxies due to a quirk of the fact those proxies were not originally
part of a Rust component.

We're sorry for pain this may have caused.

[pr#2738]: https://github.com/rust-lang/rustup/pull/2738
[1.24.1]: https://github.com/rust-lang/rustup/releases/tag/1.24.1

## [1.24.0] - 2021-04-27

This release is mostly a bugfix and quality of life improvement release. However
the headlines for this release are:

1. Support of `rust-toolchain.toml` as a filename for specifying toolchains.
2. Streaming support for large files to better enable Rust on lower memory
   platforms such as some Raspberry Pi systems.

When we introduced TOML support to `rust-toolchain` we expected to see some
uptake but we saw a lot more than we had expected. Since Cargo is migrating to
explicit `.toml` extensions on things like `.cargo/config.toml` it was
considered sensible to also do this for `rust-toolchain` - at least the `toml`
variant thereof.

This release of `rustup` has seen a significant number of new contributors to
the project, and we hope to see many of you again in the future.

### Added

- Optional use of RUSTLS as TLS backend for Reqwest [pr#2517][]
- We now support some corner cases in tarballs to permit unpacking early Rust
  versions [pr#2502][]
- When running `rustup check` we now report possible `rustup` upgrades too.
  [pr#2615][]
- We detect and warn if you try and install on an `x32` system since for now
  Rust isn't hostable on that. [pr#2622][]
- We do, however, support `gnux32` as an environment label ready for future
  support [pr#2631][]
- We now support managing `PATH`s on Windows which contain non-unicode values.
  [pr#2649][]
- You can now name the TOML variant of `rust-toolchain` as `rust-toolchain.toml`
  [pr#2653][]
- We prompt harder when checking for the MSVC tooling on Windows now.
  [pr#2529][]
- _Experimental_ support for `zstd` compressed tarballs in channels. NOTE, this
  does not mean channels will magically gain `zstd` compressed component files
  any time soon. [pr#2676][]
- Register `rustup` with the Windows installed programs list when installing.
  This is another experiment into whether this is useful for Windows users.
  [pr#2670][]
- Added the ability to specify a `path` rather than a toolchain channel in the
  `rust-toolchain.toml` file. [pr#2678][]

### Changed

- `rustup-init` now detects tls1.2 for cURL 7.73+ [pr#2604][]
- Installation now indicates the defaults on all questions [pr#2605][]
- We now support the Big Sur major OS version [pr#2607][]
- You can now specify `profile` in `rust-toolchain`'s TOML form [pr#2586][]
- We now use `.` instead of `source` to better support non-bash POSIX shells
  [pr#2616][]
- We fixed a nasty corner case on wildcarded component installation/recognition
  [pr#2602][]
- Our website now has a favicon [pr#2419][]
- We no longer rely on a broken `mktemp` invocation, this should make
  `rustup-init.sh` more compatible [pr#2650][]
- We now do a better job of reporting non-installable toolchains [pr#2562][]
- We cope better when modifying RC files which lack a trailing newline
  [pr#2667][]
- We are edging closer to requiring a specific force argument to install a
  toolchain whose host doesn't match the running system. This may break your
  CI in future so you should check carefully. The main use-case for this
  capability is the `rust-embedded/cross` project which we are working with
  to ensure this doesn't cause problems in the future. [pr#2672][]
- Support streaming large files during unpack phase. [pr#2707][]
- We report when you call `rustup` with an unsupported `arg0` -- for example
  if you make a symlink or hard link to the binary with a name other than one
  of the proxies. [pr#2716][]

We also cleaned up a number of error message cases, including some on invalid
toolchain name [pr#2613][], a better message when no toolchain is installed
[pr#2657][], and some on component unavailability [pr#2619][].

### Documented

- Added notes about Powershell to proxies documentation [pr#2592][]
- Various updates to the `rustup` manual build process including [pr#2628][]
- Small fixes on how to build `rustup` documentation [pr#2641][]
- We clarified the message around restarting the shell when installing
  [pr#2684][]

Thanks go to:

- SHA Miao
- est31
- Andrew Norton
- Gareth Hubball
- 二手掉包工程师 (hi-rustin)
- Tudor Brindus
- Eduard Miller
- Daniel Alley
- наб (nabijaczleweli)
- Eric Huss
- chansuke
- skim (sl4m)
- Joshua Nelson
- kellda
- Alex Chan
- Philipp Oppermann
- Michael Cooper
- Aloïs Micard
- Gurkenglas
- Vasili (3point2)
- Jakub Stasiak
- Robert Collins
- Jubilee (workingjubilee)
- Avery Harnish

[pr#2716]: https://github.com/rust-lang/rustup/pull/2716
[pr#2718]: https://github.com/rust-lang/rustup/pull/2718
[pr#2707]: https://github.com/rust-lang/rustup/pull/2707
[pr#2678]: https://github.com/rust-lang/rustup/pull/2678
[pr#2695]: https://github.com/rust-lang/rustup/pull/2695
[pr#2670]: https://github.com/rust-lang/rustup/pull/2670
[pr#2684]: https://github.com/rust-lang/rustup/pull/2684
[pr#2676]: https://github.com/rust-lang/rustup/pull/2676
[pr#2529]: https://github.com/rust-lang/rustup/pull/2529
[pr#2667]: https://github.com/rust-lang/rustup/pull/2667
[pr#2653]: https://github.com/rust-lang/rustup/pull/2653
[pr#2657]: https://github.com/rust-lang/rustup/pull/2657
[pr#2562]: https://github.com/rust-lang/rustup/pull/2562
[pr#2649]: https://github.com/rust-lang/rustup/pull/2649
[pr#2650]: https://github.com/rust-lang/rustup/pull/2650
[pr#2641]: https://github.com/rust-lang/rustup/pull/2641
[pr#2419]: https://github.com/rust-lang/rustup/pull/2419
[pr#2631]: https://github.com/rust-lang/rustup/pull/2631
[pr#2628]: https://github.com/rust-lang/rustup/pull/2628
[pr#2622]: https://github.com/rust-lang/rustup/pull/2622
[pr#2619]: https://github.com/rust-lang/rustup/pull/2619
[pr#2615]: https://github.com/rust-lang/rustup/pull/2615
[pr#2602]: https://github.com/rust-lang/rustup/pull/2602
[pr#2502]: https://github.com/rust-lang/rustup/pull/2502
[pr#2616]: https://github.com/rust-lang/rustup/pull/2616
[pr#2613]: https://github.com/rust-lang/rustup/pull/2613
[pr#2586]: https://github.com/rust-lang/rustup/pull/2586
[pr#2607]: https://github.com/rust-lang/rustup/pull/2607
[pr#2605]: https://github.com/rust-lang/rustup/pull/2605
[pr#2604]: https://github.com/rust-lang/rustup/pull/2604
[pr#2517]: https://github.com/rust-lang/rustup/pull/2517
[pr#2592]: https://github.com/rust-lang/rustup/pull/2592
[1.24.0]: https://github.com/rust-lang/rustup/releases/tag/1.24.0

## [1.23.1] - 2020-12-01

This point release is mostly to correct a problem where if you installed
`rustup` with `--no-modify-path` then the `.cargo/env` file would not be created
in some cases. In addition, we have rebuilt the macos binaries to correct an
oversight which caused older Macs to be unable to run the new version. If you
encountered a problem with `liblzma` on mac os 10.13 then this version should
solve that for you.

Finally, the illumos binary is now part of the release properly.

Thanks go to:

- Élie Roudninski
- Jeroen Ooms
- Jake Goulding
- Joshua M. Clulow
- Neil Mitchell
- Richard Gomes

[1.23.1]: https://github.com/rust-lang/rustup/releases/tag/1.23.1

## [1.23.0] - 2020-11-27

The main points for this release are that `rustup` now supports a number of new
host platforms, most importantly of which is `aarch64-apple-darwin` for the new
Apple M1 based devices, and that we support a new structured format for the
`rust-toolchain` file. You can find more information
[in the new book format documentation][toolchain-file].

[toolchain-file]: https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file

It is now also possible to install a particular release of the compiler as a
two-part version number. If you do this, then the release channel will only
update if there is a patch release of the compiler. For example, if you ran
`rustup toolchain install 1.48` at the time of this release of `rustup` you
would end up with a toolchain called `1.48` which contained `1.48.0`. If
subsequently `1.48.1` were released, a `rustup update` would update your `1.48`
from `1.48.0` to `1.48.1`.

As always, there were more changes than described below, thanks to everyone
who contributed to this release. Highlights for this release are detailed below,
but you can always see the full list of changes via the Git repository.

### Added

- Our documentation is now in "book" form. [pr#2448]
- When you retrieve `rustup`'s version, you'll also be told the version of the
  compiler for your default toolchain, to disambiguate things a little. [pr#2465]
- Support added for `aarch64-unknown-linux-musl` [pr#2493]
- Support added for `aarch64-apple-darwin` [pr#2521]
- Support added for `x86_64-unknown-illumos` [pr#2432]
- You can now override the system-wide settings fallback path [pr#2545]
- Support for `major.minor` channels [pr#2551]

### Changed

- Significant updates to our handling of `PATH` updating on installation was
  made. Nominally this ought to have little external change visibility but
  it may make it more robust for some people. [pr#2387]
- New support for toml-based `rust-toolchain` file format. This will be expanded
  upon going into the future to add new functionality, but for now the basics
  are in place, permitting you to select a channel, targets, and components which
  may be needed to build your applications. [pr#2438]
- We now fall back to copying files when rename-in-place causes problems. This
  may improve matters in dockerised environments where `rustup` is preinstalled
  with a toolchain already. [pr#2410]
- We do a better job of exiting gracefully in a number of circumstances.
  [pr#2427]
- The `reqwest` backend (the default download backend) now supports socks5
  proxies. [pr#2466]
- If you use a proxy for a component which is not part of a custom toolchain
  you are using then we emit a message about trying to build that component.
  [pr#2487]
- If you try and unpack super-large components which would previously be
  gracefully rejected, instead we _try_ and if we succeed then you get to have
  the component unpacked. Unfortunately this means if we fail you could end
  up with a broken toolchain install. [pr#2490]
- We will recommend ways to recover if you can't update your toolchain due to
  components or targets going missing. [pr#2384]
- If you choose to install a toolchain which is for a different target than
  you are running on, we will warn you and direct you toward `rustup target install`
  in case that's what you meant. [pr#2534]

### Thanks

- Aaron Loucks
- Aleksey Kladov
- Aurelia Dolo
- Camelid
- Chansuke
- Carol (Nichols || Goulding)
- Daniel Silverstone
- Dany Marcoux
- Eduard Miller
- Eduardo Broto
- Eric Huss
- Francesco Zardi
- FR Bimo
- Ivan Nejgebauer
- Ivan Tham
- Jake Goulding
- Jens Reidel
- Joshua M. Clulow
- Joshua Nelson
- Jubilee Young
- Leigh McCulloch
- Lzu Tao
- Matthias Krüger
- Matt Kraai
- Matt McKay
- Nick Ashley
- Pascal Hertleif
- Paul Lange
- Pietro Albini
- Robert Collins
- Stephen Muss
- Tom Eccles

[pr#2534]: https://github.com/rust-lang/rustup/pull/2534
[pr#2384]: https://github.com/rust-lang/rustup/pull/2384
[pr#2545]: https://github.com/rust-lang/rustup/pull/2545
[pr#2432]: https://github.com/rust-lang/rustup/pull/2432
[pr#2521]: https://github.com/rust-lang/rustup/pull/2521
[pr#2493]: https://github.com/rust-lang/rustup/pull/2493
[pr#2499]: https://github.com/rust-lang/rustup/pull/2499
[pr#2487]: https://github.com/rust-lang/rustup/pull/2487
[pr#2466]: https://github.com/rust-lang/rustup/pull/2466
[pr#2465]: https://github.com/rust-lang/rustup/pull/2465
[pr#2448]: https://github.com/rust-lang/rustup/pull/2448
[pr#2427]: https://github.com/rust-lang/rustup/pull/2427
[pr#2410]: https://github.com/rust-lang/rustup/pull/2410
[pr#2438]: https://github.com/rust-lang/rustup/pull/2438
[pr#2387]: https://github.com/rust-lang/rustup/pull/2387
[pr#2551]: https://github.com/rust-lang/rustup/pull/2551
[1.23.0]: https://github.com/rust-lang/rustup/releases/tag/1.23.0

## [1.22.1] - 2020-07-08

A regression in proxied behaviour slipped in due to a non-compatible change
in `url` slipping in in 2.1 which [caused a misbehaviour][env_proxy#8] in `env_proxy`.
which was [fixed][env_proxy@5591cc7] but not released to crates.io until after
1.22.0 was built.

Fortunately, _inejge_ noticed and provided a fix for us by publishing a new
`env_proxy` and providing us with [this fix][pr#2399].

We apologise for any inconvenience this caused.

### Changed

- Update to `env_proxy` 0.4.1 - [#2399][pr#2399]
- Fixed website copy button and copy space overflow - [#2398][pr#2398]

### Thanks

- Ivan Nejgebauer
- Ben Chen

[env_proxy#8]: https://github.com/inejge/env_proxy/issues/8
[env_proxy@5591cc7]: https://github.com/inejge/env_proxy/commit/5591cc7
[pr#2399]: https://github.com/rust-lang/rustup/pull/2399
[pr#2398]: https://github.com/rust-lang/rustup/pull/2398
[1.22.1]: https://github.com/rust-lang/rustup/releases/tag/1.22.1

## [1.22.0] - 2020-06-30

Alongside a significant amount of internal refactoring and code updates,
the highlights of this release include:

- We have switched to Github Actions to make our CI and release process
  more consistent.
- We've invested time in the flow when you reinstall `rustup` atop an existing
  installation.
- We've doubled down on discouraging the use of the internal-development-focussed
  `complete` profile. Please use `default` or `minimal` unless you're trying to
  test/develop the Rust tooling itself.
- We've made a number of subtle quality-of-life improvements around the CLI.
- Added a (provisionally unofficial) snap of `rustup`
- We've worked hard to improve a lot of the messages (error and informational)
  in the tool.
- We've increased internal timeouts and retries in an attempt to improve the
  situation for McAfee users.
- While it's not a change, we've documented that `rust-toolchain` **must** be
  UTF8 encoded.

While the changes spanned around 90 individual pull requests, here are the
main changes and additions…

### Changed

- Fixed various links to our repo and to the forge - [#2173][pr#2173]
- Improved OS detection (particularly darwin) in `rustup-init.sh` - [#2042][pr#2042]
- Fixed bug where i686 installer on x86_64 windows would intend to install 64-bit but
  would actually install 32-bit toolchains by default. - [#2186][pr#2186]
- Increased width of copy box on rustup website - [#2208][pr#2208]
- When updating a toolchain, indicate the version you updated _from_ as well. - [#2152][pr#2152]
- When installing atop an existing `rustup` installation, we will now update
  the installed default toolchain, particularly we'll also try and install any
  additional targets or components specified - [#2201][pr#2201] and [#2339][pr#2339]
- Fixed issue where `rustup doc` wouldn't work with custom toolchains - [#2235][pr#2235]
- In low-memory situations, attempt to unpack more conservatively - [#2236][pr#2236]
- Improved consistency in where `rustup` will auto-install a toolchain on use. - [#2252][pr#2252]
- Try to force strong cipher suites in `rustup-init.sh` - [#2287][pr#2287]
- When skipping a `nightly` indicate **all** the missing components - [#2316][pr#2316]
- Increase timeout for rename retries - [#2348][pr#2348]
- Increased 'sanity limit' to account for MIPS binary size increases - [#2363][pr#2363]
- Fallback to non-threaded installation pathway on 1-CPU systems to improve chance
  that installation will succeed on Raspberry Pi - [#2372][pr#2372]

### Added

- It is now possible to install `rustup` even when there's an existing `rustup.sh`
  installation, and we can install alongside `rustc` or `cargo` without necessarily
  forcing via `-y` by means of the `RUSTUP_INIT_SKIP_EXISTENCE_CHECKS` environment
  variable. - [#2214][pr#2214]
- Added the concept of a _fallback_ settings file which will allow snaps, distro
  packages, etc. to provide a default toolchain for users who have not passed
  through the `rustup-init` managed one-time question set. - [#2244][pr#2244]
- You can now specify multiple components in a single argument in the form
  `--component rls,rust-analysis,rust-src` when installing toolchains - [#2239][pr#2239]
- It is now possible to `snap install --classic rustup` in theory (channel
  details may take some time to settle) - [#1898][pr#1898]
- Added indication of why overrides are happening when running `rustup show` - [#2312][pr#2312]
- Added `riscv64gc-unknown-linux-gnu` support (note: There is still work to be
  done on the compiler etc before this will necessarily work) - [#2313][pr#2313]

### Thanks

- Alejandro Martinez Ruiz
- Alexander D'hoore
- Ben Chen
- Chris Denton
- Daniel Silverstone
- Evan Weiler
- Guillaume Gomez
- Harry Sarson
- Jacob Lifshay
- James Yang
- Joel Parker Henderson
- John Titor
- Jonas Platte
- Josh Stone
- Jubilee
- Kellda
- LeSeulArtichaut
- Linus Färnstrand
- LitoMore
- LIU An (劉安)
- Luciano Bestia
- Lzu Tao
- Manish Goregaokar
- Mingye Wang
- Montgomery Edwards
- Per Lundberg
- Pietro Albini
- Robert Collins
- Rudolf B.
- Solomon Ucko
- Stein Somers
- Tetsuharu Ohzeki
- Tom Eccles
- Trevor Arjeski
- Tshepang Lekhonkhobe

[pr#2173]: https://github.com/rust-lang/rustup/pull/2173
[pr#2042]: https://github.com/rust-lang/rustup/pull/2042
[pr#2186]: https://github.com/rust-lang/rustup/pull/2186
[pr#2208]: https://github.com/rust-lang/rustup/pull/2208
[pr#2152]: https://github.com/rust-lang/rustup/pull/2152
[pr#2201]: https://github.com/rust-lang/rustup/pull/2201
[pr#2235]: https://github.com/rust-lang/rustup/pull/2235
[pr#2214]: https://github.com/rust-lang/rustup/pull/2214
[pr#2236]: https://github.com/rust-lang/rustup/pull/2236
[pr#2252]: https://github.com/rust-lang/rustup/pull/2252
[pr#2244]: https://github.com/rust-lang/rustup/pull/2244
[pr#2239]: https://github.com/rust-lang/rustup/pull/2239
[pr#2287]: https://github.com/rust-lang/rustup/pull/2287
[pr#2316]: https://github.com/rust-lang/rustup/pull/2316
[pr#1898]: https://github.com/rust-lang/rustup/pull/1898
[pr#2348]: https://github.com/rust-lang/rustup/pull/2348
[pr#2312]: https://github.com/rust-lang/rustup/pull/2312
[pr#2313]: https://github.com/rust-lang/rustup/pull/2313
[pr#2363]: https://github.com/rust-lang/rustup/pull/2363
[pr#2372]: https://github.com/rust-lang/rustup/pull/2372
[pr#2339]: https://github.com/rust-lang/rustup/pull/2339
[1.22.0]: https://github.com/rust-lang/rustup/releases/tag/1.22.0

## [1.21.1] - 2019-12-19

A panic occurred if a `rustup update` was run with nothing to update and the
download directory was missing. This was harmless but could have confused some
automation jobs.

[1.21.1]: https://github.com/rust-lang/rustup/releases/tag/1.21.1

## [1.21.0] - 2019-12-19

In release 1.20.x profiles could incorrectly ascribe host-independent components
to the host architecture, resulting in surprising behaviour with `rust-src`.
We have [corrected this][pr#2087] and [added mitigations][pr#2115] which should
mean that as of this release, such incorrect ascriptions are supported and also
automatically corrected on toolchain update.

Due to the large number of confusions around the `complete` profile, we have
[introduced a warning][pr#2138] if you use it. It's really only meant for
developers _of_ Rust, or those exploring particular issues in `nightly`.

There are also a large number of other changes, the highlights of which are below.
Thanks to everyone who helped work on this release. Even if your changes are not
listed below, they are still greatly appreciated.

### Changed

- [Download directory is cleaned up after successful full update.][pr#2046]
- [Bad `.partial` downloads will be cleaned up for you][pr#1889]
- [Force installation of toolchain if install is automatic][pr#2074]
- [Switch to darker colours to improve terminal readability][pr#2083]
- [Attempt to be less surprising wrt. default-host during installation][pr#2086]
- [`rustup toolchain list --verbose` now correctly shows the paths][pr#2084]
- [Fallback environment for non-cargo toolchains updated to match `rustc`][pr#2108]
- [Made human-readable units slightly more comprehensible][pr#2043]
- [Improved detection of armhf userland on aarch64 kernels][pr#2133]
- [Improved error message when rustc is detected on installation][pr#2155]

### Added

- [Added `--profile` support to `rustup toolchain install`][pr#2075]
- [Added `+toolchain` support to `rustup` itself to match proxy functionality][pr#2031]
- [Added ability to `rustup component add component-architecture`][pr#2088]
- [Added clear report when `rustup doc` is run without `rust-docs` available][pr#2116]
- [Added `keyword:`, `primitive:`, and `macro:` prefix support to `rustup doc FOO`][pr#2119]
- [Added retry logic so that `rustup` will try and repeat interrupted downloads][pr#2121]
- [Added `--allow-downgrade` support to `rustup toolchain install`][pr#2126]
- [Added display of previous version when upgrading channels][pr#2143]
- [Added support for local non-channel toolchains in rust-toolchain file][pr#2141]

### Thanks

- Roman Frołow
- Jean Simard
- Lzu Tao
- Benjamin Chen
- Daniel Silverstone
- Jon Hoo
- Carlo Abelli
- Filip Demski
- Chris Tomlinson
- Kane Green
- Ralf Jung
- Yves Dorfsman
- Rudolf B
- Pietro Albini
- Takayuki Nakata
- Justus K
- Gilbert Röhrbein
- Friedel Ziegelmayer
- Robbie Clarken
- Tetsuharu OHZEKI

[pr#1889]: https://github.com/rust-lang/rustup/pull/1889
[pr#2031]: https://github.com/rust-lang/rustup/pull/2031
[pr#2043]: https://github.com/rust-lang/rustup/pull/2043
[pr#2046]: https://github.com/rust-lang/rustup/pull/2046
[pr#2074]: https://github.com/rust-lang/rustup/pull/2074
[pr#2075]: https://github.com/rust-lang/rustup/pull/2075
[pr#2083]: https://github.com/rust-lang/rustup/pull/2083
[pr#2084]: https://github.com/rust-lang/rustup/pull/2084
[pr#2086]: https://github.com/rust-lang/rustup/pull/2086
[pr#2087]: https://github.com/rust-lang/rustup/pull/2087
[pr#2108]: https://github.com/rust-lang/rustup/pull/2108
[pr#2088]: https://github.com/rust-lang/rustup/pull/2088
[pr#2115]: https://github.com/rust-lang/rustup/pull/2115
[pr#2116]: https://github.com/rust-lang/rustup/pull/2116
[pr#2119]: https://github.com/rust-lang/rustup/pull/2119
[pr#2121]: https://github.com/rust-lang/rustup/pull/2121
[pr#2126]: https://github.com/rust-lang/rustup/pull/2126
[pr#2133]: https://github.com/rust-lang/rustup/pull/2133
[pr#2138]: https://github.com/rust-lang/rustup/pull/2138
[pr#2141]: https://github.com/rust-lang/rustup/pull/2141
[pr#2143]: https://github.com/rust-lang/rustup/pull/2143
[pr#2155]: https://github.com/rust-lang/rustup/pull/2155
[1.21.0]: https://github.com/rust-lang/rustup/releases/tag/1.21.0

## [1.20.2] - 2019-10-16

One final tweak was needed to the force-installation of toolchains because
otherwise components would be marked as installed when they were not.

Our apologies to anyone adversely affected by the 1.20.0/1 releases.

[1.20.2]: https://github.com/rust-lang/rustup/releases/tag/1.20.2

## [1.20.1] - 2019-10-16

This release was made to solve two problems spotted in `1.20.0`

- Force installation of toolchain during `rustup-init` to improve handling
  on non-tier-one platforms
- Assume the `default` profile if a profile is missing from configuration
  which will solve a problem where distro-provided `rustup` binaries did not
  upgrade the configuration properly

[1.20.1]: https://github.com/rust-lang/rustup/releases/tag/1.20.1

## [1.20.0] - 2019-10-15

### Changed

- [Toolchain listing now supports a verbose mode][pr#1988]
- [Improve zsh completions for cargo][pr#1995]
- [Updates/Installations of nightly now backtrack][pr#2002]
- [Improve handling of Ctrl+C on Windows][pr#2014]
- [`rustup which` now supports `--toolchain`][pr#2030]

### Added

- [Added installation profiles][pr#1673]
- [Added `rustup check`][pr#1980]
- [Support for `--quiet` in most places][pr#1945]
- [Support for adding components and targets during toolchain install][pr#2026]

### Thanks

- Nick Cameron
- Andy McCaffrey
- Pietro Albini
- Benjamin Chen
- Artem Borisovskiy
- Jon Gjengset
- Lzu Tao
- Daniel Silverstone
- PicoJr
- Mitchell Hynes
- Matt Kantor

[pr#1673]: https://github.com/rust-lang/rustup/pull/1673
[pr#1980]: https://github.com/rust-lang/rustup/pull/1980
[pr#1988]: https://github.com/rust-lang/rustup/pull/1988
[pr#1995]: https://github.com/rust-lang/rustup/pull/1995
[pr#2002]: https://github.com/rust-lang/rustup/pull/2002
[pr#2014]: https://github.com/rust-lang/rustup/pull/2014
[pr#1945]: https://github.com/rust-lang/rustup/pull/1945
[pr#2026]: https://github.com/rust-lang/rustup/pull/2026
[pr#2030]: https://github.com/rust-lang/rustup/pull/2030
[1.20.0]: https://github.com/rust-lang/rustup/releases/tag/1.20.0

## [1.19.0] - 2019-09-09

### Changed

- [Fix race condition with some virus scanners][pr#1873]
- [UI improvements for race condition fix][pr#1885]
- [Improve home mismatch explanation][pr#1895]
- [Enable fully threaded IO for installs][pr#1876]
- [Improve look of rustup homepage][pr#1901]
- [Improve messaging if shell profile cannot be updated][pr#1925]
- [Improve messaging around directory names during install][pr#1914]
- [Disregard unavailable targets][pr#1931]
- [No longer provide non-panic backtraces by default][pr#1961]

### Added

- [Add support for `rustup target add all`][pr#1868]
- [Add `rustup show home`][pr#1933]
- [Add NetBSD target to CI][pr#1978]
- [Add x86_64 musl to CI][pr#1882]

### Thanks

- Lzu Tao
- Gonzalo Brito Gadeschi
- Paul Oppenheimer
- Robert Collins
- KennyTM
- Daniel Silverstone
- Nicholas Parker
- Caleb Cartwright
- Josh Holland
- Charlie Saunders
- Wesley Van Melle
- Jason Cooke
- CrLF0710
- Brian Anderson
- Bryan Dady
- Fisher Darling
- Bjorn3
- Iku Iwasa

[pr#1873]: https://github.com/rust-lang/rustup/pull/1873
[pr#1885]: https://github.com/rust-lang/rustup/pull/1885
[pr#1882]: https://github.com/rust-lang/rustup/pull/1882
[pr#1895]: https://github.com/rust-lang/rustup/pull/1895
[pr#1876]: https://github.com/rust-lang/rustup/pull/1876
[pr#1901]: https://github.com/rust-lang/rustup/pull/1901
[pr#1925]: https://github.com/rust-lang/rustup/pull/1925
[pr#1914]: https://github.com/rust-lang/rustup/pull/1914
[pr#1931]: https://github.com/rust-lang/rustup/pull/1931
[pr#1961]: https://github.com/rust-lang/rustup/pull/1961
[pr#1868]: https://github.com/rust-lang/rustup/pull/1868
[pr#1933]: https://github.com/rust-lang/rustup/pull/1933
[pr#1978]: https://github.com/rust-lang/rustup/pull/1978
[1.19.0]: https://github.com/rust-lang/rustup/releases/tag/1.19.0

## [1.18.3] - 2019-05-22

### Changed

- [Improve performance by only opening terminfo once][pr#1820]
- [Use same webpage opening logic as cargo][pr#1830]
- [Report download duration on completion][pr#1837]
- [Reduce stat() usage in unpacking][pr#1839]
- [Buffer reads from tarfile during unpacking][pr#1840]
- [Buffer for hashing of dist content][pr#1845]
- [Don't set mtime on unpacked toolchain files][pr#1847]
- [UI consistency/improvement in download speeds][pr#1832]
- [Avoid blocking on CloseHandle][pr#1850]

### Added

- [Suggest possible components or targets if misspelled][pr#1824]

### Thanks

- Robert Collins (who has tirelessly worked to improve
  the performance of Rustup, particularly on Windows)
- Lucien Greathouse
- Filip Demski
- Peter Hrvola
- Bogdan Kulbida
- Srinivas Reddy Thatiparthy
- Sunjay Varma
- Lzu Tao (behind the scenes, lots of housekeeping and CI)

[pr#1820]: https://github.com/rust-lang/rustup/pull/1820
[pr#1830]: https://github.com/rust-lang/rustup/pull/1830
[pr#1837]: https://github.com/rust-lang/rustup/pull/1837
[pr#1839]: https://github.com/rust-lang/rustup/pull/1839
[pr#1840]: https://github.com/rust-lang/rustup/pull/1840
[pr#1845]: https://github.com/rust-lang/rustup/pull/1845
[pr#1847]: https://github.com/rust-lang/rustup/pull/1847
[pr#1832]: https://github.com/rust-lang/rustup/pull/1832
[pr#1850]: https://github.com/rust-lang/rustup/pull/1850
[pr#1824]: https://github.com/rust-lang/rustup/pull/1824
[1.18.3]: https://github.com/rust-lang/rustup/releases/tag/1.18.3

## [1.18.2] - 2019-05-02

### Changed

- [Fix local bash-completion directory path][pr#1809]
- [Handle stray toolchain hashes during install][pr#1801]
- [Update to env_proxy 0.3.1][pr#1819]
- [Improvements to release process around Windows versions][pr#1822]

### Added

- [Support listing installed targets only][pr#1808]
- [Added CI of CentOS 6 support for rustup-init.sh][pr#1810]
- [FAQ entry about not being able to update rustup on Windows][pr#1813]

### Thanks

This release was made, in part, thanks to:

- Brian Ericson
- Onat Mercan
- Lzu Tao
- Takuto Ikuta
- Jason Williams
- Filip Demski
- Michael Maclean
- Daniel Silverstone

[pr#1809]: https://github.com/rust-lang/rustup/pull/1809
[pr#1801]: https://github.com/rust-lang/rustup/pull/1801
[pr#1819]: https://github.com/rust-lang/rustup/pull/1819
[pr#1822]: https://github.com/rust-lang/rustup/pull/1822
[pr#1808]: https://github.com/rust-lang/rustup/pull/1808
[pr#1810]: https://github.com/rust-lang/rustup/pull/1810
[pr#1813]: https://github.com/rust-lang/rustup/pull/1813
[1.18.2]: https://github.com/rust-lang/rustup/releases/tag/1.18.2

## [1.18.1] - 2019-04-25

### Changed

- [Fix panic when no default toolchain is installed][pr#1787]
- [Remove repeated CLI subcommands][pr#1796]
- [Detect s390x in rustup-init.sh][pr#1797]
- [Fallback to less secure curl/wget invocation][pr#1803]

[pr#1787]: https://github.com/rust-lang/rustup/pull/1787
[pr#1796]: https://github.com/rust-lang/rustup/pull/1796
[pr#1797]: https://github.com/rust-lang/rustup/pull/1797
[pr#1803]: https://github.com/rust-lang/rustup/pull/1803
[1.18.1]: https://github.com/rust-lang/rustup/releases/tag/1.18.1

## [1.18.0] - 2019-04-22

### Added

- [Output shell completions for cargo by `rustup completions <shell> cargo`][pr#1646]
- [Add `--embedded-book` flag to `rustup doc`][pr#1762]
- [Add --path option to `rustup override set`][pr#1524]

### Changed

- [`rustup default` now tells user if current directory is override][pr#1655]
- [`rustup-init`: Force highest TLS version supported][pr#1716]
- [Switch to git-testament rather than old `build.rs`][pr#1701]
- [Less copying during dist installation][pr#1744]
- [Improve error messages when missing nightly components][pr#1769]
- [Improve `rustup install` error message][pr#1770]
- [Update Visual C++ install instructions, to link to Visual Studio 2019][pr#1773]
- [Use `DYLD_FALLBACK_LIBRARY_PATH` for `dylib_path_envvar` on macOS][pr#1752]
- [Improved documentation for shell completion enabling][pr#1780]
- [Added shellcheck and Travis folding][pr#1776]

### Fixed

- [`rustup-init.sh`: Fix unset variable usage][pr#1683]
- [Treat time in seconds as an integer for download times][pr#1699]
- [Fix man proxy in FreeBSD][pr#1725]
- [Fix networking failure after using socks5 proxy][pr#1746]
- [Fix `rustup show` fails on terminal without color][pr#1747]
- [Fix installation failed if `rustup-init` is owned by another user][pr#1767]
- [Fix panics with "Broken pipe" when using in a shell pipeline][pr#1765]
- [Document `--no-self-update` properly][pr#1763]
- [Clear line properly in download progress][pr#1781]
- [More download progress line clearing fixes][pr#1788]
- [Fix a bunch of clippy warnings/errors][pr#1778]

### Removed

- [Remove old `multirust` & compatibility code][pr#1715]

[pr#1646]: https://github.com/rust-lang/rustup/pull/1646
[pr#1762]: https://github.com/rust-lang/rustup/pull/1762
[pr#1524]: https://github.com/rust-lang/rustup/pull/1524
[pr#1655]: https://github.com/rust-lang/rustup/pull/1655
[pr#1716]: https://github.com/rust-lang/rustup/pull/1716
[pr#1701]: https://github.com/rust-lang/rustup/pull/1701
[pr#1744]: https://github.com/rust-lang/rustup/pull/1744
[pr#1769]: https://github.com/rust-lang/rustup/pull/1769
[pr#1770]: https://github.com/rust-lang/rustup/pull/1770
[pr#1773]: https://github.com/rust-lang/rustup/pull/1773
[pr#1752]: https://github.com/rust-lang/rustup/pull/1752
[pr#1683]: https://github.com/rust-lang/rustup/pull/1683
[pr#1699]: https://github.com/rust-lang/rustup/pull/1699
[pr#1725]: https://github.com/rust-lang/rustup/pull/1725
[pr#1746]: https://github.com/rust-lang/rustup/pull/1746
[pr#1747]: https://github.com/rust-lang/rustup/pull/1747
[pr#1767]: https://github.com/rust-lang/rustup/pull/1767
[pr#1765]: https://github.com/rust-lang/rustup/pull/1765
[pr#1763]: https://github.com/rust-lang/rustup/pull/1763
[pr#1715]: https://github.com/rust-lang/rustup/pull/1715
[pr#1776]: https://github.com/rust-lang/rustup/pull/1776
[pr#1778]: https://github.com/rust-lang/rustup/pull/1778
[pr#1780]: https://github.com/rust-lang/rustup/pull/1780
[pr#1781]: https://github.com/rust-lang/rustup/pull/1781
[pr#1788]: https://github.com/rust-lang/rustup/pull/1788
[1.18.0]: https://github.com/rust-lang/rustup/releases/tag/1.18.0

## [1.17.0] - 2019-03-05

- [Allow using inherited RUSTUP_UPDATE_ROOT variable in rustup-init.sh.][pr#1495]
- [Fix `utils::copy_file` for symlink.][pr#1521]
- [Improve formatting of longer download times in download tracker][pr#1547]
- [Basic 2018 edition fix][pr#1583]
- [Update rustup-init.sh for 32bit powerpc userland][pr#1587]
- [Reformat the entire codebase using `cargo fmt`][pr#1585]
- [Support to open more documents directly in `rustup doc`][pr#1597]
- [Fix HumanReadable#fmt][pr#1603]
- [Add more detail error messages when installing with some components has failed.][pr#1595]
- [Fix a panic when a component is missing][pr#1576]
- [Update to use `dirs::home_dir()`][pr#1588]
- [Self update after updating a specific toolchain][pr#1605]
- [Add miri to rustup][pr#1606]
- [allow non-utf8 arguments to proxies][pr#1599]
- [rustup-dist: Use Download notifications to track install][pr#1593]
- [Deal cleanly with malformed default-host][pr#1578]
- [Better error message for missing binary][pr#1619]
- [Add tab completion instructions for PowerShell][pr#1623]
- [Add tab completion test for PowerShell][pr#1629]
- [When updating, show "removing old component" to avoid confusion][pr#1639]
- [Upgrade to Rust 2018 edition idioms][pr#1643]
- [Simplify host triplet passing code][pr#1645]
- [Remove telemetry][pr#1642]
- [Print default toolchain on `rustup default` without arguments][pr#1633]
- [Bring output of `rustup show active-toolchain` and `rustup default` into line with rest of rustup][pr#1654]
- [Deprecate cURL][pr#1660]
- [Thread toolchain through to error message][pr#1616]
- [Add Listing of Installed Components (`rustup component list --installed`)][pr#1659 ]
- [Add `clippy-driver` as a proxy][pr#1679]
- [Remove the `rustup-win-installer` directory][pr#1666]

[pr#1495]: https://github.com/rust-lang/rustup/pull/1495
[pr#1521]: https://github.com/rust-lang/rustup/pull/1521
[pr#1547]: https://github.com/rust-lang/rustup/pull/1547
[pr#1583]: https://github.com/rust-lang/rustup/pull/1583
[pr#1587]: https://github.com/rust-lang/rustup/pull/1587
[pr#1585]: https://github.com/rust-lang/rustup/pull/1585
[pr#1597]: https://github.com/rust-lang/rustup/pull/1597
[pr#1603]: https://github.com/rust-lang/rustup/pull/1603
[pr#1595]: https://github.com/rust-lang/rustup/pull/1595
[pr#1576]: https://github.com/rust-lang/rustup/pull/1576
[pr#1588]: https://github.com/rust-lang/rustup/pull/1588
[pr#1605]: https://github.com/rust-lang/rustup/pull/1605
[pr#1606]: https://github.com/rust-lang/rustup/pull/1606
[pr#1599]: https://github.com/rust-lang/rustup/pull/1599
[pr#1593]: https://github.com/rust-lang/rustup/pull/1593
[pr#1578]: https://github.com/rust-lang/rustup/pull/1578
[pr#1619]: https://github.com/rust-lang/rustup/pull/1619
[pr#1623]: https://github.com/rust-lang/rustup/pull/1623
[pr#1629]: https://github.com/rust-lang/rustup/pull/1629
[pr#1639]: https://github.com/rust-lang/rustup/pull/1639
[pr#1643]: https://github.com/rust-lang/rustup/pull/1643
[pr#1645]: https://github.com/rust-lang/rustup/pull/1645
[pr#1642]: https://github.com/rust-lang/rustup/pull/1642
[pr#1633]: https://github.com/rust-lang/rustup/pull/1633
[pr#1654]: https://github.com/rust-lang/rustup/pull/1654
[pr#1660]: https://github.com/rust-lang/rustup/pull/1660
[pr#1616]: https://github.com/rust-lang/rustup/pull/1616
[pr#1659]: https://github.com/rust-lang/rustup/pull/1659
[pr#1679]: https://github.com/rust-lang/rustup/pull/1679
[pr#1666]: https://github.com/rust-lang/rustup/pull/1666
[1.17.0]: https://github.com/rust-lang/rustup/releases/tag/1.17.0

## [1.16.0] - 2018-12-06

- [Fix rename_rls_remove test on Windows][pr#1561]

[pr#1561]: https://github.com/rust-lang/rustup/pull/1561
[1.16.0]: https://github.com/rust-lang/rustup/releases/tag/1.16.0

## [1.15.0] - 2018-11-27

- [More tweaks to renames][pr#1554]
- [Return Ok status when trying to add required component][pr#1553]
- [Use `renames` instead of `rename` to match the actual manifest][pr#1552]
- [Size optimizations: Build with LTO and alloc_system][pr#1526]
- [Use `openssl-src` from crates.io to link to OpenSSL][pr#1536]
- [Change handling of renames][pr#1549]

[pr#1554]: https://github.com/rust-lang/rustup/pull/1554
[pr#1553]: https://github.com/rust-lang/rustup/pull/1553
[pr#1552]: https://github.com/rust-lang/rustup/pull/1552
[pr#1526]: https://github.com/rust-lang/rustup/pull/1526
[pr#1536]: https://github.com/rust-lang/rustup/pull/1536
[pr#1549]: https://github.com/rust-lang/rustup/pull/1549
[1.15.0]: https://github.com/rust-lang/rustup/releases/tag/1.15.0

## [1.14.0] - 2018-10-04

- [Fix Windows job management][pr#1511]
- [Preserve symlinks when installing][pr#1504]
- [Add `--toolchain` option to `rustup doc`][pr#1478]
- [Fix removing toolchain fail when update-hash does not exist][pr#1472]
- [Add note about installing the Windows SDK component][pr#1468]

[pr#1511]: https://github.com/rust-lang/rustup/pull/1511
[pr#1504]: https://github.com/rust-lang/rustup/pull/1504
[pr#1478]: https://github.com/rust-lang/rustup/pull/1478
[pr#1472]: https://github.com/rust-lang/rustup/pull/1472
[pr#1468]: https://github.com/rust-lang/rustup/pull/1468
[1.14.0]: https://github.com/rust-lang/rustup/releases/tag/1.14.0

## [1.13.0] - 2018-07-16

- [Add clippy to the tools list][pr1461]

[pr1461]: https://github.com/rust-lang/rustup/pull/1461
[1.13.0]: https://github.com/rust-lang/rustup/releases/tag/1.13.0

Contributors: Jane Lusby

## [1.12.0] - 2018-07-07

- [Add --path flag to 'rustup doc'][pr1453]
- [Add flag to "rustup show" for active-toolchain][pr1449]
- [Bring rustup.js and markup into alignment with rust-www][pr1437]
- [Add caret after first installation question][pr1435]
- [Add "rustup doc --reference"][pr1430]
- [Update Visual C++ Build Tools URL][pr1428]
- [Fix download indicator on OSes with newer ncurses package][pr1422]
- [Remove components if they don't exist anymore during update][pr1419]
- [Make sure rustup uses `utils::rename*` consistently][pr1389]
- [Do not try to get CWD if not required][pr1379]
- [Give correct error message if user tries to install an unavailable toolchain][pr1380]
- [Fall back to wget if curl is not installed][pr1373]
- [Added a link to all installers to the homepage][pr1370]
- [Display helpful advice even with -y][pr1290]
- [Use browser in BROWSER env if present for `doc` command][pr1289]
- [Update shebang to reflect bashisms][pr1269]

[pr1453]: https://github.com/rust-lang/rustup/pull/1453
[pr1449]: https://github.com/rust-lang/rustup/pull/1449
[pr1437]: https://github.com/rust-lang/rustup/pull/1437
[pr1435]: https://github.com/rust-lang/rustup/pull/1435
[pr1430]: https://github.com/rust-lang/rustup/pull/1430
[pr1428]: https://github.com/rust-lang/rustup/pull/1428
[pr1422]: https://github.com/rust-lang/rustup/pull/1422
[pr1419]: https://github.com/rust-lang/rustup/pull/1419
[pr1389]: https://github.com/rust-lang/rustup/pull/1389
[pr1379]: https://github.com/rust-lang/rustup/pull/1379
[pr1380]: https://github.com/rust-lang/rustup/pull/1380
[pr1373]: https://github.com/rust-lang/rustup/pull/1373
[pr1370]: https://github.com/rust-lang/rustup/pull/1370
[pr1290]: https://github.com/rust-lang/rustup/pull/1290
[pr1289]: https://github.com/rust-lang/rustup/pull/1289
[pr1269]: https://github.com/rust-lang/rustup/pull/1269
[1.12.0]: https://github.com/rust-lang/rustup/releases/tag/1.12.0

Contributors: Andrew Pennebaker, Who? Me?!, Matteo Bertini, mog422,
Kasper Møller Andersen, Thibault Delor, Justin Worthe, TitanSnow,
aimileus, Antonio Murdaca, Cyryl Płotnicki, Nick Cameron, Alex Crichton,
Kornel, Stuart Dootson, Pietro Albini, Diggory Blake, Yuji Nakao,
Johannes Hofmann, CrLF0710, Aaron Lee, Brian Anderson, Mateusz Mikuła,
Segev Finer, Dan Aloni, Joeri van Ruth

## [1.11.0] - 2018-02-13

- [windows: detect architecture on website, update to matching arch][pr1354]

[pr1354]: https://github.com/rust-lang/rustup/pull/1354
[1.11.0]: https://github.com/rust-lang/rustup/releases/tag/1.11.0

Contributors: Steffen Butzer

## [1.10.0] - 2018-01-25

- [Warn when tools are missing and allow an override][pr1337]

[pr1337]: https://github.com/rust-lang/rustup/pull/1337
[1.10.0]: https://github.com/rust-lang/rustup/releases/tag/1.10.0

Contributors: Nick Cameron, Steffen Butzer

## [1.9.0] - 2018-01-04

- [Fix self update errors filling in missing proxies][pr1326]

[pr1326]: https://github.com/rust-lang/rustup/pull/1326
[1.9.0]: https://github.com/rust-lang/rustup/releases/tag/1.9.0

Contributors: Alex Crichton

## [1.8.0] - 2017-12-19

- [Add `rustup run --install`][pr1295]
- [Prevent `rustup update` to a toolchain without `rustc` or `cargo`][pr1298]
- [Add support for `rustfmt` shims][pr1294]

[pr1295]: https://github.com/rust-lang/rustup/pull/1295
[pr1298]: https://github.com/rust-lang/rustup/pull/1298
[pr1294]: https://github.com/rust-lang/rustup/pull/1294
[1.8.0]: https://github.com/rust-lang/rustup/releases/tag/1.8.0

Contributors: Alex Crichton, kennytm, Nick Cameron, Simon Sapin, Who? Me?!

## [1.7.0] - 2017-10-30

- [Improve clarity of component errors][pr1255]
- [Support `--default-toolchain none`][pr1257]
- [Automatically install override toolchain when missing][pr1250]

[pr1255]: https://github.com/rust-lang/rustup/pull/1255
[pr1257]: https://github.com/rust-lang/rustup/pull/1257
[pr1250]: https://github.com/rust-lang/rustup/pull/1250
[1.7.0]: https://github.com/rust-lang/rustup/releases/tag/1.7.0

Contributors: Aidan Hobson Sayers, Alan Du, Alex Crichton, Christoph Wurst,
Jason Mobarak, Leon Isenberg, Simon Sapin, Vadim Petrochenkov

## [1.6.0] - 2017-08-30

- [Fix support for s390x][pr1228]
- [Fix `show` so it displays helpful information if the active toolchain is not installed][pr1189]
- [Fix uninstalling toolchains with stale symlinks][pr1201]
- [Replace the hyper backend with a reqwest downloading backend][pr1222]
- [Consistently give a toolchain argument in the help text][pr1212]
- [Use `exec` on Unix where possible to help manage Unix signals][pr1242]

[pr1228]: https://github.com/rust-lang/rustup/pull/1228
[pr1189]: https://github.com/rust-lang/rustup/pull/1189
[pr1201]: https://github.com/rust-lang/rustup/pull/1201
[pr1222]: https://github.com/rust-lang/rustup/pull/1222
[pr1212]: https://github.com/rust-lang/rustup/pull/1212
[pr1242]: https://github.com/rust-lang/rustup/pull/1242
[1.6.0]: https://github.com/rust-lang/rustup/releases/tag/1.6.0

Contributors: Alex Crichton, Chen Rotem Levy, Krishna Sundarram, Martin Geisler,
Matt Brubeck, Matt Ickstadt, Michael Benfield, Michael Fletcher, Nick Cameron,
Patrick Reisert, Ralf Jung, Sean McArthur, Steven Fackler

## [1.5.0] - 2017-06-24

- [Rename references to multirust to rustup where applicable](https://github.com/rust-lang/rustup/pull/1148)
- [Update platform support in README](https://github.com/rust-lang/rustup/pull/1159)
- [Allow rustup to handle unavailable packages](https://github.com/rust-lang/rustup/pull/1063)
- [Update libz-sys and curl-sys](https://github.com/rust-lang/rustup/pull/1176)
- [Teach rustup to override the toolchain from a version file](https://github.com/rust-lang/rustup/pull/1172)
- [Update sha2 crate](https://github.com/rust-lang/rustup/pull/1162)
- [Check for unexpected cargo/rustc before install](https://github.com/rust-lang/rustup/pull/705)
- [Update PATH in .bash_profile](https://github.com/rust-lang/rustup/pull/1179)

[1.5.0]: https://github.com/rust-lang/rustup/releases/tag/1.5.0

Contributors: Allen Welkie, bors, Brian Anderson, Diggory Blake, Erick
Tryzelaar, Ricardo Martins, Артём Павлов [Artyom Pavlov]

## [1.4.0] - 2017-06-09

- [set_file_perms: if the file is already executable, keep it executable](https://github.com/rust-lang/rustup/pull/1141)
- [Disable man support on Windows](https://github.com/rust-lang/rustup/pull/1139)
- [VS 2017 updates](https://github.com/rust-lang/rustup/pull/1145)
- [Show version of rust being installed](https://github.com/rust-lang/rustup/pull/1025)
- [Detect MSVC 2017](https://github.com/rust-lang/rustup/pull/1136)
- [Use same precision as rustc for commit sha](https://github.com/rust-lang/rustup/pull/1134)
- [Fix prompt asking for msvc even though -y is provided](https://github.com/rust-lang/rustup/pull/1124)
- [README: fix rust build dir](https://github.com/rust-lang/rustup/pull/1135)
- [Add support for XZ-compressed packages](https://github.com/rust-lang/rustup/pull/1100)
- [Add PATH in post-install message when not modifying PATH](https://github.com/rust-lang/rustup/pull/1126)
- [Cleanup download-related code in the rustup_dist crate](https://github.com/rust-lang/rustup/pull/1131)
- [Increase Rust detection timeout to 3 seconds](https://github.com/rust-lang/rustup/pull/1130)
- [Suppress confusing NotADirectory error and show override missing](https://github.com/rust-lang/rustup/pull/1128)
- [Don't try to update archive toolchains](https://github.com/rust-lang/rustup/pull/1121)
- [Exit successfully on "update not yet available"](https://github.com/rust-lang/rustup/pull/1120)
- [Add a message when removing a component](https://github.com/rust-lang/rustup/pull/1119)
- [Use ShellExecute rather than start.exe to open docs on windows](https://github.com/rust-lang/rustup/pull/1117)
- [Clarify that rustup update updates rustup itself](https://github.com/rust-lang/rustup/pull/1113)
- [Ensure that intermediate directories exist when unpacking an entry](https://github.com/rust-lang/rustup/pull/1098)
- [Add the rust lib dir (containing std-<hash>.dll) to the path on windows](https://github.com/rust-lang/rustup/pull/1093)
- [Add x86_64-linux-android target](https://github.com/rust-lang/rustup/pull/1086)
- [Fix for help.rs suggestion](https://github.com/rust-lang/rustup/pull/1107)
- [Ignore remove_override_nonexistent on windows](https://github.com/rust-lang/rustup/pull/1105)
- [Update proxy setting docs](https://github.com/rust-lang/rustup/pull/1088)
- [Add sensible-browser to the browser list](https://github.com/rust-lang/rustup/pull/1087)
- [Added help for `rustup toolchain link`](https://github.com/rust-lang/rustup/pull/1017)

[1.4.0]: https://github.com/rust-lang/rustup/releases/tag/1.4.0

Contributors: Andrea Canciani, bors, Brian Anderson, CrazyMerlyn, Diggory Blake,
Fabio B, James Elford, Jim McGrath, johnthagen, Josh Lee, Kim Christensen, Marco
A L Barbosa, Mateusz Mikula, Matthew, Matt Ickstadt, Mikhail Modin, Patrick
Deuster, pxdeu, Ralf Jung, Raphaël Huchet, Robert Vally, theindigamer, Tommy Ip,
Xidorn Quan

## [1.3.0] - 2017-05-09

- [Add armv8l support](https://github.com/rust-lang/rustup/pull/1055)
- [Update curl crate](https://github.com/rust-lang/rustup/pull/1101)
- [Fix inadvertent dependency on bash](https://github.com/rust-lang/rustup/pull/1048)
- [Update openssl-probe to 0.1.1](https://github.com/rust-lang/rustup/pull/1061)
- [zsh completions cleanup](https://github.com/rust-lang/rustup/pull/1068)
- [Alias 'rustup toolchain uninstall' to 'rustup uninstall'](https://github.com/rust-lang/rustup/pull/1073)
- [Fix a typo in PowerShell completion script help](https://github.com/rust-lang/rustup/pull/1076)
- [Enforce timeouts for reading rustc version](https://github.com/rust-lang/rustup/pull/1071)
- [Fix OpenSSL linkage by using the final install-directory in the build](https://github.com/rust-lang/rustup/pull/1065)

[1.3.0]: https://github.com/rust-lang/rustup/releases/tag/1.3.0

Contributors: bors, Brian Anderson, Diggory Blake, Greg Alexander, James Elford,
Jordan Hiltunen, Justin Noah, Kang Seonghoon, Kevin K, Marco A L Barbosa

## [1.2.0] - 2017-04-08

- [Check ZDOTDIR when adding path to .zprofile](https://github.com/rust-lang/rustup/pull/1038)
- [Update links and install page to include android support](https://github.com/rust-lang/rustup/pull/1037)
- [Add bash completion guidance for macOS users](https://github.com/rust-lang/rustup/pull/1035)
- [Support partial downloads](https://github.com/rust-lang/rustup/pull/1020)
- [Don't crash if modifying multiple profile files](https://github.com/rust-lang/rustup/pull/1040)

[1.2.0]: https://github.com/rust-lang/rustup/releases/tag/1.2.0

Contributors: Brian Anderson, James Elford, Jason Dreyzehner, Marco A
L Barbosa, Wim Looman

## [1.1.0] - 2017-04-06

- [Fix browser detection for Linux ppc64 and NetBSD](https://github.com/rust-lang/rustup/pull/875)
- [Update windows info](https://github.com/rust-lang/rustup/pull/879)
- [Update to markdown 0.2](https://github.com/rust-lang/rustup/pull/896)
- [Make running program extension case insensitive](https://github.com/rust-lang/rustup/pull/887)
- [Add MIPS/s390x builders (with PPC64 compilation fixed)](https://github.com/rust-lang/rustup/pull/890)
- [Fix two missing quotes of download error message](https://github.com/rust-lang/rustup/pull/867)
- [www: MIPS support and cleanups](https://github.com/rust-lang/rustup/pull/866)
- [Update release instructions](https://github.com/rust-lang/rustup/pull/863)
- [Don't set low speed limits for curl](https://github.com/rust-lang/rustup/pull/914)
- [Attempt to fix msi build. Pin appveyor nightlies](https://github.com/rust-lang/rustup/pull/910)
- [Stop defaulting to \$PATH searches when the binary can't be found and causing infinite recursion](https://github.com/rust-lang/rustup/pull/917)
- [Upgrade openssl](https://github.com/rust-lang/rustup/pull/934)
- [Improve browser detection and install instructions](https://github.com/rust-lang/rustup/pull/936)
- [Add android support to rustup-init.sh](https://github.com/rust-lang/rustup/pull/949)
- [Add fallback to symlink if hardlink fails](https://github.com/rust-lang/rustup/pull/951)
- [readme: add tmp dir hint to Contributing section](https://github.com/rust-lang/rustup/pull/985)
- [Fixed link to the list of supported platforms](https://github.com/rust-lang/rustup/pull/970)
- [Update job object code to match Cargo's](https://github.com/rust-lang/rustup/pull/984)
- [Added argument-documentation to rustup-init.sh](https://github.com/rust-lang/rustup/pull/962)
- [Add/remove multiple toolchains](https://github.com/rust-lang/rustup/pull/986)
- [Remove curl usage from appveyor](https://github.com/rust-lang/rustup/pull/1001)
- [Store downloaded files in a persistent directory until installation](https://github.com/rust-lang/rustup/pull/958)
- [Add android build support](https://github.com/rust-lang/rustup/pull/1000)
- [Fix up a bunch of things indicated by clippy](https://github.com/rust-lang/rustup/pull/1012)
- [Ensure librssl compatibility](https://github.com/rust-lang/rustup/pull/1011)
- [RLS support](https://github.com/rust-lang/rustup/pull/1005)
- [Add 'docs' alias](https://github.com/rust-lang/rustup/pull/1010)
- [Use correct name for undefined linked toolchain invocation](https://github.com/rust-lang/rustup/pull/1008)
- [zsh install support](https://github.com/rust-lang/rustup/pull/1013)
- [Add/remove multiple components+targets](https://github.com/rust-lang/rustup/pull/1016)
- [Better error message when not running in a tty](https://github.com/rust-lang/rustup/pull/1026)
- [Indent help text](https://github.com/rust-lang/rustup/pull/1019)
- [Document installing to a custom location using CARGO_HOME and RUSTUP_HOME environment variables](https://github.com/rust-lang/rustup/pull/1024)
- [Aggressive remove_dir_all](https://github.com/rust-lang/rustup/pull/1015)

[1.1.0]: https://github.com/rust-lang/rustup/releases/tag/1.1.0

Contributors: Aarthi Janakiraman, Alex Burka, Alex Crichton, bors,
Brian Anderson, Christian Muirhead, Christopher Armstrong, Daniel
Lockyer, Diggory Blake, Evgenii Pashkin, Grissiom, James Elford, Luca
Bruno, Lyuha, Manish Goregaokar, Marc-Antoine Perennou, Marco A L
Barbosa, Mikhail Pak, Nick Cameron, polonez, Sam Marshall, Steve
Klabnik, Tomáš Hübelbauer, topecongiro, Wang Xuerui

## [1.0.0] - 2016-12-15

- [Statically link MSVC CRT](https://github.com/rust-lang/rustup/pull/843)
- [Upgrade ~/.multirust correctly from rustup-init](https://github.com/rust-lang/rustup/pull/858)

[1.0.0]: https://github.com/rust-lang/rustup/releases/tag/1.0.0

Contributors: Alex Crichton, Andrew Koroluk, Arch, benaryorg, Benedikt Reinartz,
Björn Steinbrink, bors, Boutin, Michael, Brian Anderson, Cam Swords, Chungmin
Park, Corey Farwell, Daniel Keep, David Salter, Diggory Blake, Drew Fisher,
Erick Tryzelaar, Florian Gilcher, geemili, Guillaume Fraux, Ivan Nejgebauer,
Ivan Petkov, Jacob Shaffer, Jake Goldsborough, James Lucas, Jeremiah Peschka,
jethrogb, Jian Zeng, Jimmy Cuadra, Joe Wilm, Jorge Aparicio, Josh Machol, Josh
Stone, Julien Blanchard, Kai Noda, Kai Roßwag, Kamal Marhubi, Kevin K, Kevin
Rauwolf, Kevin Yap, Knight, leonardo.yvens, llogiq, Marco A L Barbosa, Martin
Pool, Matt Brubeck, mdinger, Michael DeWitt, Mika Attila, Nate Mara, NODA, Kai,
Oliver Schneider, Patrick Reisert, Paul Padier, Ralph Giles, Raphael Cohn, Ri,
Ricardo Martins, Ryan Havar, Ryan Kung, Severen Redwood, Tad Hardesty, Taylor
Cramer, theindigamer, Tim Neumann, Tobias Bucher, trolleyman, Vadim
Petrochenkov, Virgile Andreani, V Jackson, Vladimir, Wang Xuerui, Wayne Warren,
Wesley Moore, Yasushi Abe, Y. T. Chung

## [0.7.0] - 2016-12-11

- [Correctly "detect" host endianness on MIPS](https://github.com/rust-lang/rustup/pull/802)
- [Add powershell completions](https://github.com/rust-lang/rustup/pull/801)
- [Update toolchain used to build rustup](https://github.com/rust-lang/rustup/pull/741)
- [Support probing MIPS64 n64 targets](https://github.com/rust-lang/rustup/pull/815)
- [Support MIPS architectures in rustup-init.sh](https://github.com/rust-lang/rustup/pull/825)
- [Automatically detect NetBSD during standard install](https://github.com/rust-lang/rustup/pull/824)
- [Fix symlink creation on windows](https://github.com/rust-lang/rustup/pull/823)
- [Search PATH for binaries run by `rustup run`](https://github.com/rust-lang/rustup/pull/822)
- [Recursive tool invocations should invoke the proxy, not the tool directly](https://github.com/rust-lang/rustup/pull/812)
- [Upgrade error-chain](https://github.com/rust-lang/rustup/pull/841)
- [Add FAQ entry for downloading Rust source](https://github.com/rust-lang/rustup/pull/840)
- [Rename ~/.multirust to ~/.rustup](https://github.com/rust-lang/rustup/pull/830)
- [Remove some codegen hacks](https://github.com/rust-lang/rustup/pull/850)
- [Update libc for MIPS64 host builds](https://github.com/rust-lang/rustup/pull/847)
- [Default to MSVC on Windows](https://github.com/rust-lang/rustup/pull/842)

[0.7.0]: https://github.com/rust-lang/rustup/releases/tag/0.7.0

Contributors: Alex Crichton, Arch, bors, Brian Anderson, Diggory Blake, Kai
Roßwag, Kevin K, Oliver Schneider, Ryan Havar, Tobias Bucher, Wang Xuerui

## [0.6.5] - 2016-11-04

- [Update bundled curl code](https://github.com/rust-lang/rustup/pull/790)
- [Remove old zsh completions](https://github.com/rust-lang/rustup/pull/779)
- [Fix two small typos in the error descriptions](https://github.com/rust-lang/rustup/pull/788)
- [Update README](https://github.com/rust-lang/rustup/pull/782)
- [Fix name of bash completion directory](https://github.com/rust-lang/rustup/pull/780)

[0.6.5]: https://github.com/rust-lang/rustup/releases/tag/0.6.5

Contributors: Alex Crichton, Björn Steinbrink, Brian Anderson, Jian Zeng, Matt
Brubeck

## [0.6.4] - 2016-10-24

- [making rustup prepend cargo bin to path instead of append](https://github.com/rust-lang/rustup/pull/707)
- [Use released version of rustls dependency](https://github.com/rust-lang/rustup/pull/711)
- [Update OpenSSL](https://github.com/rust-lang/rustup/pull/733)
- [Made outputting of ANSI terminal escapes codes defensive](https://github.com/rust-lang/rustup/pull/725)
- [Adjusted rustup-init.sh need_cmd to add uname and remove printf](https://github.com/rust-lang/rustup/pull/723)
- [Update to error-chain 0.5.0 to allow optional backtrace](https://github.com/rust-lang/rustup/pull/591)
- [Fix variable naming in rustup-init.sh](https://github.com/rust-lang/rustup/pull/737)
- [Update clap to fix --help formatting](https://github.com/rust-lang/rustup/pull/738)
- [Add an FAQ entry about troubles with antivirus](https://github.com/rust-lang/rustup/pull/739)
- [Clarify how rustup toolchain installation works on Windows](https://github.com/rust-lang/rustup/pull/744)
- [Do not interpret commas when using "rustup run"](https://github.com/rust-lang/rustup/pull/752)
- [Fix local declarations for zsh completions](https://github.com/rust-lang/rustup/pull/753)
- [Fix checksum failures](https://github.com/rust-lang/rustup/pull/759)
- [Treat an empty `CARGO_HOME` the same as an unset `CARGO_HOME`](https://github.com/rust-lang/rustup/pull/767)
- [Check stdout is a tty before using terminal features](https://github.com/rust-lang/rustup/pull/772)
- [Add completion generation for zsh, bash and fish shells](https://github.com/rust-lang/rustup/pull/773)

[0.6.4]: https://github.com/rust-lang/rustup/releases/tag/0.6.4

Contributors: Alex Crichton, Andrew Koroluk, Brian Anderson, Chungmin Park,
Diggory Blake, Guillaume Fraux, Jake Goldsborough, jethrogb, Kamal Marhubi,
Kevin K, Kevin Rauwolf, Raphael Cohn, Ricardo Martins

## [0.6.3] - 2016-08-28

- [Disable anti-sudo check](https://github.com/rust-lang/rustup/pull/698)
- [Fixed CI toolchain pinning](https://github.com/rust-lang/rustup/pull/696)

[0.6.3]: https://github.com/rust-lang/rustup/releases/tag/0.6.3

Contributors: Brian Anderson

## [0.6.2] - 2016-08-27

- [Add basic autocompletion for Zsh](https://github.com/rust-lang/rustup/pull/689)
- [Sort toolchains by semantic version](https://github.com/rust-lang/rustup/pull/688)

[0.6.2]: https://github.com/rust-lang/rustup/releases/tag/0.6.2

Contributors: Brian Anderson, Diggory Blake, Knight, Marco A L Barbosa

## [0.6.1] - 2016-08-24

- [Fix mysterious crash on OS X 10.10+](https://github.com/rust-lang/rustup/pull/684)
- [Fix `component remove` command and add a test for it](https://github.com/rust-lang/rustup/pull/683)

[0.6.1]: https://github.com/rust-lang/rustup/releases/tag/0.6.1

Contributors: Brian Anderson, Diggory Blake

## [0.6.0] - 2016-08-23

- [Print rustup version after update](https://github.com/rust-lang/rustup/pull/614)
- [Don't spawn processes for copying](https://github.com/rust-lang/rustup/pull/630)
- [Upgrade error-chain to 0.3](https://github.com/rust-lang/rustup/pull/636)
- [Support telemetry with lots of output](https://github.com/rust-lang/rustup/pull/645)
- [Remove empty directories after component uninstall](https://github.com/rust-lang/rustup/pull/634)
- [Update rustup-init.sh for powerpc](https://github.com/rust-lang/rustup/pull/647)
- [Switch builds to current nightly toolchain](https://github.com/rust-lang/rustup/pull/651)
- [Add a WIP MSI installer](https://github.com/rust-lang/rustup/pull/635)
- [Add `--path` and `--nonexistent` options to `rustup override unset`](https://github.com/rust-lang/rustup/pull/650)
- [Add `component` subcommand](https://github.com/rust-lang/rustup/pull/659)

[0.6.0]: https://github.com/rust-lang/rustup/releases/tag/0.6.0

Contributors: Alex Crichton, Brian Anderson, Diggory Blake, Ivan Nejgebauer Josh
Machol, Julien Blanchard, Patrick Reisert, Ri, Tim Neumann

## [0.5.0] - 2016-07-30

- [List custom toolchains in `rustup show`](https://github.com/rust-lang/rustup/pull/620)
- [Add a usage example for local builds](https://github.com/rust-lang/rustup/pull/622)
- [Read/Write impl rework for rustls](https://github.com/rust-lang/rustup/pull/592)
- [Introduce `+TOOLCHAIN` syntax for proxies](https://github.com/rust-lang/rustup/pull/615)
- [Add `rustup man`](https://github.com/rust-lang/rustup/pull/616)
- [Try detecting sudo when running `rustup-init`](https://github.com/rust-lang/rustup/pull/617)
- [Handle active custom toolchain in `rustup show`](https://github.com/rust-lang/rustup/pull/621)

[0.5.0]: https://github.com/rust-lang/rustup/releases/tag/0.5.0

Contributors: Brian Anderson, Cam Swords, Daniel Keep, Diggory Blake,
Florian Gilcher, Ivan Nejgebauer, theindigamer

## [0.4.0] - 2016-07-22

- [Improve rustls CA certificate loading](https://github.com/rust-lang/rustup/pull/585)
- [Detect ARMv7 CPUs without NEON extensions and treat as ARMv6](https://github.com/rust-lang/rustup/pull/593)
- [Allow any toolchain to be specified as the default during rustup installation](https://github.com/rust-lang/rustup/pull/586)
- [Add details about updating rustup to README](https://github.com/rust-lang/rustup/pull/590)
- [Update libbacktrace to generate less filesystem thrashing on Windows](https://github.com/rust-lang/rustup/pull/604)
- [Update gcc dep to fix building on MSVC](https://github.com/rust-lang/rustup/pull/605)
- [Remove the multirust binary](https://github.com/rust-lang/rustup/pull/606)
- [Use the env_proxy crate for proxy environment variable handling](https://github.com/rust-lang/rustup/pull/598)
- [Set system-specific dynamic loader env var for command execution](https://github.com/rust-lang/rustup/pull/600)
- [Hide telemetry command from top level help](https://github.com/rust-lang/rustup/pull/601)
- [Add the "no-self-update" feature](https://github.com/rust-lang/rustup/pull/602)
- [Update to error-chain 0.2.2](https://github.com/rust-lang/rustup/pull/609)
- [Add HTTP proxy documentation to README](https://github.com/rust-lang/rustup/pull/610)

[0.4.0]: https://github.com/rust-lang/rustup/releases/tag/0.4.0

Contributors: Alex Crichton, Brian Anderson, Ivan Nejgebauer, Jimmy
Cuadra, Martin Pool, Wesley Moore

## [0.3.0] - 2016-07-14

- [Teach rustup to download manifests from the `/staging/` directory](https://github.com/rust-lang/rustup/pull/579).
- [Treat all HTTP client errors the same](https://github.com/rust-lang/rustup/pull/578).
- [Remove winapi replacement](https://github.com/rust-lang/rustup/pull/577).
- [Remove toolchain directory if initial toolchain install fails](https://github.com/rust-lang/rustup/pull/574).
- [Fallback to old download methods if server returns 403](https://github.com/rust-lang/rustup/pull/573).
- [Add preliminary rustls support](https://github.com/rust-lang/rustup/pull/572).
- [Add a hack to remediate checksum failure issues](https://github.com/rust-lang/rustup/pull/562).
- [Move error-chain out of tree](https://github.com/rust-lang/rustup/pull/564).
- [Remove uses of subcommand synonyms in the examples](https://github.com/rust-lang/rustup/pull/560).
- [Add `--yes` as alias for `-y`](https://github.com/rust-lang/rustup/pull/563).
- [Remove unavailable toolchains from `target list`](https://github.com/rust-lang/rustup/pull/553).
- [Add powerpc builds](https://github.com/rust-lang/rustup/pull/534).
- [Fix help text for `rustup update`](https://github.com/rust-lang/rustup/pull/552).
- [Remove noisy "rustup is up to date" message](https://github.com/rust-lang/rustup/pull/550).
- [Fix references to non-existent `.rustup` directory](https://github.com/rust-lang/rustup/pull/545).
- [When listing toolchains only list directories](https://github.com/rust-lang/rustup/pull/544).
- [rustup-init: remove dependency on `file` command](https://github.com/rust-lang/rustup/pull/543).
- [Link to rustup-init.sh in README](https://github.com/rust-lang/rustup/pull/541).
- [Improve docs for `set default-host`](https://github.com/rust-lang/rustup/pull/540).

[0.3.0]: https://github.com/rust-lang/rustup/releases/tag/0.3.0

Contributors: Alex Crichton, Brian Anderson, Drew Fisher, geemili,
Ivan Petkov, James Lucas, jethrogb, Kevin Yap, leonardo.yvens, Michael
DeWitt, Nate Mara, Virgile Andreani

## [0.2.0] - 2016-06-21

- [Indicate correct path to remove in multirust upgrade instructions](https://github.com/rust-lang/rustup/pull/518).
- [Bring back optional hyper with proxy support](https://github.com/rust-lang/rustup/pull/532).
- ['default' and 'update' heuristics for bare triples](https://github.com/rust-lang/rustup/pull/516).
- [Change upstream via \$RUSTUP_DIST_SERVER](https://github.com/rust-lang/rustup/pull/521).
- [Fail with a nicer error message if /tmp is mounted noexec](https://github.com/rust-lang/rustup/pull/523).
- [Remove printfs from ~/.cargo/env](https://github.com/rust-lang/rustup/pull/527).
- [Reduce margin in installer text to 79 columns](https://github.com/rust-lang/rustup/pull/526).
- [Fix typos](https://github.com/rust-lang/rustup/pull/519).
- [Fix missing curly braces in error-chain docs](https://github.com/rust-lang/rustup/pull/522).
- [Fix downloads of builds without v2 manifests](https://github.com/rust-lang/rustup/pull/515).
- [Explain toolchains in `help install`](https://github.com/rust-lang/rustup/pull/496).
- [Compile on stable Rust](https://github.com/rust-lang/rustup/pull/476).
- [Fix spelling mistakes](https://github.com/rust-lang/rustup/pull/489).
- [Fix the toolchain command synonyms](https://github.com/rust-lang/rustup/pull/477).
- [Configurable host triples](https://github.com/rust-lang/rustup/pull/421).
- [Use a .toml file to store settings](https://github.com/rust-lang/rustup/pull/420).
- [Point PATH to toolchain/bin on Windows](https://github.com/rust-lang/rustup/pull/402).
- [Remove extra '.' in docs](https://github.com/rust-lang/rustup/pull/472).

[0.2.0]: https://github.com/rust-lang/rustup/releases/tag/0.2.0

Contributors: Alex Crichton, benaryorg, Benedikt Reinartz, Boutin,
Michael, Brian Anderson, Diggory Blake, Erick Tryzelaar, Ivan
Nejgebauer, Jeremiah Peschka, Josh Stone, Knight, mdinger, Ryan Kung,
Tad Hardesty

## [0.1.12] - 2016-05-12

- [Don't install when multirust metadata exists](https://github.com/rust-lang/rustup/pull/456).

[0.1.12]: https://github.com/rust-lang/rustup/releases/tag/0.1.12

## [0.1.11] - 2016-05-12

- [Actually dispatch the `rustup install` command](https://github.com/rust-lang/rustup/pull/444).
- [Migrate to libcurl instead of hyper](https://github.com/rust-lang/rustup/pull/434).
- [Add error for downloading bogus versions](https://github.com/rust-lang/rustup/pull/428).

[0.1.11]: https://github.com/rust-lang/rustup/releases/tag/0.1.11

## [0.1.10] - 2016-05-09

- [Multiple cli improvements](https://github.com/rust-lang/rustup/pull/419).
- [Support HTTP protocol again](https://github.com/rust-lang/rustup/pull/431).
- [Improvements to welcome screen](https://github.com/rust-lang/rustup/pull/418).
- [Don't try to update non-tracking channels](https://github.com/rust-lang/rustup/pull/425).
- [Don't panic when NativeSslStream lock is poisoned](https://github.com/rust-lang/rustup/pull/429).
- [Fix multiple issues in schannel bindings](https://github.com/sfackler/schannel-rs/pull/1)

[0.1.10]: https://github.com/rust-lang/rustup/releases/tag/0.1.10

## [0.1.9] - 2016-05-07

- [Do TLS hostname verification](https://github.com/rust-lang/rustup/pull/400).
- [Expand `rustup show`](https://github.com/rust-lang/rustup/pull/406).
- [Add `rustup doc`](https://github.com/rust-lang/rustup/pull/403).
- [Refuse to install if it looks like other Rust installations are present](https://github.com/rust-lang/rustup/pull/408).
- [Update www platform detection for FreeBSD](https://github.com/rust-lang/rustup/pull/399).
- [Fix color display during telemetry capture](https://github.com/rust-lang/rustup/pull/394).
- [Make it less of an error for the self-update hash to be wrong](https://github.com/rust-lang/rustup/pull/372).

[0.1.9]: https://github.com/rust-lang/rustup/releases/tag/0.1.9

## [0.1.8] - 2016-04-28

- [Initial telemetry implementation (disabled)](https://github.com/rust-lang/rustup/pull/289)
- [Add hash to `--version`](https://github.com/rust-lang/rustup/pull/347)
- [Improve download progress](https://github.com/rust-lang/rustup/pull/355)
- [Completely overhaul error handling](https://github.com/rust-lang/rustup/pull/358)
- [Add armv7l support to www](https://github.com/rust-lang/rustup/pull/359)
- [Overhaul website](https://github.com/rust-lang/rustup/pull/363)

[0.1.8]: https://github.com/rust-lang/rustup/releases/tag/0.1.8

## [0.1.7] - 2016-04-17

- [Fix overrides for Windows root directories](https://github.com/rust-lang/rustup/pull/317).
- [Remove 'multirust' binary and rename crates](https://github.com/rust-lang/rustup/pull/312).
- [Pass rustup-setup.sh arguments to rustup-setup](https://github.com/rust-lang/rustup/pull/325).
- [Don't open /dev/tty if passed -y](https://github.com/rust-lang/rustup/pull/334).
- [Add interactive install, `--default-toolchain` argument](https://github.com/rust-lang/rustup/pull/293).
- [Rename rustup-setup to rustu-init](https://github.com/rust-lang/rustup/pull/303).

[0.1.7]: https://github.com/rust-lang/rustup/releases/tag/0.1.7
