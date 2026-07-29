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

1. The app creates the authenticated process broker before any singleton lock. `Daemon::run` then
   acquires the daemon lock, installs the owned Unix signal listener and queue watcher, and
   publishes the strict v2 runtime identity only after every discovery source is active.
2. It optionally starts the status tray and portal global-shortcut listener.
3. Typed `--daemon-toggle` requests publish canonical, generation-bound controls and atomically
   rename an ordered queue reference. The watched queue wakes directly; typed discovery never uses
   a signal. Raw `SIGUSR1` remains only as a legacy argument-free visibility intent.
4. The control loop linearizes cancellation and effect commit under the durable decision lock.
   Overlay actions use the shared ordered journal, and the event-loop thread remains the sole
   action applier.
5. Overlay candidates and runtime helpers are created only by the pre-lock process broker. The
   daemon owns generation/pidfd decisions while the broker owns wait/reap; overlay readiness is
   accepted only after the child wins its lock and publishes matching process identity.
6. Queue renames, producer eventfds, signals, and child pidfds drive the loop without a lifecycle
   polling tick. Shutdown invalidates readiness, terminates owned work, and joins listeners.

The complete route, recovery, process-site, compatibility, and rollback contracts are documented
in [Daemon Protocol v2 and Process Ownership](daemon-protocol-v2.md).

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
1. `InputState::handle_action` records screenshot, export, and other backend-owned work in `pending_backend_action`; independently coalesced slots retain the authored badge and zoom-chip preference saves.
2. The Wayland event loop centrally drains that pending work, so keybindings, command-palette Return, and command-palette mouse clicks share the same dispatch path without the two preference saves overwriting one another.
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
- **`ConfigDocument`** is the single edit owner, and it has two callers:
  `configurator/src/app/io.rs`, reached only from the configurator's Save control, and the narrow
  editors in `src/config/io.rs`, one per explicit overlay gesture (below). It keeps
  validated `Config`, the authored pre-validation `Config`, the lossless TOML source, unknown-path
  diagnostics, source path, and exact source revision behind one interface.
  `save_with_backup` merges known fields while retaining comments and unsupported settings, copies
  the previous contents to a timestamped `.bak`, and writes through the durable atomic-write policy.
  Its editor load path can expose a defaults-based repair document for readable but invalid config,
  while true I/O failures leave the configurator's last good document untouched.
- A save records only the delta between the config the document loaded and the config its caller
  hands back. A value that loading clamped, normalized, deduplicated, or reset keeps the text the
  user authored, so editing one preference can never rewrite a setting nobody touched.
- The Performance section is the first bounded scalar-metadata slice: core config owns its field
  IDs, paths, labels, help/search terms, and numeric constraints while the configurator keeps typed
  draft fields and messages.

### Only an explicit user edit writes `config.toml`

- The invariant: `config.toml` changes only through an explicit user edit action, never as a side
  effect of running Wayscriber. There are exactly four such actions — the configurator's Save,
  which writes the whole edited draft, and the overlay's three narrow editors (shortcut, preset
  slot, quick color), which each rewrite one key. Everything else — the overlay's other controls,
  the daemon, the tray, startup, validation, migration preview, and shutdown — reads the file and
  leaves its bytes, mode, and mtime alone, including for a missing, read-only, or old-revision
  file. `tools/check-config-writers.py` pins that set by name.
- Every one of those writes goes through `ConfigDocument::save_with_backup`, which holds an
  advisory lock on a sibling `config.toml.lock` across the whole check-copy-rename window
  (`src/config/document/lock.rs`). The revision check and the atomic rename are separate syscalls
  in separate processes: without the lock the configurator and an overlay editor can both find the
  file unchanged and the second rename discards the first edit, with both reporting success and
  both `.bak` copies holding the same pre-edit source. With it the loser sees the file change and
  takes the reload-and-reapply path below. The wait is bounded; a lock another editor will not
  release is reported as `ConfigWriteLockTimeout` rather than waited on forever.
