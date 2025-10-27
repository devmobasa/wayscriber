# Wayscriber Codebase Overview (Except Configurator)

This document explains how the application boots, how user input travels through the system, and how the major modules fit together. Use it as a map when adding features or debugging. The configurator binary lives in `configurator/` and is intentionally excluded here.

---

## 1. Execution Flow From `main.rs`

1. **CLI parsing (`src/main.rs`)**
   - Uses `clap` to parse `--daemon`, `--active`, `--mode`, and migration flags.
   - Immediately handles `--migrate-config` via `config::migrate_config` and exits.
   - Verifies `WAYLAND_DISPLAY` when a Wayland session is required.

2. **Mode selection**
   - `--daemon`: instantiate `daemon::Daemon` with the optional initial board mode and call `run()`.
   - `--active`: print usage/help tips, then call `backend::run_wayland`.
   - No flags: print a usage summary and exit.

3. **Shared subsystems automatically pulled in**
   - `config`: loads user settings, key bindings, and drawing defaults.
   - `legacy`: prints notices for old binary names and handles hyprmarker compatibility.

---

## 2. Daemon Mode Lifecycle

**Files:** `src/daemon.rs`, `src/backend/mod.rs`, `src/backend/wayland/*`

1. `Daemon::run` starts signal handlers (SIGUSR1 toggles overlay, SIGTERM/SIGINT exit).
2. Spawns a status tray (`ksni`) for manual toggle/quit/configurator actions.
3. Maintains two atomics:
   - `toggle_requested`: set by signals or tray to show/hide overlay.
   - `should_quit`: set by signals or tray quit item.
4. On toggle:
   - Launches (or terminates) the Wayland backend via `backend::run_wayland`.
   - Keeps track of overlay state so repeated toggles do the right thing.
5. On exit:
   - Signals the backend to shut down and joins the tray thread.

Daemon mode therefore provides a persistent background service that reacts to user keybinds (typically configured in the compositor to send SIGUSR1) or to tray actions.

---

## 3. Active Mode / Wayland Backend

**Modules:**
- `src/backend/mod.rs`: exported API (`run_wayland`)
- `src/backend/wayland/backend.rs`: high-level bootstrapper
- `src/backend/wayland/state.rs`: runtime state (surfaces, buffers, runtime handles)
- `src/backend/wayland/handlers/*.rs`: smithay trait impls (input, compositor, registry, etc.)

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

**Files:** `src/input/mod.rs`, `src/input/state/{core,actions,mouse,render}.rs`, `src/draw/*`, `src/ui.rs`

1. **Keyboard events (`handlers/keyboard.rs`)**
   - Translate Wayland keysyms to internal `Key`.
   - Call `InputState::on_key_press` / `on_key_release`.
   - After processing a key press, check `InputState::take_pending_capture_action` to trigger captures.

2. **Mouse events (`handlers/pointer.rs`)**
   - Update `current_mouse_x/y`.
   - Call `InputState::on_mouse_press`, `on_mouse_motion`, `on_mouse_release`.
   - Adjust pen thickness or font size via scroll wheel + modifiers.

3. **`InputState` responsibilities**
   - Holds canvas data (`draw::CanvasSet`), current colors, thickness, fonts, modifier flags, and `DrawingState` (Idle/Drawing/TextInput).
   - `actions.rs` maps keybindings to `Action` enums and performs side effects (color changes, board mode switches, capture requests).
   - `mouse.rs` converts drag gestures into shapes (`draw::Shape` variants).
   - `render.rs` exposes provisional shape previews for live feedback.

4. **Rendering to the overlay**
   - `WaylandState::render` uses Cairo + SHM buffers.
   - Draw order: board background → finalized shapes → provisional shape → text cursor preview → status bar (if enabled) → help overlay (if toggled).
   - `ui` module encapsulates status/help overlays, while `draw` handles actual vector geometry routines.

The result is a predictable pipeline: Wayland → handlers → `InputState` → `CanvasSet`/`DrawingState` → `WaylandState::render`.

---

## 5. Capture Pipeline

**New structure (all under `src/capture/`):**

