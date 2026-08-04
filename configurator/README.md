# Wayscriber Configurator (GTK4)

Native Rust desktop UI for editing `~/.config/wayscriber/config.toml`. The application is built on GTK4 and libadwaita through [Relm4](https://relm4.org) and reuses the `wayscriber::Config` types directly, so validation and defaults match the CLI. It also retains the original TOML document so comments, ordering, and settings unknown to this build survive a save.

`config.toml` changes only through an explicit user edit action, never automatically. This program writes it when you press **Save**; the overlay writes it from three narrow editors — shortcut editing, preset slots, and the quick-color palette — each of which rewrites only its own key and backs the file up first. Nothing else in Wayscriber — daemon, tray, startup, shutdown, validation — ever changes it, so an incidental preference toggle applies to that run and sends you here for a durable change.

## Prerequisites

- Rust toolchain 1.95 or newer.
- System development packages for GTK 4 and libadwaita.

## Run It

```bash
cd configurator
cargo run
```

The configurator renders through GTK 4 and libadwaita. It does not compile the
overlay's GPU renderer or the portal D-Bus implementation into the configurator
binary.

The window loads the current config, lets you tweak values across the tabbed sections, and writes
changes back through the guarded `ConfigDocument` save interface when you press Save. Loading,
reloading, dismissing a migration offer, and closing without saving leave the file untouched.

### Opening a specific screen

`--open <DESTINATION>` selects the screen to show once the configuration finishes loading. The
overlay and the tray use it to send you to the setting behind the control you just used.

```bash
wayscriber-configurator --open keybindings/tools
wayscriber-configurator --open 'drawing?search=Quick Colors'
```

Destinations are `ui/toolbar`, `ui/toolbar-visibility`, `ui/status-bar`, `ui/click-highlight`,
`ui/input-hud`, `drawing`, `presets`, `boards`, `history`, `session`, `keybindings`, and
`keybindings/<section>` for `general`, `drawing`, `tools`, `selection`, `history`, `boards`,
`ui-modes`, `capture-view`, and `presets`. Append `?search=<TERM>` to any of them to open with the
search box filled. `--help` prints the same list. An unknown destination falls back to the normal
initial screen and says so in the status banner.

Toolbar pins, placement offsets, the top strip's display form, item visibility/order, pane state,
and board pin controls are labeled as configured defaults because the running overlay can store
later customizations in the separate generated
`$XDG_DATA_HOME/wayscriber/runtime-ui.toml` file. The configurator edits only `config.toml`; it does
not overwrite or reset runtime preferences. Use the overlay Settings panel to inspect or reset that
state.

Config, daemon-setup, and saved-session filesystem/process operations run through a bounded Tokio
blocking-job adapter. Two jobs may run concurrently; existing request ordering and busy-state gates
still serialize user mutations. Once started, a durable blocking operation is allowed to finish even
if its UI task is no longer observed.

### Handy actions

- **Reload** – re-read `config.toml` from disk and refresh the guarded source revision. A transient load error leaves the last good document and current draft in place.
- **Configuration update available** – shown when the file's `config_revision` predates this build's keybinding defaults. The banner lists every proposed shortcut change as before → after; **Apply Update** edits the draft only, and **Dismiss** hides the offer for this run. Nothing reaches disk until you Save, and saving an unrelated setting without applying leaves both the old bindings and the old revision on disk.
- **Defaults** – drop in the built-in defaults without saving. Pressing it asks first: **Confirm Defaults** replaces the draft and **Cancel** withdraws the question, and editing anything withdraws it too. Pressing **Defaults** again changes nothing.
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
