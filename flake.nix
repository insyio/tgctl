{
  description = "tgctl — declarative Telegram group management";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        tgctl = pkgs.rustPlatform.buildRustPackage {
          pname = "tgctl";
          version = "0.1.0";

          src = self;

          cargoLock.lockFile = ./Cargo.lock;

          meta = {
            description = "Declarative Telegram group management";
            homepage = "https://github.com/insyio/tgctl";
            license = pkgs.lib.licenses.mit;
            mainProgram = "tgctl";
          };
        };
      in
      {
        packages = {
          default = tgctl;
          tgctl = tgctl;
        };

        overlays.default = final: _prev: {
          tgctl = self.packages.${final.system}.tgctl;
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ tgctl ];
        };
      }
    ) // {
      overlays.default = final: _prev: {
        tgctl = self.packages.${final.system}.tgctl;
      };
    };
}
