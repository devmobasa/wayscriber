# Changelog

## Unreleased

### Added

- About dialog: a "Report a problem" row. It copies your diagnostics to the clipboard and opens `https://wayscriber.com/report`, which hands them straight into a prefilled bug form. Nothing is sent automatically, and the diagnostics travel in the URL fragment, so they never reach a server log.
- Automatic guidance toasts now provide explicit "Got it" and "Tip settings…" controls. The General UI setting can disable automatic tips without disabling manual tours or real warnings.

### Breaking (Rust source)

- The unified top toolbar is now the only layout. Panel-era typed config fields and order groups are removed from the serde model (`side_*` placement/pin/pane keys, `show_settings_section` / mode overrides, and `ui.toolbar.items.order.{side_sections,actions,pages,boards,presets,tool_options,sessions}`). Authored values at those exact paths remain in `config.toml` as retired settings and no longer affect the overlay. Matching keys under `runtime-ui.toml`'s recognized `item_order` map are pruned on rewrite.
- Public Rust types that described the side palette / panel-only toolbar order groups are gone. Downstream crates that constructed those fields must drop them; the in-repo configurator already matches this shape.

### Fixed

- Stylus pressure no longer overrides the selected Marker/Textmarker or Step Marker size. Pressure-to-thickness mapping remains limited to pressure-sensitive freehand Pen strokes.
- Automatic tips and skipped-default shortcut notices now remember acknowledgement and taught-feature use instead of returning every active launch. Persistence failures are surfaced instead of causing repeat loops.
- First-run onboarding no longer requires the radial-menu flick-to-commit exercise. Saved sessions paused on that retired step continue at the reference step.
