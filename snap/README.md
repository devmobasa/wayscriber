# Snap Packaging (Experimental)

This repository includes a starter Snapcraft recipe:

- `snap/snapcraft.yaml`

## Build on Ubuntu

Install tooling:

```bash
sudo snap install snapcraft --classic
```

Build snap from repo root:

```bash
snapcraft --verbose --destructive-mode --project-dir .
```

Install local build:

```bash
sudo snap install --dangerous wayscriber_*.snap
```

Run:

```bash
wayscriber
wayscriber.wayscriber-configurator
wayscriber.wayscriber-daemon
```

Smoke check:

```bash
./snap/smoke-check.sh
```

Post-build desktop check (after `snapcraft`):

```bash
./snap/check-built-desktop.sh
# or pass an explicit build dir:
./snap/check-built-desktop.sh /path/to/build-dir
```

Notes:

- This is an experimental strict-confinement recipe.
- Daemon and global-shortcut workflows are typically less seamless in strict sandboxed packaging than native `.deb`/`.rpm`.
- If you need native-like daemon integration first, prefer `.deb`/`.rpm` packaging.
