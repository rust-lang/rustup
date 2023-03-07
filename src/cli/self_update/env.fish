# rustup shell setup
if not contains "{cargo_bin}" $PATH
    # Prepending path in case a system-installed rustc needs to be overridden
    set -x PATH "{cargo_bin}" $PATH
end
