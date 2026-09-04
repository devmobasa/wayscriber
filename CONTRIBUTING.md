# Contributing to wayscriber

Thanks for your interest in improving wayscriber. This guide covers the supported workspace,
architecture boundaries, and validation workflow without trying to inventory every source file.

## Contribution principles

Wayscriber is shared as a gift exchange, not a contract. Requests are welcome, but there is no
guaranteed timeline or support obligation. See the
[gift-exchange principles](https://wayscriber.com/docs/ethos/gift-exchange.html) for context.

Changes should preserve foreground/fullscreen safety, serialized config and session compatibility,
and the feature-gate behavior described below. Prefer focused changes with tests at the owning
module boundary.

## Workspace and toolchain

The repository is one Cargo workspace using Rust 1.98.1 and edition 2024.
`rust-toolchain.toml` pins the development toolchain, including Clippy and rustfmt; CI uses
the same version. The Nix flake selects that version through the `rust-overlay` input pinned
in `flake.lock`, using the root package's `rust-version` for both crates and the development
shell. When raising the version, update both manifests, `rust-toolchain.toml`, and CI pins;
run `nix flake update rust-overlay` if the locked overlay does not yet provide it.

- the root `wayscriber` package owns the overlay, daemon, CLI, shared domain/config/session code,
  rendering, and integration tests;
- `wayscriber-configurator` is the GTK4/libadwaita (Relm4) GUI package under `configurator/`;
- root `Cargo.lock` is the single lockfile for both packages.

The workspace default member is only the root package. Use `-p wayscriber-configurator` or
`--workspace` when configurator coverage is required. A command such as
`cargo build --manifest-path configurator/Cargo.toml` still belongs to the root workspace and uses
the root lockfile.

The main package's `build.rs` only exposes an optional release-version override, embeds the current
Git hash, and tells Cargo which Git metadata should trigger a rebuild. Cargo features use standard
`cfg(feature = "...")` conditions directly.

## Development

Build both packages without launching a window:

```bash
cargo build --workspace
```

The overlay and configurator are foreground applications. Only launch them when it is safe for
them to take focus:

```bash
cargo run -- --active
cargo run -p wayscriber-configurator
```

The CLI parser is implemented manually in `src/cli.rs`; keep parsing, help/version output, and CLI
integration tests aligned when flags change.

## Architecture ownership

Use [the codebase overview](docs/codebase-overview.md) for the main runtime flow and
[the configurator README](configurator/README.md) for the editor. The stable ownership map is:

| Area | Owner |
|---|---|
| Entry and CLI | `src/main.rs`, `src/lib.rs`, `src/app/`, and `src/cli.rs` |
| Stable values | `src/domain/` for dependency-light action, tool, color, and board identities |
| Config | `src/config/`, `config.example.toml`, [docs/CONFIG.md](docs/CONFIG.md), and configurator mappings |
| Input and boards | `src/input/`; `BoardManager` owns ordered `BoardState` values and their pages |
| Drawing and rendering | `src/draw/` for frames, shapes, history, page storage, and Cairo/Pango helpers |
| Wayland runtime | `src/backend/wayland/` for protocol setup, handlers, event-loop phases, state, and rendering |
| Daemon | `src/daemon/` for control requests, service lifecycle, overlay children, and tray integration |
| Sessions | `src/session/` plus Wayland session transactions for snapshots, storage, locks, and recovery |
| Capture/export | `src/capture/` and `src/canvas_export/` |
| UI and toolbars | `src/ui/`, `src/ui/toolbar/`, `src/backend/wayland/toolbar/`, and `src/toolbar_gtk/` |
| Configurator | `configurator/src/app/` for state/update/side effects and `configurator/src/models/` for typed drafts |
| Packaging and automation | `packaging/`, `tools/`, and `.github/workflows/` |

Scoped `AGENTS.md` files record invariants and coupled-change reminders near these owners. They are
more durable than a central list of every module and should be updated when a boundary changes.

## Common coupled changes

- Config fields often require core types/defaults/validation, `config.example.toml`,
  `docs/CONFIG.md`, configurator draft/view/search behavior, schema behavior, and tests.
- Actions, tools, and keybindings often require action metadata, default maps, input routing, help
  and command UI, both toolbar frontends, configurator labels/search, docs, and tests.
- Board/page changes can affect session snapshots, board picker and toolbar UI, persistence config,
  export behavior, and identity/ordering tests.
- Daemon/service/shortcut changes can affect `src/daemon/`, shared service/path helpers,
  configurator daemon setup, packaging units, and setup docs.
- Serialized config or session changes must preserve compatibility or provide an explicit migration
  with regression fixtures.

## Feature gates

Preserve the intent of the root package features:

- `tablet-input` — tablet/stylus protocol support;
- `dbus` — shared D-Bus support;
- `portal` — portal capture/runtime support, layered on `dbus`;
- `tray` — status tray support, layered on `dbus`;
- `toolbar-gtk` — GTK layer-shell toolbar frontend;
- `config-schema` — schema export support.

The configurator has its own `tablet-input` feature forwarding to the root package. Full CI covers
all features and no default features across the workspace.

## Tests and linting

Run focused checks while iterating, for example:

```bash
cargo test config::
cargo test session::
cargo test -p wayscriber-configurator
```

Before submitting a broad or cross-package change, run the local CI entry point:

```bash
./tools/lint-and-test.sh
```

It checks release/package metadata, package layout, Rust source coverage, formatting, strict
all-feature Clippy, all-feature tests, and no-default-feature tests. The source-coverage gate uses
current rustc dep-info and rejects tracked or unignored `.rs` files that are outside the supported
Cargo target/feature matrix.

For offline work, prefetch dependencies first:

```bash
./tools/fetch-all-deps.sh
```

### Freeze, zoom, and capture manual matrix

Geometry unit tests cover crop arithmetic and stale-output rejection, but they do not prove a
compositor's protocol behavior. For capture-related changes, exercise every unchecked cell below
on real hardware; this checklist is manual evidence, not CI coverage.

| Compositor | 1x | Fractional scale | Mixed DPI |
|---|---|---|---|
| Hyprland | [ ] | [ ] | [ ] |
| Plasma/KWin | [ ] | [ ] | [ ] |
| Niri | [ ] | [ ] | [ ] |
| Sway | [ ] | [ ] | [ ] |
| GNOME | [ ] | [ ] | [ ] |

For each cell, record the selected capture backend from the logs. Verify that Freeze, Zoom,
and a PDF desktop backdrop all use the active output's exact pixels. User full-screen
screenshots remain whole-desktop captures. Repeat while switching outputs and, where supported,
changing scale or transform during capture: the result must either stay exact or fail visibly,
never shift, stretch, or reuse another output's image.

For changes to the native region picker, use the same compositor/scale matrix and record the
following permission-gated evidence (these checks open or foreground the overlay):

- `Ctrl+Shift+C` delivers the selected frozen pixels; the saved PNG dimensions match the picker
  readout, including while zoomed or panned.
- With `[capture].include_drawings = true`, committed annotations are already visible over the
  frozen desktop while selecting and match the delivered PNG; transient strokes, handles, and UI
  stay hidden. In interactive Review, toggling drawings off/on updates that preview immediately.
- Cancelling while pending, armed, or selecting leaves no picker-owned freeze behind, and the next
  capture action works.
- Output, scale, transform, zoom-level, and pan changes cancel with one visible message.
- `picker = "slurp"` restores the external selector. Native acquisition failure hands off to it,
  while a dismissed portal request does not.
- Pointer, touch, and stylus drags all select when the hardware is available; losing the owning
  device during a drag rearms the picker.
- Holding Shift before the first press produces a square crop, and the crosshair remains on the
  physical pointer while the squared frame follows the constrained endpoint.
- `Ctrl+A` during a pointer, touch, or stylus drag captures the whole output and consumes the
  outstanding release without starting a canvas gesture.
- Deactivating or aborting zoom while the picker waits cancels quietly; a reported zoom failure
  produces exactly one message.
- With `picker = "slurp"` and `format = "jpg"`, the legacy path remains JPEG; native selection
  remains PNG-only.
- On Hyprland and Sway, `Space` enters window mode only after the provider is ready. Verify pointer,
  touch, and stylus selection, plus `Super+Arrow` navigation and `Enter`, on overlapping tiled and
  floating windows. The highlighted readout and delivered PNG must have identical pixel bounds.
  Also switch workspaces during picker startup: candidates intentionally follow the workspace
  visible at provider-query time, while output/layout mismatches still disable window mode.
- On Plasma/KWin, Niri, and GNOME, the window-mode legend and `Space` control stay absent while area
  selection continues to work. Provider failure or an empty workspace must also fall back quietly.

At 1080p@1, 4K@2, and a fractional-scale 4K setup, capture the debug-level `Region picker frame`
timings during pointer motion. Also record the process peak-RSS delta while encoding a full-output
`Ctrl+A` crop. Keep these measurements with the change's manual test evidence; they are intentionally
not collected by CI.

These captures now fail when the active output does not advertise a current `wl_output` mode.
Plugging or unplugging any monitor also cancels in-flight Freeze and Zoom captures on unrelated
outputs, because output count is part of the layout identity used to reject stale frames.

`./tools/code-health-report.sh` reports navigational maintainability metrics. Its CI artifact is
observational, not a global file/function-size gate; use the report to find code worth understanding,
not as a reason for mechanical splitting.

## Documentation and release metadata

- User-visible config behavior belongs in `docs/CONFIG.md` and `config.example.toml`.
- Installation, service, and shortcut behavior belongs in `docs/SETUP.md` and packaging docs.
- Main-crate architecture belongs in `docs/codebase-overview.md`.
- Drafts under `docs/temp/` are planning material unless explicitly promoted.
- Version changes must go through `tools/bump-version.sh`; keep both package manifests, root
  `Cargo.lock`, packaging metadata, and tag/release policy aligned.
- Close a user-visible "I don't have that setting / this build" report only after the change is in
  a tagged GitHub release. `main` is not what `arch-install.sh`, AUR `wayscriber-bin`, or other
  packaged installs ship. `--version` reports the crate version, not the git hash, so bump with
  `tools/bump-version.sh` in the same change as a user-visible overlay, settings, or config toggle
  (or immediately before tagging that release). Otherwise two binaries can print the same
  `wayscriber 0.9.x` and look identical.

See [tools/README.md](tools/README.md) for build, install, packaging, version, and release helpers.
