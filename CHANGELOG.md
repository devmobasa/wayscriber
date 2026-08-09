# Changelog

## Unreleased

### Breaking (Rust source)

- The unified top toolbar is now the only layout. Panel-era typed config fields and order groups are removed from the serde model (`side_*` placement/pin/pane keys, `show_settings_section` / mode overrides, and `ui.toolbar.items.order.{side_sections,actions,pages,boards,presets,tool_options,sessions}`). Authored values at those exact paths remain in `config.toml` as retired settings and no longer affect the overlay. Matching keys under `runtime-ui.toml`'s recognized `item_order` map are pruned on rewrite.
- Public Rust types that described the side palette / panel-only toolbar order groups are gone. Downstream crates that constructed those fields must drop them; the in-repo configurator already matches this shape.

### Fixed

- Stylus pressure no longer overrides the selected Marker/Textmarker or Step Marker size. Pressure-to-thickness mapping remains limited to pressure-sensitive freehand Pen strokes.
