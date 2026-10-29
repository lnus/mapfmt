{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = {
    self,
    nixpkgs,
  }: let
    inherit (nixpkgs) lib;
    forAllSystems = f:
      lib.genAttrs ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"]
      (system: f nixpkgs.legacyPackages.${system});

    cargoToml = lib.importTOML ./Cargo.toml;
  in {
    packages = forAllSystems (pkgs: {
      default = pkgs.rustPlatform.buildRustPackage {
        pname = cargoToml.package.name;
        inherit (cargoToml.package) version;

        # Only the build inputs, so doc edits don't rebuild.
        src = lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.unions [./src ./Cargo.toml ./Cargo.lock];
        };
        cargoLock = {
          lockFile = ./Cargo.lock;
          # Update after changing the tree-sitter-mapfile tag in Cargo.toml.
          outputHashes."tree-sitter-mapfile-0.3.0" = "sha256-or7pdG79b84C6Fx2SWGZtE0W569vrwUbZEq6wJmXSGs=";
        };

        meta = {
          inherit (cargoToml.package) description;
          homepage = cargoToml.package.repository;
          license = lib.licenses.mit;
          mainProgram = "mapfmt";
        };
      };
    });

    devShells = forAllSystems (pkgs: {
      default = pkgs.mkShell {
        packages = with pkgs; [
          cargo
          rustc
          clippy
          rustfmt
          rust-analyzer
        ];
        RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
        # Debug builds of the grammar's parser.c warn about _FORTIFY_SOURCE without -O.
        hardeningDisable = ["fortify"];
      };
    });
  };
}
