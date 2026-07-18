# Wayscriber Codebase Overview (Except Configurator)

This document explains how the application boots, how user input travels through the system, and how the major modules fit together. Use it as a map when adding features or debugging. The configurator binary lives in `configurator/` and is intentionally excluded here.

---

## 1. Execution Flow From the Library Entry Facade

1. **Binary entry (`src/main.rs` and `src/lib.rs`)**
   - `src/main.rs` only returns `wayscriber::run_from_env()`.
   - The library facade uses the manual parser in `src/cli.rs`, prints help/version or argument diagnostics, initializes logging for runtime commands, and maps application errors to process exit codes.

2. **Mode selection (`src/app/`)**
   - `--daemon`: instantiate `daemon::Daemon` with the optional initial board mode and call `run()`.
   - `--active`: print usage/help tips, then call `backend::run_wayland`.
   - No flags: print a usage summary and exit.
   - Modes that require a compositor verify `WAYLAND_DISPLAY` before runtime startup.

3. **Canonical module graph**
   - `src/lib.rs` declares both reusable public modules and private runtime modules, so the binary does not compile a second copy of shared types or unit tests.
   - `domain`: owns stable action, tool, color, and board value identities used across higher layers.
   - `config`: loads user settings, key bindings, and drawing defaults.
   - `session`: builds configured or named session targets, validates `--session-file`, loads saved state, and records named-session catalog entries.

---

## 2. Daemon Mode Lifecycle

**Modules:** `src/daemon/` (control, core, overlay, tray, shortcuts, and setup), plus the public
backend entry in `src/backend/mod.rs`.

1. `Daemon::run` acquires the single-instance runtime lock, installs the owned Unix signal
   listener, creates its wake descriptor, and publishes pid plus instance-token readiness only
   after signal handling is active.
2. It optionally starts the status tray and portal global-shortcut listener.
3. Typed `--daemon-toggle` requests are written to the daemon command queue, wake the control loop
   through SIGUSR1, and wait for a request-specific response. A raw SIGUSR1 remains a legacy
   argument-free toggle.
4. The control loop drains published commands, starts/stops or forwards actions to the overlay
   child, and reaps child state before handling the next transition.
5. Shutdown invalidates readiness, terminates owned helpers/overlay work, and joins listener and
   tray threads.

Daemon mode therefore provides a persistent background service that reacts to user keybinds (preferably configured to run `wayscriber --daemon-toggle`, which forwards to the daemon) or to tray actions.

---

## 3. Active Mode / Wayland Backend

**Modules:**
- `src/backend/mod.rs`: exported API (`run_wayland`)
- `src/backend/wayland/backend/`: high-level bootstrap, setup, and event loop
- `src/backend/wayland/state.rs`: runtime state (surfaces, buffers, runtime handles)
- `src/backend/wayland/handlers/`: Smithay trait implementations and protocol handlers

**Flow:**
1. `backend::run_wayland` creates `WaylandBackend`.
2. `WaylandBackend::run`:
   - Connects to Wayland (`smithay-client-toolkit`).
   - Binds compositor, layer shell, SHM, outputs, seats, registry.
   - Loads configuration (color defaults, board settings, keybindings).
   - Initializes `InputState` (see section 4).
   - Creates the layer-shell overlay surface and enters the event loop.
3. Main loop responsibilities:
   - Dispatch Wayland events via smithay handlers (keyboard, pointer, seat, compositor).
   - Throttle rendering with frame callbacks / vsync support.
   - Communicate with `capture::CaptureManager` for screenshot actions.
   - Exit when `InputState.should_exit` is set (Escape, tray close, etc.).

`WaylandState` centralizes everything the handlers need: current buffers, Cairo context, mouse positions, capture state, and tokio handle for async work.

---

## 4. Input Handling & Drawing State

**Modules:** `src/input/`, `src/input/state/{core,actions,mouse,interaction}/`, `src/input/state/render.rs`, `src/draw/`, `src/ui.rs`, and `src/ui/`

