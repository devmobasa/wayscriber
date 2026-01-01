# Contributing to wayscriber

Thanks for your interest in improving wayscriber. This guide covers development setup,
useful scripts, and the current project structure.

## Where help is most valuable

Priority areas for contributions:
- Compositor compatibility testing
- Click-through option while the overlay is active
- Multi-monitor support
- New drawing tools

## Development

Run the overlay locally:

```bash
cargo run -- --active
```

Run the GUI configurator:

```bash
cargo run -p wayscriber-configurator
```

## Tests and linting

Use one of the local CI scripts (they run the same checks):

```bash
./tools/lint-and-test.sh
```

If you are working offline, prefetch dependencies first:

```bash
./tools/fetch-all-deps.sh
```

## Project structure

### Root files

- `Cargo.toml` - Workspace metadata, crate info, features, and dependencies.
- `Cargo.lock` - Locked dependency versions for reproducible builds.
- `README.md` - User-facing overview, install, usage, and troubleshooting.
- `LICENSE` - MIT license text.
- `build.rs` - Build-time config for feature aliases via `cfg_aliases`.
- `clean.sh` - Local cleanup helper.
- `config.example.toml` - Annotated configuration template shipped to users.

### Top-level directories

#### `assets/`
- `tray_icon.png` - Tray icon for daemon mode.

<details>
<summary>configurator/ (separate GUI configurator crate)</summary>

- `Cargo.toml` - Configurator crate manifest.
- `Cargo.lock` - Configurator lockfile.
- `README.md` - Configurator-specific readme.
- `src/main.rs` - Configurator entry point.
- `src/messages.rs` - UI message types and dispatch.
- `src/app/` - App state, update logic, and views (Iced UI).
  - `entry.rs` - App entry wiring.
  - `io.rs` - File IO helpers for config read/write.
  - `mod.rs` - Module glue.
  - `state.rs` - Top-level app state.
  - `update/` - Update handlers for user actions.
    - `config.rs` - Update logic for config-wide changes.
    - `fields.rs` - Update logic for field edits.
    - `presets.rs` - Update logic for preset slots.
    - `tabs.rs` - Tab switching logic.
    - `mod.rs` - Update module wiring.
  - `view/` - UI view components grouped by config section.
    - `arrow.rs` - Arrow tool UI.
    - `board.rs` - Board and page UI.
    - `capture.rs` - Screenshot capture UI.
    - `drawing/` - Drawing tool UI.
      - `color.rs` - Drawing color UI.
      - `font.rs` - Drawing font UI.
      - `mod.rs` - Drawing view wiring.
    - `history.rs` - Undo/redo history UI.
    - `keybindings.rs` - Keybinding UI.
    - `performance.rs` - Performance tuning UI.
    - `presets/` - Preset editor UI.
      - `mod.rs` - Preset view wiring.
      - `slot/` - Preset slot controls.
        - `color.rs` - Preset color entry.
        - `header.rs` - Slot header UI.
        - `rows.rs` - Slot row layout.
        - `mod.rs` - Slot module wiring.
    - `session.rs` - Session persistence UI.
    - `tablet.rs` - Tablet input UI.
    - `ui/` - UI/UX settings views.
      - `click_highlight.rs` - Click highlight settings.
      - `help_overlay.rs` - Help overlay settings.
      - `status_bar.rs` - Status bar settings.
      - `toolbar.rs` - Toolbar settings.
      - `mod.rs` - UI section wiring.
    - `widgets/` - Reusable UI widgets.
      - `colors.rs` - Color picker widgets.
      - `constants.rs` - UI constants and shared styles.
      - `inputs.rs` - Text input widgets.
      - `labels.rs` - Label styles.
      - `validation.rs` - Validation indicators.
      - `mod.rs` - Widget module wiring.
    - `mod.rs` - View wiring.
- `src/models/` - Configurator models and mapping to/from core config.
  - `color/` - Color model helpers.
    - `input.rs` - Color input parsing.
    - `named.rs` - Named color mapping.
    - `quad.rs` - Quad color models.
    - `triplet.rs` - RGB triplet helpers.
    - `tests.rs` - Color model tests.
    - `mod.rs` - Color module wiring.
  - `config/` - Draft config model and conversion routines.
    - `draft.rs` - Draft config structure.
    - `parse.rs` - TOML parsing into draft.
    - `presets.rs` - Preset defaults and conversions.
    - `setters.rs` - Draft setters.
    - `tests.rs` - Config draft tests.
    - `toolbar_overrides.rs` - Toolbar override mapping.
    - `to_config/` - Convert draft into core config.
      - `board.rs` - Board config conversion.
      - `capture.rs` - Capture config conversion.
      - `drawing.rs` - Drawing config conversion.
      - `history.rs` - History config conversion.
      - `keybindings.rs` - Keybinding config conversion.
      - `performance.rs` - Performance config conversion.
      - `presets.rs` - Preset conversion.
      - `session.rs` - Session conversion.
      - `tablet.rs` - Tablet conversion.
      - `ui.rs` - UI conversion.
      - `mod.rs` - Conversion wiring.
    - `mod.rs` - Config module wiring.
  - `error.rs` - Model error types.
  - `fields/` - Field types and validation helpers.
    - `board.rs` - Board fields.
    - `eraser.rs` - Eraser fields.
    - `font.rs` - Font fields.
    - `session.rs` - Session fields.
    - `status.rs` - Status bar fields.
    - `toggles.rs` - Toggle fields.
    - `tool.rs` - Tool fields.
    - `toolbar.rs` - Toolbar fields.
    - `tests.rs` - Field tests.
    - `mod.rs` - Field module wiring.
  - `keybindings/` - Configurator keybinding models.
    - `draft.rs` - Draft keybinding model.
    - `parse.rs` - Parsing helpers.
    - `tests.rs` - Keybinding model tests.
    - `field/` - Field-level keybinding helpers.
      - `config.rs` - Field config mapping.
      - `labels.rs` - Human-readable labels.
      - `list.rs` - Key list helpers.
      - `tab.rs` - Tab grouping.
      - `mod.rs` - Field module wiring.
    - `mod.rs` - Keybinding module wiring.
  - `tab.rs` - Tab definitions.
  - `util.rs` - Shared model helpers.
  - `mod.rs` - Model module wiring.

