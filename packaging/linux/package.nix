# Nix package for OpenHID on Linux (CLI + agent + GUI/overlay).
#
# Build via the flake:
#   nix build .#openhid
#
# ## Why this doesn't suffer the #262 cargoHash churn
#
# The previous flake (removed in #262) used fetchCargoVendor, whose single
# cargoHash covers one FOD containing every dependency plus a copy of
# Cargo.lock. Because the lock embeds the local openhid* crate versions,
# every release bump invalidated the hash even when no dependency changed.
#
# This package uses rustPlatform's `cargoLock` (importCargoLock) instead:
# - crates.io dependencies are fetched individually using the checksums
#   already recorded in Cargo.lock — no manual hashes, ever.
# - git dependencies need one manual hash per repository (not per crate,
#   not per release): importCargoLock resolves `outputHashes` keys to git
#   commit SHAs, so the hashes below stay valid until a git pin actually
#   moves. A version bump of OpenHID itself changes nothing here.
#
# The only recurring maintenance is: when a git pin (gpui-component,
# ...) is bumped, update the corresponding entry below — the failing build
# prints the correct hash to paste. Nix CI watches Cargo.lock and the workspace
# manifests so that failure happens in the PR that moves the pin.
{
  lib,
  stdenv,
  rustPlatform,
  fetchgit,
  src,
  git,
  pkg-config,
  patchelf,
  versionCheckHook,
  fontconfig,
  freetype,
  libGL,
  libxkbcommon,
  wayland,
  vulkan-loader,
  libxcb,
}:

let
  # Keep documentation, CI metadata, and unrelated release tooling out of the
  # source derivation so editing them does not rebuild the application.
  source = lib.fileset.toSource {
    root = src;
    fileset = lib.fileset.unions [
      (src + "/Cargo.lock")
      (src + "/Cargo.toml")
      (src + "/LICENSE-APACHE")
      (src + "/LICENSE-MIT")
      (src + "/NOTICE")
      (src + "/crates")
      (src + "/design/icon")
      (src + "/docs/config.example.toml")
      # Workspace tests require the recorded corpus in the filtered source.
      (src + "/fixtures/devices")
      (src + "/packaging/linux/desktop")
      (src + "/packaging/linux/systemd")
      (src + "/packaging/linux/udev")
      # xtask's packaged-bins test include_str!s this; omitting it fails
      # `cargo test --workspace` inside the Nix sandbox.
      (src + "/packaging/linux/nfpm.yaml")
      (src + "/xtask")
    ];
  };

  # Single source of truth for the version: [workspace.package] in the
  # workspace Cargo.toml (every crate uses version.workspace = true). Read it
  # through the flake source accessor so package evaluation does not require
  # materialising the filtered build source first.
  version = (builtins.fromTOML (builtins.readFile (src + "/Cargo.toml"))).workspace.package.version;

  # GPUI discovers these graphics backends at runtime instead of linking them,
  # so normal fixup cannot infer their store paths. Add only those paths to the
  # GUI's RUNPATH; linked xkbcommon/xcb/font libraries are fixed up normally.
  runtimeLibs = lib.makeLibraryPath [
    libGL
    wayland
    vulkan-loader
  ];

  # gpui-component checkout for the GUI build script. The upstream themes live
  # at the repository root next to (not inside) the gpui-component crate, so
  # the per-crate vendor tree importCargoLock produces doesn't contain them.
  # build.rs provides OPENHID_THEMES_DIR as an explicit override — point it
  # at a separate checkout. The rev must match Cargo.lock (a mismatch fails
  # the build with a hash error, so it cannot drift silently); the hash is
  # shared with outputHashes below.
  gpuiComponentRev = "1ac69bd51f744f3664361d9abd609bfc16de489f";
  gpuiComponentHash = "sha256-up6s9rM5brgtiBQq9CSQRf8vJ4vE2ZhY4d8wRE1nfDk=";
  gpuiComponentSrc = fetchgit {
    url = "https://github.com/AprilNEA/gpui-kit";
    rev = gpuiComponentRev;
    hash = gpuiComponentHash;
  };