1. **Keyboard events (`handlers/keyboard.rs`)**
   - Translate Wayland keysyms to internal `Key`.
   - Call `InputState::on_key_press` / `on_key_release`.
   - Key presses can enqueue backend output work; the event loop drains `InputState::take_pending_backend_action`.

2. **Mouse events (`handlers/pointer.rs`)**
   - Update `current_mouse_x/y`.
   - Call `InputState::on_mouse_press`, `on_mouse_motion`, `on_mouse_release`.
   - Adjust pen thickness or font size via scroll wheel + modifiers.

3. **`InputState` responsibilities**
   - Holds `input::BoardManager`, whose ordered `BoardState` entries each own `draw::BoardPages`,
     plus current colors, tool settings, fonts, modifiers, and `DrawingState`.
   - `state/actions/` maps keybindings to `Action` values and routes color, board/page, capture,
     history, selection, tool, and UI behavior.
   - `state/mouse/` and `state/interaction/` convert pointer gestures into drawing/state changes.
   - `render.rs` exposes provisional shape previews for live feedback.

4. **Rendering to the overlay**
   - `WaylandState::render` uses Cairo + SHM buffers.
   - Draw order: board background → finalized shapes → provisional shape → text cursor preview → status bar (if enabled) → help overlay (if toggled).
   - `ui` module encapsulates status/help overlays, while `draw` handles actual vector geometry routines.

The result is a predictable pipeline: Wayland → handlers → `InputState` →
`BoardManager`/active `BoardPages`/`DrawingState` → `WaylandState::render`.

---

## 5. Capture Pipeline

**New structure (all under `src/capture/`):**

| File/Folder | Purpose |
|-------------|---------|
| `mod.rs` | Public exports and shared submodules. |
| `manager.rs` | `CaptureManager` – unique owner of capacity-one request/completion channels, checked request IDs, status, and its Tokio worker task. |
| `dependencies.rs` | Trait definitions (`CaptureSource`, `CaptureFileSaver`, `CaptureClipboard`) and default implementations. |
| `pipeline.rs` | `perform_capture`, `deliver_image`, `deliver_document`, and capture/delivery request definitions. |
| `sources/` | Strategies for acquiring image bytes: Hyprland fast-path (`hyprland.rs`), portal fallback (`portal.rs`), and URI reader/cleanup (`reader.rs`). |
| `clipboard.rs`, `file.rs`, `portal.rs` | Support code reused by the pipeline. |
| `tests/` | Unit tests and fixtures for the manager, sources, and pipeline. |

**Runtime flow:**
1. `InputState::handle_action` sets `pending_backend_action` for screenshot capture and canvas export actions.
2. The Wayland event loop centrally drains the pending backend action, so keybindings, command-palette Return, and command-palette mouse clicks share the same dispatch path.
3. Screenshot actions call `WaylandState::handle_capture_action`; explicit canvas PNG export actions call `WaylandState::handle_canvas_export_action`; board PDF actions call `WaylandState::handle_board_pdf_export_action`.
4. `WaylandState::handle_capture_action` builds a `CaptureRequest` (type + destination + save config), hides the overlay, and queues the request until the suppression frame is confirmed; it then calls `CaptureManager::request_capture`.
5. Canvas export snapshots persisted board content in the current panned viewport, renders PNG bytes, and calls `CaptureManager::request_image_delivery`.
6. Board PDF export snapshots active-board or all-board pages with per-page layout metadata, renders PDF bytes, and calls `CaptureManager::request_document_delivery`.
7. A mutable `CaptureManager` submission returns a checked `CaptureRequestId`. `CaptureState` records that ID and remains the sole event-side completion owner until the matching terminal result is consumed.
8. `CaptureManager`’s owned Tokio task receives the request, updates status, and calls `perform_capture`, `deliver_image`, or `deliver_document`.
9. `perform_capture`:
   - Calls the configured `CaptureSource` (default: `sources::capture_image` with Hyprland→portal fallback).
   - Optionally saves via `CaptureFileSaver`.
   - Optionally copies to clipboard via `CaptureClipboard`.
   - Returns `CaptureResult` used for desktop notifications.
