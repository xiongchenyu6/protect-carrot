# SPDX-FileCopyrightText: 2021 Serokell <https://serokell.io/>
#
# SPDX-License-Identifier: CC0-1.0
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs =
    { nixpkgs, flake-parts, ... }@inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];

      # NixOS modules for easy deployment.
      flake.nixosModules.default = ./nixos/nginx-module.nix;

      # Overlay so the NixOS module can reference pkgs.protect-carrot-web.
      flake.overlays.default = final: prev: {
        protect-carrot-web = final.callPackage ./packages/web.nix { };
      };

      perSystem =
        {
          config,
          self',
          inputs',
          pkgs,
          system,
          lib,
          ...
        }:
        {
          # Web package: prebuilt wasm bundle fetched from GitHub Releases
          # (fast, no Rust toolchain). See packages/web.nix + web-release.json.
          packages.web = pkgs.callPackage ./packages/web.nix { };

          # Source build: compile the game to wasm locally. Used to produce the
          # bundle CI uploads, and for local iteration without a release.
          packages.web-source = pkgs.callPackage ./packages/web-source.nix { };

          # Default package points to the (prebuilt) web build.
          packages.default = config.packages.web;

          devShells.default =
            let
              wasm-bindgen-cli_0_2_126 = pkgs.buildWasmBindgenCli rec {
                src = pkgs.fetchCrate {
                  pname = "wasm-bindgen-cli";
                  version = "0.2.126";
                  hash = "sha256-H6Is3fiZVxZCfOMWK5dWMSrtn50VGv0sfdnsT+cTtyk=";
                };

                cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
                  inherit src;
                  inherit (src) pname version;
                  hash = "sha256-VucqkXbCi4qtQzY/HrXiDnbSURsagPsdNVMn1Tw3UiY=";
                };
              };
            in
            with pkgs;
            mkShell.override { stdenv = pkgs.clangStdenv; } {
              RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
              RUST_BACKTRACE = 1;

              # Bevy needs these libs discoverable at runtime on Linux
              # (GPU via Vulkan, windowing via Wayland, audio via ALSA).
              LD_LIBRARY_PATH = lib.makeLibraryPath [
                vulkan-loader
                wayland
                libxkbcommon
                alsa-lib
                udev
              ];

              buildInputs = [
                vulkan-loader
                alsa-lib
                udev
                libxkbcommon
                wayland
              ];
              nativeBuildInputs = [
                pkg-config
                nixfmt-rfc-style
                nixd
                rustc
                cargo
                rust-analyzer
                clippy
                openssl
                rustfmt
                # Web (WebGPU/wasm) build toolchain.
                wasm-bindgen-cli_0_2_126
                trunk
                lld # wasm32-unknown-unknown linker
                binaryen # wasm-opt (release size/perf optimization)
                brotli # pre-compress wasm/js (.br) — smaller than gzip on slow links
                # Sprite generation (tools/gen_sprites.py).
                (python3.withPackages (ps: [ ps.pillow ]))
              ];
            };
        };
    };
}