- That window is about one file, resolved once. `SourceRevision` records the symlink chain the
  load walked, and the lock, both byte comparisons, and the atomic rename all address the file at
  the end of it — the rename is handed that path with `SymlinkPolicy::Reject`, so nothing resolves
  the config path a second time. A link retargeted while the window is open is therefore either
  seen (`ensure_source_unchanged` reports a chain change as a stale source, which the editors
  reload and reapply through) or harmless (a retarget after the last comparison writes the file the
  window was about, leaving the new target untouched).
- The lock binds the writers that take it, which is every writer this application has and nobody
  else's. So `SourceRevision` also records *which file* it read — device and inode on Unix — and
  its exact bytes. The rename is conditional on finding that complete revision at the destination
  (`DestinationExpectation`, checked immediately before `rename`). An editor outside Wayscriber
  that replaces the checked `config.toml`, or truncates and rewrites the same file in place, is
  refused as a stale source rather than having its work overwritten by a merge of the old text; a
  load that found *nothing* expects to find nothing, so a config created in the window is not
  created over. Both halves of that check are one observation of one file — the contents are read
  through a handle whose own identity is confirmed, not through a second lookup of the name.
  Identity and bytes are also compared by `ensure_source_unchanged`. What remains is the gap
  between that final check and the rename: no rename takes a condition on the destination's
  revision, so a replacement landing there cannot be prevented — only narrowed to that gap. A
  *creation* has no such gap, because `Absent` is the one condition a rename can carry: a name that
  fills up is refused by `RENAME_NOREPLACE` itself, and reported in the same stale-source wording
  so the editors reload and reapply rather than seeing a failed save.
- Nothing writes the file on its own account. There is no automatic config writer, mutation
  enum, retry queue, backup directory, or flush lifecycle: the overlay's `config_writer.rs`
  worker, `ConfigMutation`, `Config::persist_pending_migrations`, `ConfigDocument::save_migration`,
  the tray's session-resume save, and `src/config/runtime_backup.rs` were all removed;
  `$XDG_STATE_HOME` no longer holds anything of Wayscriber's, and directories left by older
  releases are user data. The config-edit worker described below is not a return of any of that —
  it writes nothing of its own, and exists only to keep the three explicit gestures' writes off
  the thread that dispatches input.
- Incidental overlay preference controls (toolbar layout mode and section visibility, icon mode,
  status bar and badge flags, click highlight, input HUD, Step section) mutate the
  in-memory `Config` — the effective value — and write nothing. Restart restores the configured
  value. Each one is classified `Ephemeral` in `src/ui/toolbar/model/event_policy.rs` and pairs with
  honest wording plus a route into the configurator; the routes are named in
  `src/configurator_destination.rs` and launched from
  `src/input/state/core/utility/launcher.rs` (overlay) and `src/daemon/tray/helpers.rs` (tray).
- Board edits are not in that list, and are not a third kind of `config.toml` write either. Board
  contents — rename, recolor, add, delete, and everything drawn on them — belong to the session:
  they mark the session dirty, ride the session autosave, and come back on the next start for
  boards marked `persist`. A board *pin* is a direct UI preference and goes to `runtime-ui.toml`
  with the rest of them. `config.toml`'s `[boards]` holds the boards a new session starts from, and
  only the configurator writes it.
- Three overlay gestures are deliberate edits and do write, through the narrow editors in
  `src/config/io.rs`: `persist_keybinding_edit`, `persist_preset_slot`, and `persist_quick_color`.
  All three share `edit_one_config_key`, which loads a `ConfigDocument`, clones
  `document.config()`, sets exactly one value, and calls `save_with_backup`. Using the validated
  `config()` as the base — not `authored_config()` — is what limits the write: it is the same value
  the merge gate receives as `previous`, so the one field the closure sets is the only difference
  the gate can see. An unparseable file is refused rather than rebuilt from defaults, a
  changed-on-disk conflict is reloaded and reapplied once, and the value is confirmed against the
  document parsed from the bytes the save wrote — the merge output, not a fresh read of the file —
  so a value validation declined on the way out is reported instead of assumed durable.