</details>

#### `docs/`
- `CONFIG.md` - User configuration guide.
- `SETUP.md` - Installation and setup notes.
- `codebase-overview.md` - High-level architecture notes.

#### `packaging/`
- `PKGBUILD` - Arch packaging recipe.
- `package.configurator.yaml` - Packaging manifest for configurator.
- `package.wayscriber.yaml` - Packaging manifest for wayscriber.
- `wayscriber.desktop` - Desktop entry for the overlay.
- `wayscriber-configurator.desktop` - Desktop entry for configurator.
- `wayscriber.service` - Systemd user service unit.
- `icons/` - PNG app icons.
  - `wayscriber-24.png` - 24px icon.
  - `wayscriber-64.png` - 64px icon.
  - `wayscriber-128.png` - 128px icon.
- `wayscriber-configurator-24.png` - 24px configurator icon.
- `wayscriber-configurator-64.png` - 64px configurator icon.
- `wayscriber-configurator-128.png` - 128px configurator icon.

#### `tests/`
- `cli.rs` - CLI integration tests.
- `ui.rs` - UI smoke/integration tests.

#### `tools/`
- `README.md` - Tooling overview.
- `build.sh` - Build helper.
- `build-package-repos.sh` - Build package repositories.
- `bump-version.sh` - Version bump helper.
- `create-release-tag.sh` - Create release tags.
- `fetch-all-deps.sh` - Prefetch dependencies for offline builds.
- `install-configurator.sh` - Install the configurator.
- `install.sh` - Install wayscriber.
- `lint-and-test.sh` - Local lint + test runner.
- `package.sh` - Build packages.
- `publish-release-tag.sh` - Publish release tags.
- `reload-daemon.sh` - Reload the daemon systemd unit.
- `test.sh` - Test wrapper.
- `update-aur-from-manifest.sh` - Update AUR metadata from manifests.
- `update-aur.sh` - AUR update helper.

### `src/` (main crate) detailed map

#### Root files in `src/`
- `src/main.rs` - CLI entry point; selects daemon/overlay execution.
- `src/lib.rs` - Library root for shared modules and tests.
- `src/cli.rs` - CLI definitions and argument parsing (clap).
- `src/notification.rs` - Desktop notification helpers.
- `src/session_override.rs` - CLI overrides for session persistence behavior.
- `src/time_utils.rs` - Time formatting helpers.
- `src/ui.rs` - UI module wiring (status, help overlay, context menu, toasts).

#### `src/about_window/`
- `src/about_window/mod.rs` - About window module wiring.
- `src/about_window/state.rs` - About window state.
- `src/about_window/clipboard.rs` - Clipboard support for the about dialog.
- `src/about_window/handlers/` - Wayland handlers for about window.
  - `src/about_window/handlers/mod.rs` - Handler wiring.
  - `src/about_window/handlers/compositor.rs` - Compositor callbacks.
  - `src/about_window/handlers/dispatch.rs` - Event dispatch integration.
  - `src/about_window/handlers/keyboard.rs` - Keyboard input handling.
  - `src/about_window/handlers/output.rs` - Output enter/leave handling.
  - `src/about_window/handlers/pointer.rs` - Pointer input handling.
  - `src/about_window/handlers/registry.rs` - Wayland registry wiring.
  - `src/about_window/handlers/seat.rs` - Seat and input device setup.
  - `src/about_window/handlers/shm.rs` - Shared memory setup.
  - `src/about_window/handlers/window.rs` - Window/layer handling.
- `src/about_window/render/` - Rendering helpers for the about dialog.
  - `src/about_window/render/mod.rs` - Render module wiring.
  - `src/about_window/render/draw.rs` - Draw helpers.
  - `src/about_window/render/text.rs` - Text rendering helpers.
  - `src/about_window/render/widgets.rs` - Small UI widgets.

#### `src/app/`
- `src/app/mod.rs` - App module wiring.
- `src/app/env.rs` - Environment helpers.
- `src/app/session.rs` - Session CLI helpers.
- `src/app/usage.rs` - Usage output formatting.

