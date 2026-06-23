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
        protect-carrot-web = final.callPackage ./packages/web.nix {};
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
          # Web package: builds the game to wasm and produces static files.
          packages.web = pkgs.callPackage ./packages/web.nix {};

          # Default package points to the web build.
          packages.default = config.packages.web;

          devShells.default =
            with pkgs;
            mkShell.override { stdenv = pkgs.clangStdenv; } {
              RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
              RUST_BACKTRACE = 1;

              # Bevy needs these libs discoverable at runtime on Linux
              # (GPU via Vulkan, windowing via X11/Wayland, audio via ALSA).
              LD_LIBRARY_PATH = lib.makeLibraryPath [
                vulkan-loader
                wayland
                libxkbcommon
                xorg.libX11
                xorg.libXcursor
                xorg.libXi
                xorg.libXrandr
                alsa-lib
                udev
              ];

              buildInputs = [
                vulkan-loader
                alsa-lib
                udev
                libxkbcommon
                wayland
                xorg.libX11
                xorg.libXcursor
                xorg.libXi
                xorg.libXrandr
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
                wasm-bindgen-cli
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