- `edit_one_config_key` also decides whether there is anything to write: it compares the validated
  edit against the loaded config the way the merge gate does, and an edit the file already resolves
  to returns `ConfigEditWrite::AlreadyCurrent` without a write or a `.bak`, so the three call sites
  can say "already" instead of claiming a save. A write that lands but does not read back is a
  distinct `ConfigEditNotReadBack`, because the file did change.
- Two layers decide a chord. The overlay's own check runs `claimed_keys()` on this run's keymap
  with the shortcut deltas already queued and unanswered folded in, in submission order
  (`ConfigEditWorker::projected_shortcuts`): nothing is installed until a write reports back, so
  the keymap alone still shows the bindings an outstanding edit has asked to move, and a gesture
  reaching for a chord that edit gave up would be refused over a claim the file is about to drop.
  Each projection is retired when its completion is taken, whatever the outcome, so the keymap is
  the authority again the moment there is nothing outstanding.
- The write is the second layer and the arbiter. The shortcut editor re-checks the requested chord
  against the freshly loaded document's `claimed_keys()` before touching anything, because the
  projection only knows about this run's queue and the file may have been given the chord by
  another window, the configurator, or a hand edit. A chord another action now owns returns `ShortcutClaimedOnDisk`,
  the write is refused, and the completion handler — the only place a keymap is installed — leaves
  the run as it was and names the owner. The edited action's own `[keybindings]` key is then marked
  authored (`Config::mark_keybinding_explicit`) so the omitted-default pass cannot re-classify the
  list the editor just typed; the other keys keep the file's own presence.
- The quick-color editor materializes the palette array only as far as the edited slot, so later
  slots stay implied and keep tracking the shipped defaults, and `create_config_backup` claims its
  timestamped name with `create_new`, suffixing on collision, so two edits inside one second leave
  two backups. Alias canonicalization in `document/merge.rs` is limited to aliases the save's delta
  actually reaches, so a narrow edit cannot respell an unrelated legacy key.
- All three run off the dispatch thread. `src/backend/wayland/config_edits.rs` owns a lazily
  spawned worker thread and a bounded FIFO channel; the gestures hand it a typed `ConfigEdit` and
  the event loop drains typed completions in `event_loop/capture.rs`, woken by the same
  `RuntimeWakeHandle` eventfd the other background workers use. Edits execute one at a time in
  submission order and are answered in that order. Nothing is ever completed on the spot: a
  submission joins a staging queue in front of the channel and is pumped in as the worker makes
  room, so a burst that fills the channel cannot answer the newest gesture ahead of the older ones
  it was made after (which would leave their completions applying on top of it). That module is the
  only production caller of the three editors, and is what `tools/check-config-writers.py` pins.
- Teardown is `finish_config_edits` (called by `shutdown_config_edits`, beside
  `shutdown_runtime_ui`). It drains the edit-bearing pending slots — preset action, quick-color
  recolor, recorded shortcut edits — one last time before stopping the worker, because a gesture
  and the exit that follows it can arrive in the same batch of input events and the loop breaks
  before the pass that would have queued it. Then it waits a bounded five seconds for the channel
  *and* the staging queue, so an edit made a moment before quitting still lands. Those completions
  are logged rather than shown: there is no overlay left to toast on, and the write is the half the
  user cannot redo from memory. Any pending slot added later whose drain queues a `ConfigEdit`
  belongs in that function too.
