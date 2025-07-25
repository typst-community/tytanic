# SPDX-License-Identifier: Apache-2.0
# Credits: The Typst Authors

{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    systems.url = "github:nix-systems/default";

    crane.url = "github:ipetkov/crane";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-manifest = {
      url = "https://static.rust-lang.org/dist/channel-rust-1.84.toml";
      flake = false;
    };
  };

  outputs = inputs@{ flake-parts, crane, nixpkgs, fenix, rust-manifest, self, ... }: flake-parts.lib.mkFlake { inherit inputs; } {
    systems = import inputs.systems;

    imports = [
      inputs.flake-parts.flakeModules.easyOverlay
    ];

    perSystem = { self', pkgs, lib, system, ... }:
      let
        cargoToml = lib.importTOML ./Cargo.toml;

        pname = "tytanic";
        version = cargoToml.workspace.package.version;

        rust-toolchain = (fenix.packages.${system}.fromManifestFile rust-manifest).defaultToolchain;

        # Crane-based Nix flake configuration.
        # Based on https://github.com/ipetkov/crane/blob/master/examples/trunk-workspace/flake.nix
        craneLib = (crane.mkLib pkgs).overrideToolchain rust-toolchain;

        # Tytanic files to include in the derivation.
        # Here we include Rust files.
        src = lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.unions [
            ./Cargo.toml
            ./Cargo.lock
            ./rustfmt.toml
            ./crates
            ./assets
          ];
        };

        # Tytanic derivation's args, used within crane's derivation
        # generation functions.
        commonCraneArgs = {
          inherit src pname version;

          buildInputs = [
            pkgs.openssl
          ] ++ (lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.CoreServices
            pkgs.libiconv
          ]);

          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.openssl.dev
          ];
        };

        # Derivation with just the dependencies, so we don't have to keep
        # re-building them.
        cargoArtifacts = craneLib.buildDepsOnly commonCraneArgs;

        tytanic = craneLib.buildPackage (commonCraneArgs // {
          inherit cargoArtifacts;

          nativeBuildInputs = commonCraneArgs.nativeBuildInputs ++ [
            pkgs.installShellFiles
          ];

          GEN_ARTIFACTS = "artifacts";
          TYTANIC_VERSION =
            let
              rev = self.shortRev or "dirty";
              version = cargoToml.workspace.package.version;
            in
            "${version} (${rev})";

          meta.mainProgram = "tt";
        });
      in
      {
        formatter = pkgs.nixpkgs-fmt;

        packages = {
          default = tytanic;
          tytanic-dev = self'.packages.default;
        };

        overlayAttrs = builtins.removeAttrs self'.packages [ "default" ];

        apps.default = {
          type = "app";
          program = lib.getExe tytanic;
        };

        checks = {
          tytanic-fmt = craneLib.cargoFmt commonCraneArgs;
          tytanic-clippy = craneLib.cargoClippy (commonCraneArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--workspace -- --deny warnings";
          });
          tytanic-test = craneLib.cargoTest (commonCraneArgs // {
            inherit cargoArtifacts;
            cargoTestExtraArgs = "--workspace";
          });
        };

        devShells.default = craneLib.devShell {
          checks = self'.checks;
          inputsFrom = [ tytanic ];

          buildInputs = with pkgs; [
            rust-analyzer
          ];
        };
      };
  };
}