10. The worker publishes one identified terminal result before waking the shared Wayland runtime descriptor. `WaylandState` non-blockingly polls `CaptureManager`, accepts only the recorded ID, restores the overlay, and emits notifications. Worker loss and identity mismatches are terminal and are reported once.

`CaptureManager` is intentionally not cloneable: one owner controls submission and completion
consumption. Both transports have capacity one, so queued, running, and completed-but-unread work
all remain single-flight and overlapping submissions return `CaptureSubmitError::Busy` with the
active ID. Non-Wayland callers can construct a manager without a wake callback and poll it directly.

Notifications are sent via `notification::send_notification_async`, keeping all UI feedback on the event loop thread.

---

## 6. Toolbar Frontends

- `src/ui/toolbar/model/top_spec.rs` owns the renderer-neutral top-toolbar contract: stable
  control IDs, ordered strip/divider/chrome/overflow nodes, events, active/enabled state, labels,
  tooltips, shortcut badges, and semantic icons. It consumes the shared width-degradation result
  but contains no geometry or toolkit types.
- `src/backend/wayland/toolbar/view/top/` exhaustively adapts that contract to the built-in
  `WidgetTree`, which remains the sole owner of Cairo geometry, hit testing, popover placement,
  and surface input regions.
- `src/toolbar_gtk/view/top_bar/` exhaustively adapts the same contract to GTK widgets while
  retaining GTK sizing, CSS, updater closures, drag gestures, and popover lifecycle.
- Shape-picker compound rows and all side-palette layout remain frontend-specific. Their existing
  tool/section ordering still comes from the shared toolbar model.

---

## 7. Domain Values and Dependency Direction

- **`src/domain/`** is the canonical owner of dependency-light action, tool, color, and board
  value identities. Production code there depends only on the standard library, serde, and the
  optional schema derive; runtime policy and mutable state stay in higher layers.
- Existing paths such as `config::Action`, `input::Tool`, `input::BoardBackground`, and
  `draw::Color` are compatibility re-exports of the same domain types. They preserve public Rust
  API and serialized config/session formats while callers migrate incrementally.
- `src/config/` retains config representation, keybinding syntax/defaults, validation, and action
  metadata. `src/input/` retains tool catalogs/behavior and board state. `src/draw/` retains shapes,
  history, and rendering.
- New dependency-light identities belong in `domain`; I/O, toolkit types, rendering behavior,
  state machines, and config-specific metadata do not.

---

## 8. Configuration

- **`src/config/`** handles loading `config.toml`, validating fields, and building the keybinding map.
- **`ConfigDocument`** is the configurator-facing edit owner. It keeps validated `Config`, the
  lossless TOML source, unknown-path diagnostics, source path, and exact source revision behind one
  interface. Guarded saves merge known fields while retaining comments and unsupported settings,
  then reuse the normal backup and durable atomic-write policy. Its editor load path can expose a
  backup-protected defaults-based repair document for readable but invalid config, while true I/O
  failures leave the configurator's last good document untouched. Runtime callers can continue
  using the typed `Config::load()` and `Config::save*()` interfaces.
- The Performance section is the first bounded scalar-metadata slice: core config owns its field
  IDs, paths, labels, help/search terms, and numeric constraints while the configurator keeps typed
  draft fields and messages.

---

## 9. Session Persistence and Named Session Manager

**Modules:**
- `src/session/`: target options, primary-file validation, snapshot load/save, sidecars, clear/recovery markers, saved tool-state reset, locks, catalog metadata, and inactive file operations.
- `src/backend/wayland/session/`: runtime Open, Save As, Clear, and saved tool-state reset transactions for the active overlay.
- `src/backend/wayland/state/toolbar/events/session.rs`: overlay Session panel routing for Open, Save As, Info, Clear, recent sessions, and configurator launch.
- `src/daemon/`: accepts daemon-toggle requests that carry an optional named session target.

