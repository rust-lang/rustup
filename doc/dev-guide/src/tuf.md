# TUF

rustup can verify channel manifests and its own updates against a
[TUF](https://theupdateframework.io) repository before trusting them.
The support lives in `src/tuf` and is off by default; set
`RUSTUP_TUF_ENABLE=on` to turn it on.

This page describes how it is designed, where it hooks into the rest of
rustup, and how to test it.

## Design

[rust-tuf]: https://github.com/rf-signing-experiment/rust-tuf

The repository publishes two kinds of files:

- channel manifests, under `channels/`, and
- rustup's own release file and binaries, under `rustup/`.

Component tarballs are not TUF targets. They keep coming straight from the
dist server and are verified against the hashes in the manifest, so the chain
of trust is TUF, then the manifest, then the tarball.

### Modules

- `config.rs` holds `TufConfig`, read from the environment by `Cfg::new`, and
  `TufMode`. It owns the repository, opened lazily on first use.
- `repository.rs` holds `TufRepository`, a rust-tuf client with a local
  metadata cache under `RUSTUP_TUF_HOME` and a remote that is either a
  directory on disk or a URL fetched through rustup's own downloader.
- `manifest.rs` adds `manifest_v3_url` to `ChannelToolchainName`, which maps a
  toolchain onto the repository's `channels/` layout.
- `consts.rs` holds the default server and the embedded root, `root.json`.

### Settings

- `RUSTUP_TUF_ENABLE`: `off` (the default), `on`, or `warn`. With `warn` the
  metadata is still synchronized but any verification failure is reported and
  the bytes are used as served.
- `RUSTUP_TUF_SERVER`: the repository, a URL or a local directory. Defaults to
  `https://storage.googleapis.com/tufops`.
- `RUSTUP_TUF_ROOT`: a `root.json` to trust instead of the one embedded in
  rustup.
- `RUSTUP_TUF_HOME`: where the metadata cache lives. Defaults to
  `$RUSTUP_HOME/tuf`.
- `RUSTUP_TUF_IGNORE=1`: ignore every verification failure, including expiry.
- `RUSTUP_TUF_IGNOREDATE`: evaluate metadata expiry as of this instant instead
  of now, so roles that expire after it still count as valid. The comparison
  is inclusive, so use a value strictly before the earliest expiry you want to
  tolerate. Takes an RFC 3339 timestamp or a bare `YYYY-MM-DD`. Values without
  a UTC offset are silently ignored.

### Manifest layout

With TUF enabled, manifests are looked up in a per-channel layout rather than
as `channel-rust-*.toml`:

```text
channels/
├── current/{stable,beta,nightly}.toml
├── stable/<version>.toml
├── beta/<version>.toml
└── nightly/<year>/<month-day>/<channel>.toml
```

Dated toolchains of every channel resolve under `nightly/`. `manifest_v3_url`
is the single place that encodes this.

## Integration

All of rustup's network access goes through one type, `Download` in
`src/download`, and TUF is wired in there. `DownloadOptions::start` takes a
third argument, `Option<&TufConfig>`:

- `None`: the file is not a TUF target. It is fetched over plain HTTP(S) exactly
  as before.
- `Some(tuf)` with TUF disabled: the same as `None`.
- `Some(tuf)` with TUF enabled: the file is fetched through the repository
  instead. `Download` derives the target name from the URL, asks
  `TufConfig::repository` for the client, which opens and synchronizes it on
  the first call and caches it for the rest of the process, and then fetches
  and verifies the target. The verified bytes are written to the output path
  and fed to the caller's hasher and progress status just as HTTP bytes would
  be. Resume is not supported on this path; the whole target is verified in
  memory.

Above `Download`, `DownloadCfg` in `src/dist/download.rs` carries a
`&TufConfig` taken from `Cfg` and decides per download whether to pass it on:

- `dl_v2_manifest` passes it. With TUF enabled it builds the URL with
  `manifest_v3_url`, and `download_and_check` skips the `.sha256` sidecar,
  taking the hash directly from the manifest and verifying with that.
- `download`, which fetches component tarballs into the download cache, uses TUF. 
  The tarball is verified against the hash from the manifest.
- The v1 manifest and installer paths, and the sidecar fetch itself, never
  pass it either. These are the legacy behaviors.

Self-update in `src/cli/self_update.rs` passes it for both of its downloads,
the release file and the new `rustup-init`. With TUF enabled the binary is
taken from `rustup/dist/<target>/rustup-init`, which the repository publishes
as the latest release, rather than from the versioned `archive/` path.

A verification failure surfaces as `DownloadError::Tuf`, which wraps the
repository's error and is reported like any other download failure. In `warn`
mode, or with `RUSTUP_TUF_IGNORE`, the failure is logged and the bytes are
handed out unverified.

To see which path a download took, run with
`RUSTUP_LOG=rustup::tuf=debug,rustup::download=debug`. Every download logs a
`via` field of `tuf`, `direct`, or `cache`, and what it was verified against.

## What TUF adds to an update

Without TUF, `rustup update stable` fetches the manifest's sidecar, the
manifest, and then each component:

```text
rustup
  --> {dist}/channel-rust-stable.toml.sha256
  --> {dist}/channel-rust-stable.toml              checked against the sidecar
  --> {dist}/<date>/rustc-<version>-<target>.tar.xz  checked against the manifest
  --> {dist}/<date>/...                            one per component
```

With TUF enabled, the sidecar goes away and a metadata refresh plus a
verified target fetch take its place. The components are unchanged:

```text
rustup
  --> {tuf}/metadata/<n+1>.root.json               until one is not found
  --> {tuf}/metadata/timestamp.json
  --> {tuf}/metadata/<n>.snapshot.json
  --> {tuf}/metadata/<n>.targets.json
  --> {tuf}/metadata/<n>.channels-current.json     the role delegated the path
  --> {tuf}/targets/channels/current/<sha256>.stable.toml
                                                   checked against the metadata
  --> {dist}/<date>/rustc-<version>-<target>.tar.xz  checked against the manifest
  --> {dist}/<date>/...                            one per component
```

The metadata refresh happens once per process, on the first TUF download. The
root probe and the timestamp are always fetched; snapshot, targets and
delegations only when the timestamp says they changed since the cache under
`RUSTUP_TUF_HOME` was written. Later manifests in the same run only add a
target fetch.

Self-update follows the same shape. Without TUF:

```text
rustup
  --> {update}/release-stable.toml
  --> {update}/archive/<version>/<target>/rustup-init
```

With TUF, after the same metadata refresh:

```text
rustup
  --> {tuf}/targets/rustup/<sha256>.release-stable.toml
  --> {tuf}/targets/rustup/dist/<target>/<sha256>.rustup-init
```

Neither self-update download had any integrity check before, so this is the
one place TUF adds verification rather than replacing it.

## Testing

The unit tests live in `src/tuf/tests` and run against the shim repository
in `src/tuf/tests/repo`: `src/` holds published manifests and `tuf/` the
metadata, targets and test keys generated from them. Run them with:

```console
$ cargo test --features test --lib -- tuf::
```

- `config.rs` covers reading the settings.
- `download.rs` drives `Download` directly: a verified fetch, the `dist/`
  strip, unknown targets, and a copy of the shim with a tampered target,
  which `on` rejects and `warn` and `RUSTUP_TUF_IGNORE` tolerate.
- `manifest.rs` builds a real `Cfg` with TUF enabled and fetches every shim
  manifest through `dl_v2_manifest`, so the whole production path runs.

The shim's timestamp has expired, so its tests set `RUSTUP_TUF_IGNOREDATE`
to one second before that expiry, read from the metadata by `ignore_date` in
`tests/mod.rs`.

`online.rs` runs the same paths, plus the release file and the latest
`rustup-init`, against the live repository. Those tests reach the network and
are ignored by default:

```console
$ cargo test --features test --lib -- tuf::tests::online --ignored
```

To try TUF against a build of rustup, install into a scratch directory as
described in the [introduction](index.md) and export the settings before
running it:

```bash
export RUSTUP_TUF_ENABLE=on
export RUSTUP_TUF_SERVER=/path/to/a/repository   # or leave unset for the default
export RUSTUP_LOG=rustup::tuf=debug,rustup::download=debug
home/bin/rustup toolchain install stable --profile minimal
```

## Dist changes

The repository does not mirror the dist server; it publishes the same
manifests in a different tree. The old layout below is what
`static.rust-lang.org` serves today under `dist/` and `rustup/`; the new one
is what the repository signs and serves under `targets/`.

The old layout is keyed by date. The current manifest of each channel sits
at the top, and every release day has a directory holding that day's
manifests, sidecars and tarballs:

```text
dist/
├── channel-rust-stable.toml            + .sha256, .asc
├── channel-rust-beta.toml
├── channel-rust-nightly.toml
├── manifests.txt
└── <date>/                             one per release day
    ├── channel-rust-nightly.toml
    ├── channel-rust-stable.toml        on days with a stable release
    ├── channel-rust-1.98.1.toml
    ├── channel-rust-1.98.toml          partial version
    └── rustc-1.98.1-<target>.tar.xz    + .sha256, one per component

rustup/
├── release-stable.toml
├── dist/<target>/rustup-init           + .sha256, always the latest
└── archive/<version>/<target>/rustup-init
```

The new layout is keyed by channel instead, and each subtree is signed by
its own delegated role:

```text
channels/
├── current/{stable,beta,nightly}.toml  role channels-current
├── stable/1.98.1.toml, 1.98.toml       role channels-stable
├── beta/1.75-beta-2023-11-13.toml      role channels-beta
└── nightly/<year>/<month-day>/         role channels-nightly
    └── {nightly,stable,beta}.toml      every dated manifest, any channel
rustup/                                 role rustup
├── release-stable.toml
├── dist/<target>/rustup-init
├── rustup-init.sh
└── rustup-setup.sh
manifests.txt
```

What changed:

- Manifests move from `<date>/channel-rust-<name>.toml` to `<channel>/<name>.toml`.
  Their contents are unchanged, byte for byte, so the tarball URLs inside
  them still point at `dist/<date>/`.
- The `channel-rust-` prefix is gone; the directory carries that meaning.
- Dated lookups of any channel go under `nightly/<year>/<month-day>/`,
  including a dated stable or beta. This is the open question noted in
  `manifest_v3_url`: it means the nightly role signs stable and beta
  manifests, which erodes the per-channel role boundary.
- There are no `.sha256` or `.asc` sidecars and no tarballs. The repository
  holds only TOML and the two rustup binaries per target. The TUF metadata
  carries the hashes of those; components are still checked against the
  hashes in the manifest, as they always were.
- The rustup tree keeps `release-stable.toml` and `dist/<target>/`, which is
  why a TUF self-update takes the `dist/` copy, and drops `archive/`: there is
  no versioned history of binaries in the repository.
- `manifests.txt` is kept as a target at the root.
