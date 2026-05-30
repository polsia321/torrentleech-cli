{
  description = "Command line client for TorrentLeech";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      forAllSystems = f: nixpkgs.lib.genAttrs
        [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ]
        (system: f nixpkgs.legacyPackages.${system});
    in {
      packages = forAllSystems (pkgs: {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "torrentleech-cli";
          version = self.shortRev or self.dirtyShortRev or "dev";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          GITHUB_SHA = self.rev or self.dirtyRev or self.shortRev or "unknown";
        };
      });

      apps = forAllSystems (pkgs: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.system}.default}/bin/tl";
        };
      });
    };
}