**Flow:**
1. CLI `--session-file` creates a named target instead of using configured storage. Named targets force persistence for that run, reject `--no-resume-session`, require an existing parent directory for foreground/open flows, and reject directories, symlinks, and special files.
2. Backend startup builds `SessionOptions` from config plus any named target, then session loading restores boards/history/tool state before rendering begins.
3. Runtime Open first saves dirty current data when needed, loads the candidate named session without mutating it, replaces board state only after a valid load, and records the open in the named-session catalog.
4. Runtime Save As validates the target, prompts before replacing existing artifacts, writes the snapshot, switches the active target, and records the save in the catalog.
5. Runtime Clear writes a durable empty-session boundary so older backup or recovery artifacts do not restore stale drawings.
6. Runtime saved tool-state reset clears the persisted tool layer for the active session and applies config-derived tool defaults in memory so autosave does not restore stale values.
7. Offline CLI maintenance can inspect sessions, clear all saved data, or clear only persisted tool state so config defaults seed the next startup without deleting boards.
8. The configurator reads the same catalog for inactive-session management: rename/reveal/forget metadata, duplicate primary files, move non-lock sidecars, clear saved tool state, and clear saved data when daemon/overlay locks are absent.

---

## 10. Utility Modules

- **`src/draw/`**: Shape/frame definitions, page storage, undo/history, fonts, and Cairo/Pango
  rendering helpers. Board ordering and active-page ownership remain in `input::BoardManager`.
- **`src/ui.rs` and `src/ui/`**: Compose status, help, toolbar models, pickers, panels, and other
  overlay UI using Cairo-facing render helpers.
- **`src/notification.rs`**: Tiny helper to send desktop notifications asynchronously (used after captures).
- **`src/util/`**: Shared arrow, color, geometry, and text helpers.
- **`tests/`**: Integration tests (CLI smoke tests, rendering sanity checks) live outside `src/`.

---

## 11. Directory Map (excluding configurator)

| Path | Role |
|------|------|
| `src/main.rs` | Thin binary wrapper around the library entry facade. |
| `src/lib.rs` | Canonical module graph, CLI/error entry facade, and reusable public exports. |
| `src/domain/` | Stable action, tool, color, and board values with no upward runtime dependencies. |
| `src/daemon/` | Background daemon control queue, lifecycle, overlay child, shortcuts, and tray. |
| `src/backend/` | Wayland backend implementation split into bootstrap (`mod.rs`), runtime (`state.rs`), and input/render handlers. |
| `src/input/` | Event/state machine, tools, board/page ownership, selection, and action routing. |
| `src/draw/` | Vector drawing primitives, frames/pages, history, fonts, and rendering helpers. |
| `src/ui.rs` | Status/help overlays. |
| `src/capture/` | Screenshot pipeline (manager, dependencies, sources, clipboard/file helpers). |
| `src/config/` | Config parsing, defaults, keybinding map. |
| `src/session/` | Configured and named session persistence, snapshots, sidecars, locks, and catalog metadata. |
| `src/notification.rs` | Desktop notifications for capture results. |
| `src/util/` | Shared math, color, arrow, and text utilities. |
| `tests/` | CLI + rendering integration tests. |

---

## 12. Putting It Together

1. **Launch** via CLI → choose daemon vs active.
2. **Daemon** provides lifecycle management, tray integration, and toggles the backend on demand.
3. **Backend** sets up Wayland surfaces and loops, forwarding input to `InputState`.
4. **InputState + draw/ui** update the overlay contents and request renders.
5. **Capture** subsystem handles screenshot actions asynchronously and notifies the user.
6. **Session** loads and saves configured or named session state, including runtime Open/Save As/Clear transactions.
7. **Config** module ensures user preferences are honored everywhere.

Use this document to trace any feature: locate the entry point (CLI, tray, keybinding), follow it through the backend/input/capture stacks, and consult the relevant modules listed above for details.
