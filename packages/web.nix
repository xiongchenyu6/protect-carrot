# Nix derivation for 保卫萝卜 WebGPU/wasm web build.
#
# Builds the game from source and produces a directory of static files ready
# to be served by any HTTP server (Nginx, Caddy, etc.).
#
# Usage:
#   nix build .#web          # build the web package
#   nix build .#web --json   # get the output path
#
# The result contains:
#   index.html, protect_carrot.js, protect_carrot_bg.wasm,
#   assets/, and pre-compressed .br/.gz variants for wasm/js.
{
  lib,
  rustPlatform,
  pkg-config,
  wasm-bindgen-cli,
  binaryen,
  brotli,
  curl,
  lld,
  stdenv,
  openssl,
  # Bevy native deps are NOT needed for wasm32-unknown-unknown target,
  # but cargo may still resolve the lock file. We provide them so the
  # dependency graph resolves, even though they're not linked into the wasm.
  vulkan-loader,
  alsa-lib,
  udev,
  wayland,
  libxkbcommon,
  xorg,
  pkgs,
}: let
  # Match the wasm-bindgen version pinned in Cargo.toml.
  wasmBindgenVersion = "0.2.121";
in
  rustPlatform.buildRustPackage rec {
    pname = "protect-carrot-web";
    version = "0.1.0";
    src = ./..;

    cargoLock = {
      lockFile = ../Cargo.lock;
      # Allow building without a git repo.
      outputHashes = {};
    };

    # Tell Cargo we're building for wasm32-unknown-unknown.
    # The native libs above are just so `cargo check` resolves the lock;
    # they won't be linked into the wasm binary.
    CARGO_BUILD_TARGET = "wasm32-unknown-unknown";

    # Bevy's build.rs probes for system libs even on wasm targets via cfg
    # checks; provide them so the build script succeeds.
    BUILD_NO_DEFAULT_FEATURES = false;

    nativeBuildInputs = [
      pkg-config
      wasm-bindgen-cli
      binaryen
      brotli
      curl
      lld # wasm32-unknown-unknown linker (.cargo config sets linker = lld)
    ];

    buildInputs = [
      openssl
      vulkan-loader
      alsa-lib
      udev
      wayland
      libxkbcommon
      xorg.libX11
      xorg.libXcursor
      xorg.libXi
      xorg.libXrandr
    ];

    # Skip the default `cargo build` and `cargo test` phases — we do a
    # custom build below.
    doCheck = false;
    dontCargoBuild = true;
    dontCargoInstall = true;

    # Use the release profile from Cargo.toml (opt-level "z", lto, strip).
    buildPhase = ''
      runHook preBuild

      echo ">>> cargo build (release, wasm32-unknown-unknown, webgpu)"
      cargo build \
        --target wasm32-unknown-unknown \
        --release \
        --features webgpu \
        --locked

      WASM="target/wasm32-unknown-unknown/release/protect_carrot.wasm"

      echo ">>> wasm-bindgen -> web/"
      mkdir -p web
      wasm-bindgen \
        --no-typescript \
        --target web \
        --out-dir web \
        --out-name protect_carrot \
        "$WASM"

      echo ">>> wasm-opt -Oz"
      BG="web/protect_carrot_bg.wasm"
      if [ -f "$BG" ]; then
        wasm-opt -Oz --strip-debug --all-features -o "$BG.opt" "$BG" && mv "$BG.opt" "$BG" \
          || echo ">>> wasm-opt failed; shipping unoptimized wasm"
      fi

      echo ">>> copying assets"
      cp -r assets web/assets

      echo ">>> pre-compressing wasm + js"
      for ff in "web/protect_carrot_bg.wasm" "web/protect_carrot.js"; do
        if [ -f "$ff" ]; then
          gzip -9 -k -f "$ff"
          brotli -q 11 -k -f "$ff"
        fi
      done

      # Record uncompressed wasm size for the progress bar.
      if [ -f "web/protect_carrot_bg.wasm" ]; then
        stat -c%s "web/protect_carrot_bg.wasm" > "web/wasm_size.txt" 2>/dev/null \
          || wc -c < "web/protect_carrot_bg.wasm" | tr -d ' ' > "web/wasm_size.txt"
      fi

      runHook postBuild
    '';

    installPhase = ''
      runHook preInstall
      mkdir -p $out/share/protect-carrot
      cp -r web/* $out/share/protect-carrot/
      runHook postInstall
    '';

    meta = with lib; {
      description = "保卫萝卜 (Protect the Carrot) — Bevy tower-defense, WebGPU/wasm web build";
      homepage = "https://github.com/freeman.xiong/protect-carrot";
      license = licenses.cc0;
      platforms = platforms.linux ++ platforms.darwin;
      maintainers = [];
    };
  }