#### `src/backend/`
- `src/backend/mod.rs` - Backend abstraction module.
- `src/backend/wayland/` - Wayland backend implementation.
  - `src/backend/wayland/mod.rs` - Wayland backend entry.
  - `src/backend/wayland/capture.rs` - Capture integration hooks.
  - `src/backend/wayland/frozen_geometry.rs` - Geometry helpers for frozen/zoom.
  - `src/backend/wayland/overlay_passthrough.rs` - Overlay passthrough mode.
  - `src/backend/wayland/session.rs` - Session integration helpers.
  - `src/backend/wayland/state.rs` - Shared state wiring.
  - `src/backend/wayland/surface.rs` - Surface management.
  - `src/backend/wayland/toolbar_intent.rs` - Toolbar intent/state helpers.
  - `src/backend/wayland/backend/` - Event loop core and initialization.
    - `src/backend/wayland/backend/mod.rs` - Backend core wiring.
    - `src/backend/wayland/backend/helpers.rs` - Shared backend helpers.
    - `src/backend/wayland/backend/run.rs` - Main run loop entry.
    - `src/backend/wayland/backend/setup.rs` - Wayland setup routines.
    - `src/backend/wayland/backend/signals.rs` - Signal handling.
    - `src/backend/wayland/backend/surface.rs` - Surface init/configuration.
    - `src/backend/wayland/backend/tray.rs` - Tray wiring for daemon mode.
    - `src/backend/wayland/backend/event_loop/` - Event loop phases.
      - `capture.rs` - Capture pipeline integration.
      - `dispatch.rs` - Calloop dispatch integration.
      - `render.rs` - Render pass orchestration.
      - `session_save.rs` - Session persistence on exit.
      - `mod.rs` - Event loop wiring.
    - `src/backend/wayland/backend/state_init/` - Initial state setup.
      - `config.rs` - Config-based initialization.
      - `input_state.rs` - Input state initialization.
      - `output.rs` - Output and monitor setup.
      - `session.rs` - Session load setup.
      - `tablet.rs` - Tablet init.
      - `mod.rs` - Init module wiring.
  - `src/backend/wayland/frozen/` - Frozen screen snapshot pipeline.
    - `capture.rs` - Screencopy capture setup.
    - `image.rs` - Frozen image handling.
    - `portal.rs` - Portal-based capture fallback.
    - `state.rs` - Frozen state machine.
    - `mod.rs` - Module wiring.
  - `src/backend/wayland/handlers/` - Wayland protocol handlers.
    - `activation.rs` - XDG activation.
    - `buffer.rs` - Buffer management.
    - `compositor.rs` - Compositor callbacks.
    - `layer.rs` - Layer-shell handling.
    - `output.rs` - Output enter/leave handling.
    - `pointer_constraints.rs` - Pointer constraint setup.
    - `registry.rs` - Registry discovery.
    - `relative_pointer.rs` - Relative pointer handling.
    - `seat.rs` - Seat setup.
    - `shm.rs` - Shared memory setup.
    - `screencopy.rs` - Screencopy protocol.
    - `xdg.rs` - XDG surface handling.
    - `keyboard/` - Keyboard handling.
      - `mod.rs` - Keyboard handler implementation.
      - `translate.rs` - Keysym to internal key mapping.
    - `pointer/` - Pointer event handling.
      - `axis.rs` - Scroll/axis events.
      - `cursor.rs` - Cursor management.
      - `enter_leave.rs` - Pointer enter/leave events.
      - `motion.rs` - Pointer motion events.
      - `press.rs` - Button press events.
      - `release.rs` - Button release events.
      - `mod.rs` - Pointer handler wiring.
    - `tablet/` - Tablet and stylus handling.
      - `device.rs` - Tablet device handling.
      - `manager.rs` - Tablet manager setup.
      - `pad.rs` - Tablet pad input.
      - `pad_group.rs` - Pad group wiring.
      - `pad_ring.rs` - Pad ring input.
      - `pad_strip.rs` - Pad strip input.
      - `seat.rs` - Tablet seat integration.
      - `tool.rs` - Stylus tool handling.
      - `mod.rs` - Tablet handler wiring.
  - `src/backend/wayland/state/` - Wayland runtime state and rendering.
    - `activation.rs` - Activation state.
    - `capture.rs` - Capture state.
    - `data.rs` - Core state data structures.
    - `helpers.rs` - State helpers.
    - `tests.rs` - State tests.
    - `toolbar.rs` - Toolbar state plumbing.
    - `zoom.rs` - Zoom state plumbing.
    - `core/` - Core state accessors and init.
      - `accessors.rs` - Shared accessors.
      - `init.rs` - State init glue.
      - `output.rs` - Output identity helpers.
      - `overlay.rs` - Overlay state helpers.
      - `mod.rs` - Module wiring.
    - `render/` - Overlay render pipeline.
      - `mod.rs` - Render wiring.
      - `ui.rs` - UI render passes.
      - `tool_preview.rs` - Tool preview rendering.
      - `canvas/` - Canvas rendering.
        - `background.rs` - Background painting.
        - `overlays.rs` - Overlay render layers.
        - `text.rs` - Text rendering.
        - `mod.rs` - Canvas render wiring.
    - `toolbar/` - Toolbar runtime state and layout helpers.
      - `events.rs` - Toolbar event definitions.
      - `geometry.rs` - Toolbar geometry calculations.
      - `drag/` - Drag handlers for toolbar.
        - `base.rs` - Common drag state.
        - `clamp.rs` - Bounds clamps.
        - `move_drag.rs` - Move drag logic.
        - `relative.rs` - Relative drag logic.
        - `mod.rs` - Drag wiring.
      - `inline/` - Inline toolbar state.
        - `drag.rs` - Inline drag handling.
        - `focus.rs` - Inline focus handling.
        - `input.rs` - Inline input routing.
        - `render.rs` - Inline rendering.
        - `mod.rs` - Inline wiring.
      - `visibility/` - Toolbar visibility state.
        - `access.rs` - Visibility accessors.
        - `pointer.rs` - Pointer-driven visibility.
        - `sync.rs` - Visibility sync helpers.
        - `tests.rs` - Visibility tests.
        - `mod.rs` - Visibility wiring.
  - `src/backend/wayland/toolbar/` - Toolbar layout, rendering, and surfaces.
    - `events.rs` - Toolbar event types.
    - `hit.rs` - Hit-testing for toolbar regions.
    - `render.rs` - Toolbar render entry.
    - `mod.rs` - Toolbar module wiring.
    - `layout/` - Toolbar layout definitions.
      - `mod.rs` - Layout wiring.
      - `side/` - Side palette layout.
        - `actions.rs` - Action rows layout.
        - `colors.rs` - Color rows layout.
        - `delay.rs` - Delay slider layout.
        - `header.rs` - Header layout.
        - `pages.rs` - Pages layout.
        - `presets.rs` - Presets layout.
        - `settings.rs` - Settings layout.
        - `sliders.rs` - Slider layout.
        - `mod.rs` - Side layout wiring.
      - `spec/` - Layout constants and specs.
        - `top.rs` - Top strip spec.
        - `side/` - Side palette specs.
          - `constants.rs` - Layout constants.
          - `sizes.rs` - Size tables.
          - `mod.rs` - Spec wiring.
        - `mod.rs` - Spec wiring.
      - `top/` - Top strip layout.
        - `icons.rs` - Icon layout.
        - `text.rs` - Text layout.
        - `mod.rs` - Top layout wiring.
      - `tests/` - Layout tests.
        - `mod.rs` - Test module.
    - `main/` - Toolbar main state and update logic.
      - `input.rs` - Input handling.
      - `lifecycle.rs` - Lifecycle helpers.
      - `render.rs` - Rendering hooks.
      - `state.rs` - Toolbar state.
      - `structs.rs` - Toolbar structs and data types.
      - `mod.rs` - Main module wiring.
    - `render/` - Toolbar render pieces.
      - `side_palette/` - Side palette renderers.
        - `actions.rs` - Action rows.
        - `colors.rs` - Color rows.
        - `header.rs` - Header render.
        - `marker.rs` - Marker rows.
        - `pages.rs` - Page controls.
        - `settings.rs` - Settings rows.
        - `text.rs` - Text rows.
        - `thickness.rs` - Thickness rows.
        - `mod.rs` - Side palette wiring.
        - `presets/` - Preset renderer.
          - `format.rs` - Preset format helpers.
          - `header.rs` - Preset header render.
          - `widgets.rs` - Preset widgets.
          - `mod.rs` - Preset wiring.
          - `slot/` - Preset slot rendering.
            - `actions.rs` - Slot actions.
            - `content.rs` - Slot content.
            - `feedback.rs` - Slot feedback.
            - `mod.rs` - Slot wiring.
        - `step/` - Custom step UI.
          - `custom_rows.rs` - Custom step rows.
          - `delay_sliders.rs` - Delay slider rows.
          - `mod.rs` - Step wiring.
      - `top_strip/` - Top strip renderers.
        - `text.rs` - Top strip text.
        - `mod.rs` - Top strip wiring.
        - `icons/` - Top strip icons.
          - `shape_picker.rs` - Shape picker icons.
          - `tool_row.rs` - Tool row icons.
          - `utility_row.rs` - Utility icons.
          - `mod.rs` - Icon wiring.
      - `widgets/` - Reusable toolbar widgets.
        - `background.rs` - Backgrounds.
        - `buttons.rs` - Button widgets.
        - `checkbox.rs` - Checkbox widgets.
        - `color.rs` - Color swatches.
        - `icons.rs` - Icon helpers.
        - `labels.rs` - Label text.
        - `primitives.rs` - Primitive shapes.
        - `tooltip.rs` - Tooltip rendering.
        - `mod.rs` - Widget wiring.
    - `surfaces/` - Toolbar surfaces and buffers.
      - `hit.rs` - Hit test wiring.
      - `lifecycle.rs` - Surface lifecycle.
      - `render.rs` - Surface rendering.
      - `state.rs` - Surface state.
      - `structs.rs` - Surface structs.
      - `mod.rs` - Surface wiring.
  - `src/backend/wayland/zoom/` - Zoom feature pipeline.
    - `capture.rs` - Zoom capture.
    - `portal.rs` - Portal fallback.
    - `state.rs` - Zoom state machine.
    - `view.rs` - Zoom view transform.
    - `mod.rs` - Module wiring.

