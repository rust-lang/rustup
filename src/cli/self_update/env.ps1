# rustup shell setup
if (-not ":${env:PATH}:".Contains(":{rustup_bin}:")) {
    ${env:PATH} = "{rustup_bin}:${env:PATH}";
}
