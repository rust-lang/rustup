# Configuration

Rustup has a settings file in [TOML](https://github.com/toml-lang/toml) format
at `${RUSTUP_HOME}/settings.toml`. The schema for this file is not part of the
public interface for rustup - the rustup CLI should be used to query and set
settings.

On Unix operating systems a fallback settings file is consulted for some
settings. This fallback file is located at `/etc/rustup/settings.toml` and
currently can define only `default_toolchain`.