#### `src/bin/`
- `src/bin/dump_config_schema.rs` - CLI for config JSON schema export.

#### `src/capture/`
- `src/capture/mod.rs` - Capture module wiring.
- `src/capture/clipboard.rs` - Clipboard output handling.
- `src/capture/dependencies.rs` - Runtime dependency checks.
- `src/capture/file.rs` - File output handling.
- `src/capture/manager.rs` - Capture orchestration.
- `src/capture/pipeline.rs` - Capture pipeline steps.
- `src/capture/portal.rs` - Portal capture integration.
- `src/capture/types.rs` - Capture data types.
- `src/capture/sources/` - Capture source backends.
  - `frozen.rs` - Frozen screenshot source.
  - `hyprland.rs` - Hyprland capture source.
  - `portal.rs` - Portal capture source.
  - `reader.rs` - Shared capture reader.
  - `mod.rs` - Source wiring.
- `src/capture/tests/` - Capture tests and fixtures.
  - `fixtures.rs` - Test fixtures.
  - `manager.rs` - Manager tests.
  - `perform_capture.rs` - End-to-end capture tests.
  - `placeholder.rs` - Placeholder tests.
  - `mod.rs` - Test wiring.

#### `src/config/`
- `src/config/mod.rs` - Config module wiring.
- `src/config/core.rs` - Core config structure.
- `src/config/enums.rs` - Config enums.
- `src/config/io.rs` - Read/write helpers.
- `src/config/keybindings.rs` - Keybinding loader/adapter glue.
- `src/config/paths.rs` - Config file path helpers.
- `src/config/schema.rs` - JSON schema export.
- `src/config/test_helpers.rs` - Test fixtures for config.
- `src/config/keybindings/` - Keybinding types, defaults, and maps.
  - `actions.rs` - Action enums.
  - `binding.rs` - Key binding parsing and display.
  - `tests.rs` - Keybinding tests.
  - `config/` - Map/build logic.
    - `map/` - Action map builders.
      - `board.rs` - Board bindings map.
      - `capture.rs` - Capture bindings map.
      - `colors.rs` - Color bindings map.
      - `core.rs` - Core bindings map.
      - `presets.rs` - Preset bindings map.
      - `selection.rs` - Selection bindings map.
      - `tools.rs` - Tool bindings map.
      - `ui.rs` - UI bindings map.
      - `zoom.rs` - Zoom bindings map.
      - `mod.rs` - Map wiring.
    - `types/` - Keybinding config structs.
      - `bindings/` - Per-domain binding lists.
        - `board.rs` - Board binding config.
        - `capture.rs` - Capture binding config.
        - `colors.rs` - Color binding config.
        - `core.rs` - Core binding config.
        - `presets.rs` - Preset binding config.
        - `selection.rs` - Selection binding config.
        - `tools.rs` - Tool binding config.
        - `ui.rs` - UI binding config.
        - `zoom.rs` - Zoom binding config.
        - `mod.rs` - Binding wiring.
      - `mod.rs` - Type wiring.
    - `mod.rs` - Keybinding config wiring.
  - `defaults/` - Default keybinding lists.
    - `board.rs` - Board defaults.
    - `capture.rs` - Capture defaults.
    - `colors.rs` - Color defaults.
    - `core.rs` - Core defaults.
    - `presets.rs` - Preset defaults.
    - `selection.rs` - Selection defaults.
    - `tools.rs` - Tool defaults.
    - `ui.rs` - UI defaults.
    - `zoom.rs` - Zoom defaults.
    - `mod.rs` - Defaults wiring.