| File/Folder | Purpose |
|-------------|---------|
| `mod.rs` | Public exports and shared submodules. |
| `manager.rs` | `CaptureManager` – owns channel, status, tokio task. |
| `dependencies.rs` | Trait definitions (`CaptureSource`, `CaptureFileSaver`, `CaptureClipboard`) and default implementations. |
| `pipeline.rs` | `perform_capture` and `CaptureRequest` definition. |
| `sources/` | Strategies for acquiring image bytes: Hyprland fast-path (`hyprland.rs`), portal fallback (`portal.rs`), and URI reader/cleanup (`reader.rs`). |
| `clipboard.rs`, `file.rs`, `portal.rs` | Support code reused by the pipeline. |
| `tests.rs` | Unit tests for the manager/pipeline, plus mocks. |

**Runtime flow:**
1. `InputState::handle_action` sets `pending_capture_action`.
2. Keyboard handler sees the pending action, calls `WaylandState::handle_capture_action`.
3. `WaylandState::handle_capture_action` builds a `CaptureRequest` (type + destination + save config) and calls `CaptureManager::request_capture`.
4. `CaptureManager`’s tokio task receives the request, updates status, and calls `perform_capture`.
5. `perform_capture`:
   - Calls the configured `CaptureSource` (default: `sources::capture_image` with Hyprland→portal fallback).
   - Optionally saves via `CaptureFileSaver`.
   - Optionally copies to clipboard via `CaptureClipboard`.
   - Returns `CaptureResult` used for desktop notifications.
6. `WaylandState` polls `CaptureManager::try_take_result()` to restore the overlay and emit notifications once capture completes.

Notifications are sent via `notification::send_notification_async`, keeping all UI feedback on the event loop thread.

---

## 6. Configuration & Legacy Support

- **`src/config/`** handles loading `config.toml`, validating fields, and building the keybinding map. It also houses migration helpers shared with `main.rs`.
- **`src/legacy.rs`** contains helpers to detect old binary names, environment overrides, and configurator paths. The daemon tray relies on it to launch the correct configurator binary.

---

## 7. Utility Modules

- **`src/draw/`**: Shape definitions, Cairo helpers, arrow geometry, fonts, and the `CanvasSet` abstraction (with undo/history per board mode).
- **`src/ui.rs`**: Composes the status bar and help overlay using Cairo.
- **`src/notification.rs`**: Tiny helper to send desktop notifications asynchronously (used after captures).
- **`src/util.rs`**: Misc helpers (color parsing, geometry math, etc.).
- **`tests/`**: Integration tests (CLI smoke tests, rendering sanity checks) live outside `src/`.

---

## 8. Directory Map (excluding configurator)

| Path | Role |
|------|------|
| `src/main.rs` | CLI entry point, mode selection, migration trigger. |
| `src/daemon.rs` | Background daemon, tray menu, signal handling, overlay toggling. |
| `src/backend/` | Wayland backend implementation split into bootstrap (`mod.rs`), runtime (`state.rs`), and input/render handlers. |
| `src/input/` | Event/state machine for drawing tools, board modes, and capture triggers. |
| `src/draw/` | Vector drawing primitives, canvases, fonts. |
| `src/ui.rs` | Status/help overlays. |
| `src/capture/` | Screenshot pipeline (manager, dependencies, sources, clipboard/file helpers). |
| `src/config/` | Config parsing, defaults, migration helpers. |
| `src/legacy.rs` | Compatibility notices and configurator path helpers. |
| `src/notification.rs` | Desktop notifications for capture results. |
| `src/util.rs` | Shared math/color utilities. |
| `tests/` | CLI + rendering integration tests. |

---

## 9. Putting It Together

1. **Launch** via CLI → choose daemon vs active.
2. **Daemon** provides lifecycle management, tray integration, and toggles the backend on demand.
3. **Backend** sets up Wayland surfaces and loops, forwarding input to `InputState`.
4. **InputState + draw/ui** update the overlay contents and request renders.
5. **Capture** subsystem handles screenshot actions asynchronously and notifies the user.
6. **Config/legacy** modules ensure user preferences and backward compatibility are honored everywhere.

Use this document to trace any feature: locate the entry point (CLI, tray, keybinding), follow it through the backend/input/capture stacks, and consult the relevant modules listed above for details.