- The gestures are decided in `src/backend/wayland/state/keybindings.rs` (palette row controls and
  the toolbar rebind gesture, which queue onto `InputState::pending_keybinding_edits` — a FIFO,
  not the single-slot `PendingBackendAction`, so two edits recorded from one batch of input events
  both reach the worker — after conflict-checking the request against `claimed_keys()` for every
  action but the one being edited),
  `.../toolbar/events/presets.rs`, and `.../toolbar/events/quick_colors.rs` (drained from the color
  picker's pointer release in `event_loop/capture.rs`, which is the only drain site that release
  reaches). The two families differ on purpose:
  - Presets and quick colors apply in memory as the gesture completes — the live slot or swatch is
    the feedback the gesture is for — and only the *wording* waits for the write, so a toast never
    claims a durable change before the file has one. A failed write degrades to a this-run change
    with a toast that says the file missed it, never to a lost edit.
  - A shortcut installs nothing until the write reports success. `prepare_keybinding_edit` takes
    the running keymap by shared reference and hands the write a *delta* — one action and the
    bindings it should end up with — so the chord exists nowhere but the queued edit until the file
    answers; `shortcut_completion` then decides from that answer whether it is folded in at all,
    and `install_keybinding_edit` folds it into the keymap the run holds by then rather than into a
    copy taken when the edit was accepted. A chord the file has given to another action since this
    run read it is refused with nothing to roll back, and every other failure still installs for
    the run with honest wording — the same four outcomes the synchronous version produced. A second
    shortcut edit issued while the first is in flight is checked against the keymap the run still
    holds and, if the two contest a chord, is caught by the same on-disk refusal at write time; if
    they contest nothing, both land, because neither completion touches the other's action. The one
    pair that reaches neither outcome is a first edit whose *write* failed and kept its chord for
    the run: the file never got it, so a second edit onto that chord is refused by the run instead,
    and the wording branches on what the file did with that second edit — "saved to config.toml,
    but this run kept its own" when the file took it, and a message claiming no save at all when it
    did not.
- Section-visibility and layout changes still call `refresh_runtime_ui_config_seeds()` from the
  apply path, because the runtime-UI store seeds off the effective config rather than off a write.
- Loading is read-only including for old revisions. `Config::apply_keybinding_migrations` is
  preview material only: `src/config/migration.rs` turns the recipes into a `MigrationPreview` the
  configurator shows as a review banner, Apply edits the draft, and the ordinary Save persists it.
  `validate_and_clamp` never calls it and never advances `config_revision`.
- Omitted `[keybindings]` fields are resolved from source presence
  (`KeybindingAuthorship`, populated by both parse paths) rather than by comparing values against
  compiled defaults, so a shipped default is only ever installed on a key nothing authored claims;
  a stand-down is reported as `DefaultShortcutSkipped`.
- An editor that rebuilds `[keybindings]` from its own fields calls
  `Config::mark_keybindings_explicit` before validating — `ConfigDraft::to_config` does — because
  presence in the loaded file no longer describes lists the user typed. A duplicate they typed is
  then arbitrated by traversal order instead of being filtered away as an unauthored default, and
  the configurator's save status names which action kept the key: the resolution reaches
  `config.toml`, so the reloaded document has nothing left to report.
- Two guards keep it that way: `tools/check-config-writers.py` (in `tools/lint-and-test.sh`) fails
  when any source outside `src/config/document.rs`, `src/config/io.rs`, and
  `configurator/src/app/io.rs` names a config write primitive, when an unpinned file calls one of
  the narrow editors, or when the editors' path-taking `_at` twins stop being `#[cfg(test)]`-gated
  (a production build has no such function at all); it reads test-only status from the
  `#[cfg(test)]` on the `mod` item rather than from the shape of the path, so a file under a
  directory named `tests` is not exempt by accident. Alongside it,
  `no_daemon_source_can_write_the_config` in `src/daemon/tests.rs` does the same for the daemon
  subtree under `cargo test`. The behavioural proof is `src/config/tests/immutability.rs`, which
  snapshots bytes, length, mtime, and mode around every loader.

### Runtime UI preference persistence

- `src/runtime_ui_state/` owns the versioned wire model, seed/override reconciliation, guarded
  mutation pipeline, exact source revisions, pinned-directory store operations, recovery barriers,
  cancellation capabilities, and per-mutation durability outcomes.
- Every seed target is runtime-owned: pins, minimize, side pane, collapsed sections, item
  visibility/order, board pins, both toolbar positions, and the top strip's display form. Authored
  `config.toml` values are the seeds; direct manipulation writes overrides. `top_position`,
  `side_position`, and `top_display_mode` were added to wire V1 additively — an older build decodes
  them as unknown keys and preserves them verbatim, which a version bump would not allow.
- The persisted display form is `full`/`micro` only. The cycle action's `hidden` rung and presenter
  mode's forced mapping stay live-only; the override is computed with `TopDisplayMode::persisted()`
  and presenter-restore precedence so neither can be written.