in
rustPlatform.buildRustPackage {
  pname = "openhid";
  inherit version;
  src = source;
  strictDeps = true;

  cargoLock = {
    # Parse dependency metadata through the flake source accessor as above;
    # only the actual build consumes the filtered source derivation.
    lockFile = src + "/Cargo.lock";
    # One hash per git repository, keyed by any crate from that repo.
    # Obtain new values from the error message of a failing build, or with
    # `nix-prefetch-git <url> --rev <rev>`.
    outputHashes = {
      "appicon-0.1.0" = "sha256-XY8NS2qrpPbUXZ3xCPGjZbbT0tSVpapbcTbgA2H5+/I=";
      "gpui-component-0.6.1" = gpuiComponentHash;
    };
  };

  env.OPENHID_THEMES_DIR = "${gpuiComponentSrc}/themes";

  nativeBuildInputs = [
    pkg-config
    patchelf
    rustPlatform.bindgenHook # `media` (a gpui dep) runs bindgen — needs libclang
  ];

  # The xtask release tests exercise version-bump checkout against throwaway
  # repositories they `git init` themselves, so the sandboxed `cargo test`
  # needs a git binary even though the build does not.
  nativeCheckInputs = [ git ];

  # Only libraries whose *-sys crates appear in Cargo.lock. TLS is rustls and
  # evdev/hidraw are opened directly. Runtime-selected graphics libraries also
  # appear here when their headers/pkg-config metadata are needed at build time.
  buildInputs = [
    fontconfig # GPUI text rendering (yeslogic-fontconfig-sys)
    freetype # font-kit (freetype-sys)
    libGL # GPUI's OpenGL fallback
    libxkbcommon # GPUI keyboard handling
    wayland # wayland-sys
    vulkan-loader # GPUI's primary Linux renderer
    libxcb # xcb / x11rb — the hook and GPUI's X11 backend
  ];

  # Select production binaries explicitly: selecting the agent package alone
  # also builds the development-only openhid-agent-mock target.
  cargoBuildFlags = [
    "--package=openhid"
    "--bin=openhid"
    "--package=openhid-agent"
    "--bin=openhid-agent"
    "--package=openhid-desktop"
    "--bin=openhid-desktop"
    "--package=openhid-overlay"
    "--bin=openhid-overlay"
  ];

  # Match Linux CI's package selection; the desktop interaction suite is
  # additionally exercised by the macOS test jobs.
  cargoTestFlags = [
    "--workspace"
    "--exclude=openhid-desktop"
  ];

  installPhase = ''
    runHook preInstall

    releaseDir=target/${stdenv.hostPlatform.rust.rustcTarget}/release
    for binary in openhid openhid-agent openhid-desktop openhid-overlay; do
      install -Dm755 "$releaseDir/$binary" "$out/bin/$binary"
    done

    install -Dm644 packaging/linux/desktop/openhid.desktop \
      "$out/share/applications/openhid.desktop"
    # Every standard indexed hicolor size: a stock `hicolor/index.theme`
    # stops at 512x512, so an icon installed only under `1024x1024/apps` is
    # invisible to launchers that resolve by theme index.
    install -Dm644 design/icon/openhid.png \
      "$out/share/icons/hicolor/1024x1024/apps/openhid.png"
    for size in 512 256 128 64 48 32 16; do
      install -Dm644 "design/icon/openhid-$size.png" \
        "$out/share/icons/hicolor/''${size}x''${size}/apps/openhid.png"
    done
    install -Dm644 packaging/linux/udev/70-openhid.rules \
      "$out/lib/udev/rules.d/70-openhid.rules"
    install -Dm644 packaging/linux/systemd/openhid-agent.service \
      "$out/share/systemd/user/openhid-agent.service"
    install -Dm644 LICENSE-APACHE "$out/share/licenses/openhid/LICENSE-APACHE"
    install -Dm644 LICENSE-MIT "$out/share/licenses/openhid/LICENSE-MIT"
    install -Dm644 NOTICE "$out/share/licenses/openhid/NOTICE"
    install -Dm644 crates/openhid-hidpp/LICENSE "$out/share/licenses/openhid/LICENSE-HIDPP"

    substituteInPlace "$out/share/systemd/user/openhid-agent.service" \
      --replace-fail \
        "ExecStart=/usr/bin/openhid-agent" \
        "ExecStart=$out/bin/openhid-agent"

    runHook postInstall
  '';

  postFixup = ''
    patchelf --add-rpath "${runtimeLibs}" "$out/bin/openhid-desktop"
  '';

  doInstallCheck = true;
  nativeInstallCheckInputs = [ versionCheckHook ];
  preInstallCheck = ''
    for binary in openhid openhid-agent openhid-desktop openhid-overlay; do
      test -x "$out/bin/$binary"
    done
    test ! -e "$out/bin/openhid-agent-mock"
    test -f "$out/lib/udev/rules.d/70-openhid.rules"
    test -f "$out/share/applications/openhid.desktop"
    for size in 1024 512 256 128 64 48 32 16; do
      test -f "$out/share/icons/hicolor/''${size}x''${size}/apps/openhid.png"
    done
    grep -Fqx \
      "ExecStart=$out/bin/openhid-agent" \
      "$out/share/systemd/user/openhid-agent.service"
  '';

  meta = {
    description = "Local-first companion for Logitech HID++ peripherals";
    homepage = "https://github.com/Ben8368/OpenHID";
    license = with lib.licenses; [
      mit
      asl20
    ];
    mainProgram = "openhid";
    # Darwin support (the .app bundle, see nixpkgs' `openhid`) could be
    # revived here later; this package is authored and tested on Linux.
    platforms = lib.platforms.linux;
  };
}
