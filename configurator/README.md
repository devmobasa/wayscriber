# Wayscriber Configurator (Iced)

Native Rust desktop UI for editing `~/.config/wayscriber/config.toml`. The application is built on [`iced`](https://github.com/iced-rs/iced) and reuses the `wayscriber::Config` types directly, so validation, defaults, and backup behavior match the CLI.

## Prerequisites

- Rust toolchain (1.80 or newer recommended).
- System dependencies required by `iced`/`wgpu` (Vulkan/Metal/DirectX drivers on the host platform).

## Run It

```bash
cd configurator
cargo run
```

The window loads the current config, lets you tweak values across the tabbed sections, and writes changes back via `Config::save_with_backup()`.

### Handy actions

- **Reload** – re-read `config.toml` from disk.
- **Defaults** – drop in the built-in defaults without saving.
- **Save** – validate inputs (including numeric ranges and color arrays) and write the TOML file. An existing file is backed up with a timestamp.
- Launch from the main overlay with the default `F11` keybinding (configurable inside the app).

## UI Coverage

- **Drawing, Arrow, Performance, UI, Board, Capture** – numeric fields with inline validation, toggles, and color editors (RGBA/RGB components).
- **Default color** – toggle between named colors and custom RGB triples.
- **Keybindings** – per-action comma-separated shortcut lists that map to `KeybindingsConfig`.
- Live dirty-state indicator plus status banner for success/error details.

## Building Releases

```bash
cargo build --release
```

Artifacts land in `target/release/`. No Node toolchain or bundler is required.
