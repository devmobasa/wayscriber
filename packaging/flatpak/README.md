# Flatpak Packaging (Experimental)

This repository includes a starter Flatpak manifest for local packaging:

- `packaging/flatpak/com.wayscriber.Wayscriber.yaml`

## Build on Ubuntu

Install Flatpak tooling:

```bash
sudo apt update
sudo apt install -y flatpak flatpak-builder
```

Install required runtime/SDK:

```bash
flatpak install -y flathub org.freedesktop.Platform//24.08
flatpak install -y flathub org.freedesktop.Sdk//24.08
flatpak install -y flathub org.freedesktop.Sdk.Extension.rust-stable//24.08
```

Build and install locally:

```bash
flatpak-builder --user --install --force-clean build-flatpak packaging/flatpak/com.wayscriber.Wayscriber.yaml
```

Run:

```bash
flatpak run com.wayscriber.Wayscriber --active
flatpak run --command=wayscriber-configurator com.wayscriber.Wayscriber
```

Optional daemon launch inside Flatpak:

```bash
flatpak run --command=wayscriber com.wayscriber.Wayscriber --daemon --no-tray
```

Notes:

- This is an experimental manifest focused on local builds.
- Strict portal/sandbox behavior may differ from native `.deb`/`.rpm`.
- Daemon + global shortcut workflows are usually less seamless in sandboxed packaging than native packages.
