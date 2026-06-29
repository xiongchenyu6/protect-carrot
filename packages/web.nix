# Prebuilt 保卫萝卜 (Protect the Carrot) WebGPU/wasm bundle.
#
# Instead of compiling the Bevy game from source (slow — minutes of Rust plus
# wasm-opt, and a multi-GB target dir), this fetches the static web bundle that
# CI publishes to GitHub Releases (.github/workflows/release-wasm.yml). Every
# deploy target just downloads the tarball, so `nixos-rebuild` is fast and needs
# no Rust toolchain.
#
# The release coordinates live in ./web-release.json, which CI rewrites whenever
# a new `v*` tag is built. To rebuild from source instead, use the
# `web-source` package (packages/web-source.nix).
#
#   nix build .#web           # download the pinned prebuilt bundle
#   nix build .#web-source    # compile from source
{
  lib,
  stdenvNoCC,
  fetchurl,
}:
let
  release = lib.importJSON ./web-release.json;
in
stdenvNoCC.mkDerivation {
  pname = "protect-carrot-web";
  version = lib.removePrefix "v" release.tag;

  src = fetchurl {
    url = release.url;
    sha256 = release.sha256;
  };

  # The tarball is produced with `tar -C web -czf … .`, so its members are the
  # bundle files (index.html, protect_carrot_bg.wasm, assets/…) at the archive
  # root with no wrapping directory.
  unpackPhase = ''
    runHook preUnpack
    mkdir -p bundle
    tar -xzf "$src" -C bundle
    runHook postUnpack
  '';
  sourceRoot = "bundle";

  dontConfigure = true;
  dontBuild = true;

  installPhase = ''
    runHook preInstall
    mkdir -p $out/share/protect-carrot
    cp -r ./* $out/share/protect-carrot/
    runHook postInstall
  '';

  meta = with lib; {
    description = "保卫萝卜 (Protect the Carrot) — prebuilt WebGPU/wasm web bundle";
    homepage = "https://github.com/xiongchenyu6/protect-carrot";
    license = licenses.cc0;
    platforms = platforms.all;
    maintainers = [ ];
  };
}