- `src/config/tests/` - Config tests.
  - `file_io.rs` - IO tests.
  - `load.rs` - Load tests.
  - `schema.rs` - Schema tests.
  - `validate.rs` - Validation tests.
  - `mod.rs` - Test wiring.
- `src/config/types/` - Config type definitions.
  - `arrow.rs` - Arrow settings types.
  - `board.rs` - Board settings types.
  - `capture.rs` - Capture settings types.
  - `click_highlight.rs` - Click highlight settings types.
  - `context_menu.rs` - Context menu settings types.
  - `drawing.rs` - Drawing settings types.
  - `help_overlay.rs` - Help overlay settings types.
  - `history.rs` - History settings types.
  - `performance.rs` - Performance settings types.
  - `presets.rs` - Preset settings types.
  - `session.rs` - Session settings types.
  - `status_bar.rs` - Status bar settings types.
  - `tablet.rs` - Tablet settings types.
  - `ui.rs` - UI settings types.
  - `toolbar/` - Toolbar settings types.
    - `config.rs` - Toolbar config.
    - `mode.rs` - Toolbar mode types.
    - `overrides.rs` - Toolbar overrides.
    - `mod.rs` - Toolbar types wiring.
  - `mod.rs` - Type wiring.
- `src/config/validate/` - Config validation.
  - `arrow.rs` - Arrow validation.
  - `board.rs` - Board validation.
  - `drawing.rs` - Drawing validation.
  - `fonts.rs` - Font validation.
  - `history.rs` - History validation.
  - `keybindings.rs` - Keybinding validation.
  - `performance.rs` - Performance validation.
  - `presets.rs` - Preset validation.
  - `session.rs` - Session validation.
  - `tablet.rs` - Tablet validation.
  - `ui.rs` - UI validation.
  - `mod.rs` - Validation wiring.

