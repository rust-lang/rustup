# rustup shell setup
if (-not ":${env:PATH}:".Contains(":{cargo_bin}:")) {
    ${env:PATH} = "{cargo_bin}:${env:PATH}";
}
