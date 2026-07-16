# Wayscriber Configurator (Iced)

Native Rust desktop UI for editing `~/.config/wayscriber/config.toml`. The application is built on [`iced`](https://github.com/iced-rs/iced) and reuses the `wayscriber::Config` types directly, so validation, defaults, and backup behavior match the CLI. It also retains the original TOML document so comments, ordering, and settings unknown to this build survive a save.

## Prerequisites

- Rust toolchain 1.95 or newer.
- System dependencies required by `iced`'s Wayland `tiny-skia` renderer.

## Run It

```bash
cd configurator
cargo run
```

The configurator uses Iced's software `tiny-skia` renderer on Wayland. It does
not compile the GPU renderer or portal D-Bus implementation into the
configurator binary.

The window loads the current config, lets you tweak values across the tabbed sections, and writes changes back through the guarded `ConfigDocument` save interface.

### Handy actions

- **Reload** – re-read `config.toml` from disk and refresh the guarded source revision. A transient load error leaves the last good document and current draft in place.
- **Defaults** – drop in the built-in defaults without saving.
- **Save** – validate inputs (including numeric ranges and color arrays), merge known changes into the source TOML, and write it atomically. An existing file is backed up with a timestamp. Save is refused if the file was created, deleted, retargeted through a symlink, or changed byte-for-byte after loading; reload before retrying. If a readable file cannot be parsed, the configurator offers a warning-marked defaults-based repair draft and backs up the unreadable source before saving it. Unknown settings are retained only when the TOML structure is parseable and safely separable; malformed content remains in the backup.
- **Search** – filter tabs, sections, saved sessions, boards, render profiles, presets, and keybindings as you type. Press `Ctrl+F` to focus search and `Escape` to clear it.
- Launch from the main overlay with the default `F11` keybinding (configurable inside the app).

## UI Coverage

- **Drawing, Arrow, Performance, UI, Board, Capture** – numeric fields with inline validation, toggles, and color editors (RGBA/RGB components).
- **Default color** – toggle between named colors and custom RGB triples.
- **Keybindings** – per-action comma-separated shortcut lists that map to `KeybindingsConfig`.
- **Session** – persistence settings plus named-session catalog management. Rename display labels, reveal files, and forget metadata without touching files. Clear Tool State preserves boards/history while removing persisted tool defaults. Duplicate, Move, Clear Tool State, and Clear are disabled while an overlay, manually started daemon, or background service is active.
- Live dirty-state indicator plus status banner for success/error details.
- Non-fatal warnings list unrecognized config paths. Those values are preserved for forward compatibility instead of being deleted.

## Building Releases

```bash
cargo build --release
```

Artifacts land in `target/release/`. No Node toolchain or bundler is required.