#### `src/daemon/`
- `src/daemon/mod.rs` - Daemon module wiring.
- `src/daemon/core.rs` - Daemon core state and logic.
- `src/daemon/icons.rs` - Tray icon helpers.
- `src/daemon/tests.rs` - Daemon tests.
- `src/daemon/types.rs` - Daemon data types.
- `src/daemon/overlay/` - Overlay process control.
  - `mod.rs` - Overlay module wiring.
  - `process.rs` - Process management.
  - `spawn.rs` - Spawn helpers.
- `src/daemon/tray/` - System tray integration.
  - `helpers.rs` - Tray helper functions.
  - `ksni.rs` - KSNI trait implementation.
  - `runtime.rs` - Tray runtime state.
  - `mod.rs` - Tray wiring.

#### `src/draw/`
- `src/draw/mod.rs` - Drawing module wiring.
- `src/draw/color.rs` - Color utilities.
- `src/draw/dirty.rs` - Dirty tracking helpers.
- `src/draw/font.rs` - Font helpers.
- `src/draw/canvas_set/` - Canvas set management.
  - `pages.rs` - Multi-page canvas state.
  - `set.rs` - Canvas set logic.
  - `tests.rs` - Canvas set tests.
  - `mod.rs` - Canvas set wiring.
- `src/draw/frame/` - Frame and history logic.
  - `core.rs` - Frame core.
  - `frame_storage.rs` - Frame storage helpers.
  - `serde.rs` - Frame serialization.
  - `types.rs` - Frame data types.
  - `history/` - Undo/redo history.
    - `undo_action.rs` - Undo action types.
    - `frame/` - Frame history helpers.
      - `apply.rs` - Apply history.
      - `primary.rs` - Primary history ops.
      - `prune.rs` - History pruning.
      - `mod.rs` - History frame wiring.
    - `mod.rs` - History wiring.
  - `tests/` - Frame tests.
    - `serialization.rs` - Serialization tests.
    - `history/` - History tests.
      - `basics.rs` - History basics.
      - `limits.rs` - History limits.
      - `prune.rs` - Pruning tests.
      - `validate.rs` - History validation tests.
      - `mod.rs` - History test wiring.
    - `mod.rs` - Frame test wiring.
  - `mod.rs` - Frame wiring.
- `src/draw/render/` - Cairo/Pango rendering helpers.
  - `background.rs` - Background render.
  - `highlight.rs` - Highlight render.
  - `primitives.rs` - Render primitives.
  - `selection.rs` - Selection render.
  - `shapes.rs` - Shape renderers.
  - `strokes.rs` - Stroke render.
  - `text.rs` - Text render.
  - `types.rs` - Render types.
  - `mod.rs` - Render wiring.
- `src/draw/shape/` - Shape definitions.
  - `bounds.rs` - Shape bounds helpers.
  - `text.rs` - Text shape helpers.
  - `types.rs` - Shape types and enums.
  - `tests.rs` - Shape tests.
  - `mod.rs` - Shape wiring.

#### `src/input/`
- `src/input/mod.rs` - Input module wiring.
- `src/input/board_mode.rs` - Board mode enums and helpers.
- `src/input/events.rs` - Input event types (keys/buttons).
- `src/input/modifiers.rs` - Modifier state tracking.
- `src/input/tool.rs` - Tool enum and defaults.
- `src/input/hit_test/` - Hit testing helpers.
  - `geometry.rs` - Geometry math.
  - `shapes.rs` - Shape hit testing.
  - `tests.rs` - Hit test tests.
  - `mod.rs` - Hit test wiring.
- `src/input/tablet/` - Tablet input glue.
  - `mod.rs` - Tablet module wiring.
