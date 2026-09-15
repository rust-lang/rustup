# rustup shell setup
if not contains "{rustup_bin}" $PATH
    # Prepending path in case a system-installed rustc needs to be overridden
    set -x PATH "{rustup_bin}" $PATH
end
