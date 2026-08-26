# Online Tests


## Setup

```sh
export RUSTUP_HOME=/tmp/tuf-home CARGO_HOME=/tmp/tuf-home
rm -rf /tmp/tuf-home && mkdir -p /tmp/tuf-home
target/debug/rustup-init -y --no-modify-path --default-toolchain none
/tmp/tuf-home/bin/rustup set auto-self-update disable
```

## Test: stable

```sh
curl -o /tmp/tuf-root.json https://storage.googleapis.com/rf-signing-rustup/metadata/1.root.json

export RUSTUP_TUF_ENABLE=on
export RUSTUP_TUF_IGNOREDATE=2026-09-26T17:34:11+00:00 
export RUSTUP_TUF_DIST_SERVER=https://storage.googleapis.com/rf-signing-rustup 
export RUSTUP_TUF_ROOT=/tmp/tuf-root.json 

/tmp/tuf-home/bin/rustup toolchain install stable --profile minimal
/tmp/tuf-home/bin/rustup run stable rustc --version
```

# Local Tests

## Setup

```sh
export RUSTUP_HOME=/tmp/tuf-home CARGO_HOME=/tmp/tuf-home
rm -rf /tmp/tuf-home && mkdir -p /tmp/tuf-home
target/debug/rustup-init -y --no-modify-path --default-toolchain none
/tmp/tuf-home/bin/rustup set auto-self-update disable

export RUSTUP_TUF_ENABLE=on 
export RUSTUP_TUF_IGNOREDATE=2026-09-26T17:34:11+00:00
export RUSTUP_TUF_DIST_SERVER=./src/tuf/tests/repo/tuf
export RUSTUP_TUF_ROOT=./src/tuf/tests/repo/tuf/metadata/1.root.json

export RUSTUP_LOG=rustup::tuf=trace,rustup::download=debug,rustup::dist=info 

# current stable
/tmp/tuf-home/bin/rustup toolchain install stable --profile minimal
/tmp/tuf-home/bin/rustup run stable rustc --version

# versioned stable
/tmp/tuf-home/bin/rustup toolchain install 1.98.1 --profile minimal
/tmp/tuf-home/bin/rustup run 1.98.1 rustc --version

#  current beta
/tmp/tuf-home/bin/rustup toolchain install beta --profile minimal
/tmp/tuf-home/bin/rustup run beta rustc --version

# current nightly
/tmp/tuf-home/bin/rustup toolchain install nightly --profile minimal
/tmp/tuf-home/bin/rustup run nightly rustc --version

# dated nightly, newest in the repo
/tmp/tuf-home/bin/rustup toolchain install nightly-2026-09-16 --profile minimal
/tmp/tuf-home/bin/rustup run nightly-2026-09-16 rustc --version

# dated nightly, older
/tmp/tuf-home/bin/rustup toolchain install nightly-2026-06-01 --profile minimal
/tmp/tuf-home/bin/rustup run nightly-2026-06-01 rustc --version

# dated stable, newest in the repo
/tmp/tuf-home/bin/rustup toolchain install stable-2026-09-03 --profile minimal
/tmp/tuf-home/bin/rustup run stable-2026-09-03 rustc --version

# dated beta, newest in the repo
/tmp/tuf-home/bin/rustup toolchain install beta-2026-09-11 --profile minimal
/tmp/tuf-home/bin/rustup run beta-2026-09-11 rustc --version
```

