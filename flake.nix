# This is a cheap nix flake for direnv use for developing
# Rustup if you are running on NixOS.
#
# We deliberately don't commit a flake.lock because we only
# provide this for developers, not as a way to have rustup
# built for NixOS.

{
  inputs = { flake-utils.url = "github:numtide/flake-utils"; };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let pkgs = nixpkgs.legacyPackages.${system};
      in {
        devShell = pkgs.mkShell {
          buildInputs = with pkgs; [
            stdenv
            openssl
            pkg-config
          ];
        };
      });
}
