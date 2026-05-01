{
  description = "ffw-slot-selector dev shell";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs@{ flake-parts, rust-overlay, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

      perSystem = { system, pkgs, ... }: {
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        devShells.default = pkgs.mkShell {
          packages = [
            (pkgs.rust-bin.stable.latest.default.override {
              targets = [ "wasm32-unknown-unknown" ];
            })
            pkgs.openapi-generator-cli
            pkgs.sqlx-cli
            pkgs.wasm-bindgen-cli
          ];
        };
      };
    };
}