- A committed side drag stages both position overrides in one mutation scope because completing it
  reconciles the top strip's horizontal base. Retained position overrides are applied on top of the
  authored seeds at startup and clamped on the first apply against real output geometry, not on
  load, so an override recorded on a disconnected monitor degrades instead of being discarded.
- `src/backend/wayland/runtime_ui_state.rs` adapts toolbar and board interactions to that controller.
  `coordinator.rs` owns previews and writer transport, `lifecycle.rs` retains the exact active
  incident/recovery capabilities and publishes safe toolbar diagnostics, and `wayland.rs` applies
  rollback or rebuilt live authority through normal toolbar transition cleanup.
- Startup inspects the generated runtime-state file after config and board seeds exist. Config and
  session reloads update the same seed registry in process; generation guards invalidate previews
  created from stale seeds.
- The storage worker accepts only conditional operations against an inspected source and verified
  directory identity. Supported V1 writes preserve passthrough data. Unsupported/invalid reset
  paths retain the original bytes as recovery artifacts, while uncertain outcomes keep a barrier
  active until reinspection establishes authority.
- Toolbar Settings is the presentation boundary for supported reset, unsupported-version
  confirmation, unhealthy retry/adopt/preserve-reset actions, cancellation state, and complete
  diagnostic/artifact paths. The actor retains recovery cancellation and completion ownership until
  the controller terminalizes the exact attempt.

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
- **`src/notification.rs`**: Tiny helper to send desktop notifications asynchronously (used after captures and by the update notice).
- **`src/update_check/`**: Compares the running version against the manifest published on
  wayscriber.com and caches the answer. The daemon's `update_watch.rs` polls it, the About
  window's update card reads it, and `--check-update` forces one check. Nothing is
  downloaded or installed; see `src/update_check/AGENTS.md`.
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
| `src/process_broker/` | Pre-lock, bounded runtime helper creation and broker-only child reaping. |
| `src/backend/` | Wayland backend implementation split into bootstrap (`mod.rs`), runtime (`state.rs`), input/render handlers, and the `runtime_ui_state/` preference store. |
| `src/input/` | Event/state machine, tools, board/page ownership, selection, and action routing. |
| `src/draw/` | Vector drawing primitives, frames/pages, history, fonts, and rendering helpers. |
| `src/ui.rs` | Status/help overlays. |
| `src/capture/` | Screenshot pipeline (manager, dependencies, sources, clipboard/file helpers). |
| `src/config/` | Config parsing, defaults, keybinding map. |
| `src/runtime_ui_state/` | Generated UI preference wire format, seed registry, guarded persistence, reset, and recovery state machines. |
| `src/session/` | Configured and named session persistence, snapshots, sidecars, locks, and catalog metadata. |
| `src/notification.rs` | Desktop notifications for capture results and the update notice. |
| `src/update_check/` | Opt-outable "a newer release exists" check: version ordering, manifest trust rules, `curl`/`wget` transport, cache. Installs nothing. |
| `src/about_window/` | Standalone About dialog: content/layout/interaction split, update card, diagnostics copy. |
| `src/util/` | Shared math, color, arrow, and text utilities. |
| `tests/` | CLI + rendering integration tests. |

---

## 12. Putting It Together

1. **Launch** via CLI → choose daemon vs active.
2. **Daemon** provides lifecycle management, tray integration, and toggles the backend on demand.
3. **Backend** sets up Wayland surfaces and loops, forwarding input to `InputState`.
4. **InputState + draw/ui** update the overlay contents and request renders.
5. **Capture** subsystem handles screenshot actions asynchronously and notifies the user.
6. **Runtime UI state** layers direct toolbar/board preferences over configured seeds and settles guarded writes or recovery barriers.
7. **Session** loads and saves configured or named session state, including runtime Open/Save As/Clear transactions.
8. **Config** module ensures user preferences are honored everywhere.

Use this document to trace any feature: locate the entry point (CLI, tray, keybinding), follow it through the backend/input/capture stacks, and consult the relevant modules listed above for details.