- `src/input/state/` - Input state machine.
  - `mod.rs` - State wiring.
  - `render.rs` - Input-driven render helpers.
  - `actions/` - Key action handling.
    - `action_board_pages.rs` - Page actions.
    - `action_capture_zoom.rs` - Capture/zoom actions.
    - `action_colors.rs` - Color actions.
    - `action_core.rs` - Core actions.
    - `action_dispatch.rs` - Action dispatch.
    - `action_history.rs` - History actions.
    - `action_presets.rs` - Preset actions.
    - `action_selection.rs` - Selection actions.
    - `action_tools.rs` - Tool actions.
    - `action_ui.rs` - UI actions.
    - `help_overlay.rs` - Help overlay actions.
    - `key_release.rs` - Key release handling.
    - `key_press/` - Key press handling.
      - `bindings.rs` - Binding resolution.
      - `panels.rs` - Panel key handling.
      - `text_input.rs` - Text input key handling.
      - `mod.rs` - Key press wiring.
    - `mod.rs` - Actions wiring.
  - `core/` - Core input state logic.
    - `base/` - Base state structs and helpers.
      - `state/` - Base state internals.
        - `init.rs` - State initialization.
        - `modifiers.rs` - Modifier synchronization.
        - `structs.rs` - State structs.
        - `mod.rs` - State wiring.
      - `types.rs` - Base types.
      - `mod.rs` - Base wiring.
    - `board.rs` - Board-level state.
    - `dirty.rs` - Dirty region tracking.
    - `highlight_controls.rs` - Highlight tool control.
    - `history.rs` - History control.
    - `index.rs` - State index helpers.
    - `selection.rs` - Selection state.
    - `utility/` - Utility helpers and dispatch.
      - `actions.rs` - Utility actions.
      - `font.rs` - Font helpers.
      - `frozen_zoom.rs` - Freeze/zoom helpers.
      - `help_overlay.rs` - Help overlay helpers.
      - `interaction.rs` - Interaction helpers.
      - `launcher.rs` - Launcher helpers.
      - `pending.rs` - Pending action helpers.
      - `toasts.rs` - Toast helpers.
      - `mod.rs` - Utility wiring.
    - `menus/` - Context menu handling.
      - `commands.rs` - Menu command handling.
      - `focus.rs` - Menu focus handling.
      - `hover.rs` - Menu hover handling.
      - `layout.rs` - Menu layout.
      - `lifecycle.rs` - Menu lifecycle.
      - `types.rs` - Menu types.
      - `entries/` - Menu entries per context.
        - `canvas.rs` - Canvas menu entries.
        - `pages.rs` - Pages menu entries.
        - `shape.rs` - Shape menu entries.
        - `mod.rs` - Entry wiring.
      - `mod.rs` - Menu wiring.
    - `properties/` - Selection properties panel.
      - `apply.rs` - Property apply logic.
      - `entries.rs` - Property entries.
      - `panel.rs` - Panel state.
      - `summary.rs` - Summary rendering helpers.
      - `types.rs` - Property types.
      - `utils.rs` - Panel utilities.
      - `panel_layout/` - Panel layout helpers.
        - `focus.rs` - Panel focus.
        - `interaction.rs` - Panel interaction.
        - `layout.rs` - Layout calc.
        - `mod.rs` - Layout wiring.
      - `apply_selection/` - Apply selection helpers.
        - `constants.rs` - Apply constants.
        - `helpers.rs` - Apply helpers.
        - `actions/` - Property-specific actions.
          - `arrow.rs` - Arrow property apply.
          - `color.rs` - Color property apply.
          - `fill.rs` - Fill apply.
          - `stroke.rs` - Stroke apply.
          - `text.rs` - Text apply.
          - `mod.rs` - Action wiring.
        - `mod.rs` - Apply selection wiring.
      - `mod.rs` - Properties wiring.
    - `selection_actions/` - Selection operations.
      - `clipboard.rs` - Copy/paste.
      - `delete.rs` - Delete helpers.
      - `delete/` - Delete helpers split by concern.
        - `tests.rs` - Delete tests.
      - `geometry.rs` - Selection geometry.
      - `reorder.rs` - Z-order changes.
      - `state.rs` - Selection state ops.
      - `translation/` - Move/resize translation.
        - `bounds.rs` - Translation bounds.
        - `transform.rs` - Transform helpers.
        - `undo.rs` - Undo helpers.
        - `mod.rs` - Translation wiring.
      - `text/` - Text selection editing.
        - `edit.rs` - Edit handling.
        - `handles.rs` - Handle geometry.
        - `wrap.rs` - Wrap logic.
        - `mod.rs` - Text wiring.
      - `mod.rs` - Selection actions wiring.
    - `tool_controls/` - Tool and preset controls.
      - `presets.rs` - Preset controls.
      - `settings.rs` - Settings controls.
      - `toolbar.rs` - Toolbar controls.
      - `mod.rs` - Tool controls wiring.
    - `mod.rs` - Core wiring.
  - `highlight/` - Highlight tool state.
    - `settings.rs` - Highlight settings.
    - `state/` - Highlight runtime state.
      - `mod.rs` - State wiring.
      - `tests.rs` - Highlight tests.
    - `mod.rs` - Highlight wiring.
  - `mouse/` - Mouse input handling.
    - `motion.rs` - Mouse motion.
    - `press.rs` - Press handling.
    - `release/` - Release handling.
      - `drawing.rs` - Drawing on release.
      - `panels.rs` - Panel release handling.
      - `selection.rs` - Selection release handling.
      - `text.rs` - Text release handling.
      - `mod.rs` - Release wiring.
    - `mod.rs` - Mouse wiring.
  - `tests/` - Input state tests.
    - `basics.rs` - Basics.
    - `drawing.rs` - Drawing tests.
    - `erase.rs` - Eraser tests.
    - `helpers.rs` - Test helpers.
    - `transform.rs` - Transform tests.
    - `menus/` - Menu tests.
      - `context_menu.rs` - Context menu tests.
      - `history.rs` - History menu tests.
      - `locks.rs` - Lock menu tests.
      - `mod.rs` - Menu test wiring.
    - `selection/` - Selection tests.
      - `actions.rs` - Selection action tests.
      - `deletion.rs` - Selection deletion tests.
      - `duplicate.rs` - Selection duplication tests.
      - `mod.rs` - Selection test wiring.
    - `text_edit/` - Text edit tests.
      - `commit_cancel.rs` - Commit/cancel tests.
      - `resize.rs` - Resize tests.
      - `triggers.rs` - Trigger tests.
      - `mod.rs` - Text edit test wiring.
    - `text_input/` - Text input tests.
      - `actions.rs` - Text input action tests.
      - `board.rs` - Board mode text input tests.
      - `escape.rs` - Escape handling tests.
      - `idle.rs` - Idle handling tests.
      - `text_mode.rs` - Text mode tests.
      - `mod.rs` - Text input test wiring.
    - `mod.rs` - Test wiring.

