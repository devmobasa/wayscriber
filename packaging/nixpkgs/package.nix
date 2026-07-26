{
  lib,
  rustPlatform,
  fetchFromGitHub,
  nix-update-script,
  pkg-config,
  runtimeShell,
  wrapGAppsHook4,
  cairo,
  grim,
  gtk4,
  gtk4-layer-shell,
  libxkbcommon,
  pango,
  slurp,
  wayland,
  wl-clipboard,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "wayscriber";
  version = "0.9.22";

  src = fetchFromGitHub {
    owner = "devmobasa";
    repo = "wayscriber";
    tag = "v${finalAttrs.version}";
    # Refresh with `nix-update wayscriber` inside a nixpkgs checkout before
    # submitting; see ./README.md.
    hash = lib.fakeHash;
  };

  nativeBuildInputs = [
    pkg-config
    wrapGAppsHook4
  ];

  # Keep in sync with the default feature set in Cargo.toml; the GTK inputs are
  # required by the `toolbar-gtk` default feature.
  # Checked by tools/check-nixpkgs-recipe.py.
  buildInputs = [
    cairo
    gtk4
    gtk4-layer-shell
    libxkbcommon
    pango
    wayland
  ];

  cargoHash = lib.fakeHash;

  postInstall = ''
    install -Dm644 packaging/wayscriber.desktop \
      $out/share/applications/wayscriber.desktop
    install -Dm644 packaging/wayscriber.service \
      $out/lib/systemd/user/wayscriber.service
    substituteInPlace $out/lib/systemd/user/wayscriber.service \
      --replace-fail "/bin/sh" "${runtimeShell}" \
      --replace-fail "/usr/bin/wayscriber" "$out/bin/wayscriber" \
      --replace-fail "/usr/local/bin:/usr/bin:/bin" \
        "${lib.makeBinPath [ grim slurp wl-clipboard ]}:/run/current-system/sw/bin:/etc/profiles/per-user/%u/bin:%h/.nix-profile/bin"

    for size in 16 19 22 24 38 64 128; do
      for category in apps status; do
        install -Dm644 packaging/icons/wayscriber-$size.png \
          $out/share/icons/hicolor/''${size}x''${size}/$category/wayscriber.png
      done
    done

    install -Dm644 packaging/icons/wayscriber.svg \
      $out/share/icons/hicolor/scalable/apps/wayscriber.svg
    install -Dm644 packaging/icons/wayscriber-symbolic.svg \
      $out/share/icons/hicolor/symbolic/apps/wayscriber-symbolic.svg

    install -Dm644 config.example.toml \
      $out/share/doc/wayscriber/config.example.toml
  '';

  passthru.updateScript = nix-update-script { };

  meta = {
    description = "ZoomIt-like screen annotation tool for Wayland compositors, written in Rust";
    homepage = "https://wayscriber.com";
    changelog = "https://github.com/devmobasa/wayscriber/releases/tag/v${finalAttrs.version}";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [
      leiserfg
    ];
    mainProgram = "wayscriber";
    platforms = lib.platforms.linux;
  };
})
