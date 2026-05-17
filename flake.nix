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

  outputs =
    inputs@{ flake-parts, rust-overlay, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      flake.nixosModules.default =
        {
          config,
          lib,
          pkgs,
          ...
        }:
        let
          cfg = config.services.ffw-slot-selector;
        in
        {
          options.services.ffw-slot-selector = {
            enable = lib.mkEnableOption "ffw-slot-selector";
            package = lib.mkOption {
              type = lib.types.package;
              default = inputs.self.packages.${pkgs.stdenv.hostPlatform.system}.default;
              description = "The ffw-slot-selector package to use.";
            };
            port = lib.mkOption {
              type = lib.types.port;
              default = 3000;
              description = "Port to listen on.";
            };
            dataDir = lib.mkOption {
              type = lib.types.str;
              default = "/var/lib/ffw-slot-selector";
              description = "Directory for the SQLite database.";
            };
            logLevel = lib.mkOption {
              type = lib.types.str;
              default = "info";
              description = "Log level (RUST_LOG format, e.g. info, debug, warn).";
            };
            environmentFile = lib.mkOption {
              type = lib.types.nullOr lib.types.path;
              default = null;
              description = ''
                Path to a file containing environment variables in KEY=value
                format. Use this to inject secrets such as SMTP_PASSWORD
                without putting them in the Nix store. Compatible with
                sops-nix: set sops.secrets.ffw-slot-selector.format = "dotenv"
                and point this option at the rendered secret path.
              '';
            };
          };

          config = lib.mkIf cfg.enable {
            systemd.services.ffw-slot-selector = {
              description = "ffw-slot-selector";
              wantedBy = [ "multi-user.target" ];
              after = [ "network.target" ];
              environment = {
                DATABASE_URL = "sqlite:${cfg.dataDir}/data.db";
                RUST_LOG = cfg.logLevel;
              };
              serviceConfig = {
                ExecStart = "${cfg.package}/bin/server --port ${toString cfg.port}";
                EnvironmentFile = lib.optional (cfg.environmentFile != null) cfg.environmentFile;
                StateDirectory = "ffw-slot-selector";
                DynamicUser = true;
                Restart = "on-failure";

                # Filesystem
                ProtectSystem = "strict";
                ProtectHome = true;
                ReadWritePaths = [ cfg.dataDir ];
                PrivateTmp = true;

                # Capabilities & syscalls
                CapabilityBoundingSet = "";
                NoNewPrivileges = true;
                RestrictSUIDSGID = true;
                SystemCallFilter = [
                  "@system-service"
                  "~@privileged"
                ];
                SystemCallArchitectures = "native";

                # Network & misc
                PrivateDevices = true;
                ProtectKernelTunables = true;
                ProtectKernelModules = true;
                ProtectControlGroups = true;
                RestrictNamespaces = true;
                LockPersonality = true;
                MemoryDenyWriteExecute = true;
              };
            };
          };
        };

      perSystem =
        { system, pkgs, ... }:
        let
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            targets = [ "wasm32-unknown-unknown" ];
          };
          tools = [
            rustToolchain
            pkgs.openapi-generator-cli
            pkgs.sqlx-cli
            pkgs.wasm-bindgen-cli
          ];
        in
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };

          packages.default = pkgs.rustPlatform.buildRustPackage {
            pname = "ffw-slot-selector";
            version = "0.1.0";
            src = pkgs.lib.cleanSource ./.;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = tools;
            preBuild = ''
              cargo build -p frontend --target wasm32-unknown-unknown --release
              wasm-bindgen \
                --target web \
                --out-dir frontend/static \
                --out-name frontend \
                target/wasm32-unknown-unknown/release/frontend.wasm
            '';
            cargoBuildFlags = [
              "-p"
              "server"
            ];
            doCheck = false;
          };

          devShells.default = pkgs.mkShell {
            packages = tools ++ [
              pkgs.qrencode # gen_qrcodes.sh
              pkgs.typst # labels.typ
            ];
          };
        };
    };
}