#### `src/paths/`
- `src/paths/mod.rs` - XDG path resolution helpers.
- `src/paths/tests.rs` - Path tests.

#### `src/session/`
- `src/session/mod.rs` - Session module wiring.
- `src/session/lock.rs` - Lockfile helpers.
- `src/session/options/` - Session options.
  - `config.rs` - Config to options mapping.
  - `identifiers.rs` - Display/output identity helpers.
  - `types.rs` - Options types.
  - `tests.rs` - Options tests.
  - `mod.rs` - Options wiring.
- `src/session/snapshot/` - Snapshot capture/load/save.
  - `apply.rs` - Apply snapshot to input state.
  - `capture.rs` - Capture snapshot from input state.
  - `compression.rs` - Compression helpers.
  - `history.rs` - History policies.
  - `load.rs` - Load snapshot.
  - `save.rs` - Save snapshot.
  - `types.rs` - Snapshot types.
  - `tests.rs` - Snapshot tests.
  - `mod.rs` - Snapshot wiring.
- `src/session/storage/` - Storage helpers.
  - `clear.rs` - Clear stored session data.
  - `inspect.rs` - Inspect stored session state.
  - `types.rs` - Storage types.
  - `tests.rs` - Storage tests.
  - `mod.rs` - Storage wiring.
- `src/session/tests/` - Session tests.
  - `helpers.rs` - Test helpers.
  - `history.rs` - History tests.
  - `limits.rs` - Size/limit tests.
  - `options.rs` - Options tests.
  - `roundtrip.rs` - Roundtrip tests.
  - `snapshot.rs` - Snapshot tests.
  - `mod.rs` - Test wiring.

#### `src/toolbar_icons/`
- `src/toolbar_icons/mod.rs` - Icon module wiring.
- `src/toolbar_icons/actions.rs` - Action icons.
- `src/toolbar_icons/controls.rs` - Control icons.
- `src/toolbar_icons/security.rs` - Security icons.
- `src/toolbar_icons/tools.rs` - Tool icons.
- `src/toolbar_icons/zoom.rs` - Zoom icons.
- `src/toolbar_icons/history/` - History icons.
  - `arrows.rs` - Arrow history icons.
  - `clock.rs` - Clock history icons.
  - `steps.rs` - Step history icons.
  - `mod.rs` - History icon wiring.

#### `src/ui/`
- `src/ui/context_menu.rs` - Context menu render.
- `src/ui/primitives.rs` - UI primitives.
- `src/ui/properties_panel.rs` - Properties panel render.
- `src/ui/toasts.rs` - Toast render.
- `src/ui/help_overlay/` - Help overlay UI.
  - `fonts.rs` - Help overlay fonts.
  - `grid.rs` - Help overlay grid layout.
  - `keycaps.rs` - Keycap rendering.
  - `layout.rs` - Layout helpers.
  - `search.rs` - Search/filter handling.
  - `sections.rs` - Section definitions.
  - `types.rs` - Overlay types.
  - `nav/` - Overlay navigation.
    - `render.rs` - Nav rendering.
    - `state.rs` - Nav state.
    - `mod.rs` - Nav wiring.
  - `render/` - Overlay render pipeline.
    - `frame.rs` - Frame render.
    - `metrics.rs` - Layout metrics.
    - `palette.rs` - Colors and palette.
    - `state.rs` - Render state.
    - `mod.rs` - Render wiring.
  - `mod.rs` - Help overlay wiring.
- `src/ui/status/` - Status bar UI.
  - `badges.rs` - Badge render.
  - `bar.rs` - Status bar render.
  - `mod.rs` - Status bar wiring.
- `src/ui/toolbar/` - Toolbar UI wiring (input side).
  - `bindings.rs` - Toolbar keybinding mapping.
  - `events.rs` - Toolbar events.
  - `snapshot.rs` - Toolbar snapshot helper.
  - `apply/` - Apply toolbar state to input.
    - `actions.rs` - Toolbar actions.
    - `delays.rs` - Delay adjustments.
    - `layout.rs` - Layout helpers.
    - `pages.rs` - Page controls.
    - `tools.rs` - Tool selection.
    - `mod.rs` - Apply wiring.
  - `mod.rs` - Toolbar wiring.

#### `src/util/`
- `src/util/mod.rs` - Utility module wiring.
- `src/util/arrow.rs` - Arrow math helpers.
- `src/util/colors.rs` - Color helpers.
- `src/util/geometry.rs` - Geometry helpers.
- `src/util/tests.rs` - Utility tests.
