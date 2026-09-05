# Wayscriber Configurator (GTK4)

The configurator is a native Rust desktop UI for editing `~/.config/wayscriber/config.toml`.
It uses GTK4, libadwaita, and [Relm4](https://relm4.org).
It shares the `wayscriber::Config` types with the CLI, so validation and defaults match.
It preserves TOML comments, ordering, and settings unknown to this build when it saves.

`config.toml` changes only when you explicitly edit it. The configurator writes the file when you press **Save**.
The overlay can also save shortcut edits, preset slots, and quick colors.
Each overlay editor changes only its own key and backs up the file first.
The daemon, tray, startup, shutdown, and validation do not change the file.
Other preference changes apply to the current run. Use the configurator to change their defaults.

This file covers building and running the configurator from source. For screenshots, a demo video, and the user-facing walkthrough, see [Configurator (GUI)](../README.md#configurator-gui) and https://wayscriber.com/docs/configuration/configurator.html

## Prerequisites

- Rust toolchain 1.98.1 or newer (`../rust-toolchain.toml` pins development builds).
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
`ui/input-hud`, `ui/help-overlay`, `ui/presenter-mode`, `drawing`, `presets`, `boards`, `history`,
`session`, `capture`, `performance`, `daemon`, `arrow`, `render-profiles`, `keybindings`,
and `keybindings/<section>` for `general`, `drawing`, `tools`, `selection`, `history`, `boards`,
`ui-modes`, `capture-view`, and `presets`. Builds with tablet input also accept `tablet`.
Append `?search=<TERM>` to any of them to open with the search box filled. `--help` prints the
same list. An unknown destination falls back to the normal initial screen and says so in the
status banner.

Toolbar pin, top placement offsets, the top strip's display form, item visibility/order, and board
pin controls are labeled as configured defaults because the running overlay can store later
customizations in the separate generated `$XDG_DATA_HOME/wayscriber/runtime-ui.toml` file. The
configurator edits only `config.toml`; it does not overwrite or reset runtime preferences. Use the
overlay Settings popover to inspect or reset that state.

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
- **Keybindings** – a bulk shortcut manager over the same per-action chips, recorder, and conflict flow. Filter by All / Changed / Conflicts / Unbound / Device / Sequences, sort by category, name, or changed status, and reset visible or all keybindings with confirmation (draft-only until Save). Review Conflicts walks each collision without picking a winner. `--open keybindings/<section>?search=...` still opens that category and now selects the matching action. Press-to-bind recording covers keys, auxiliary mouse buttons, and stylus barrel buttons, plus **Record Sequence** for two- or three-chord keyboard sequences (`Ctrl+K then Ctrl+C`). Per-row reset and a raw comma-separated text editor remain available (`F5, Ctrl+K > Ctrl+C`). Super/Meta chords record when the desktop delivers them. Legacy `[tablet.stylus_button]` assignments can be moved into the keybinding list with an explicit confirmation. Source badges mark Default, Authored, Legacy Tablet, and Unavailable shortcuts.
- **Session** – persistence settings plus named-session catalog management. Rename display labels, reveal files, and forget metadata without touching files. Clear Tool State preserves boards/history while removing persisted tool defaults. Duplicate, Move, Clear Tool State, and Clear are disabled while an overlay, manually started daemon, or background service is active.
- Live dirty-state indicator plus status banner for success/error details.
- Non-fatal warnings list unrecognized config paths. Those values are preserved for forward compatibility instead of being deleted.

## Building Releases

```bash
cargo build --release
```

Artifacts land in `target/release/`. No Node toolchain or bundler is required.

## Workflow ownership

Each workflow module owns a related set of operations:

- `app/document_workflow.rs` prevents loads and saves from running at the same time. It passes the loaded document to the save operation.
- `app/migration_workflow.rs` tracks update offers and dismissals for each document destination.
- `app/shortcut_workflow.rs` keeps shortcut recording, text editing, and conflict resolution separate. Only one can be active at a time.
- `app/daemon_workflow.rs` manages background setup actions, status request identities, and typed feedback.

App update handlers coordinate draft changes and UI effects.

Saves use `Config::validate_for_save` from the core crate.
It compares persisted typed values to detect changes outside keybindings and rejects those changes.
It also returns keybinding validation reports for user feedback.
Save decisions do not depend on diagnostic text.
