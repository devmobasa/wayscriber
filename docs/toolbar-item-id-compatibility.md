# Toolbar Item ID Compatibility Inventory

This inventory records the consumer-based decision for every historical
`side.*` toolbar item ID that existed when the legacy side palette was removed.
The serialized spelling is a configuration contract: an ID remains recognized
only when the unified top toolbar still consumes it.

The code sources of truth are
[`ids.rs`](../src/config/types/toolbar/ids.rs) and
[`items/definitions.rs`](../src/config/types/toolbar/items/definitions.rs).
Do not add, remove, or rename a historical ID without updating this inventory
and the owning model test.

Proof references used below:

- **Definitions** — `toolbar_item_definitions_are_unique_parseable_and_labeled`
  proves every retained ID is defined and parsed.
- **Sections** — `config::types::toolbar::visibility::tests` proves explicit
  section visibility overrides beat layout-mode baselines.
- **Models** — the action, session, and settings model tests exhaustively map
  commands to retained IDs; visibility tests also prove that retained IDs hide
  their corresponding unified-top controls.
- **Document** — `retired_toolbar_settings_are_preserved_and_diagnosed` proves
  representative retained overrides survive an unrelated document save.

## Retained IDs

| Serialized ID | Unified-top consumer | Proof |
| --- | --- | --- |
| `side.group.step-undo` | Canvas popover Step Undo/Redo section | Definitions, Sections |
| `side.group.actions` | Canvas popover basic Actions section | Definitions, Sections, Models |
| `side.group.pages` | Canvas popover Pages section | Definitions, Sections, Models |
| `side.group.boards` | Canvas popover Boards section | Definitions, Sections, Models |
| `side.group.presets` | Top-strip presets island | Definitions, Sections, Document |
| `side.group.actions-advanced` | Canvas popover Advanced section | Definitions, Sections, Models |
| `side.group.zoom-actions` | Canvas popover Zoom section | Definitions, Sections, Models |
| `side.group.text-controls` | Contextual style-pill text controls | Definitions, Sections |
| `side.actions.zoom-in` | Canvas Zoom command | Definitions, Models, Document |
| `side.actions.zoom-out` | Canvas Zoom command | Definitions, Models |
| `side.actions.reset-zoom` | Canvas Zoom command | Definitions, Models |
| `side.actions.toggle-zoom-lock` | Canvas Zoom command | Definitions, Models |
| `side.actions.undo-all` | Canvas Advanced command | Definitions, Models |
| `side.actions.redo-all` | Canvas Advanced command | Definitions, Models |
| `side.actions.undo-all-delayed` | Canvas Advanced command | Definitions, Models |
| `side.actions.redo-all-delayed` | Canvas Advanced command | Definitions, Models |
| `side.actions.freeze` | Canvas Advanced command | Definitions, Models, Document |
| `side.pages.previous` | Canvas Pages command | Definitions, Models |
| `side.pages.next` | Canvas Pages command | Definitions, Models |
| `side.pages.new` | Canvas Pages command | Definitions, Models |
| `side.pages.duplicate` | Canvas Pages command | Definitions, Models |
| `side.pages.delete` | Canvas Pages command | Definitions, Models, Document |
| `side.boards.picker` | Canvas Boards picker command | Definitions, Models, Document |
| `side.boards.previous` | Canvas Boards command | Definitions, Models |
| `side.boards.next` | Canvas Boards command | Definitions, Models |
| `side.boards.new` | Canvas Boards command | Definitions, Models |
| `side.boards.duplicate` | Canvas Boards command | Definitions, Models |
| `side.boards.delete` | Canvas Boards command | Definitions, Models, Document |
| `side.settings.context-aware-ui` | Settings popover toggle | Definitions, Models |
| `side.settings.text-controls` | Settings popover toggle | Definitions, Models |
| `side.settings.status-bar` | Settings popover toggle | Definitions, Models |
| `side.settings.status-board-badge` | Settings popover toggle | Definitions, Models |
| `side.settings.status-page-badge` | Settings popover toggle | Definitions, Models |
| `side.settings.floating-badge-always` | Settings popover toggle | Definitions, Models |
| `side.settings.preset-toasts` | Settings popover toggle | Definitions, Models |
| `side.settings.presets` | Settings popover toggle | Definitions, Models |
| `side.settings.actions` | Settings popover toggle | Definitions, Models |
| `side.settings.zoom-actions` | Settings popover toggle | Definitions, Models |
| `side.settings.advanced-actions` | Settings popover toggle | Definitions, Models |
| `side.settings.boards` | Settings popover toggle | Definitions, Models |
| `side.settings.pages` | Settings popover toggle | Definitions, Models |
| `side.settings.step-controls` | Settings popover toggle | Definitions, Models |
| `side.settings.command-palette` | Settings popover command | Definitions, Models |
| `side.settings.configurator` | Settings popover command | Definitions, Models |
| `side.settings.config-file` | Settings popover command | Definitions, Models |
| `side.settings.about` | Settings popover command | Definitions, Models, Document |
| `side.session.open` | Session popover command | Definitions, Models |
| `side.session.save-as` | Session popover command | Definitions, Models |
| `side.session.info` | Session popover command | Definitions, Models, Document |
| `side.session.clear` | Session popover command | Definitions, Models |
| `side.session.manager` | Session popover configurator command | Definitions, Models |

## Removed IDs

Removed IDs are deliberately unknown to the active parser. Authored strings in
`items.hidden` and `items.shown` still round-trip as unknown raw values; they do
not control runtime UI.

| Serialized ID | Consumer trace and disposition |
| --- | --- |
| `side.group.colors` | Panel Draw card only; the style pill uses `top.group.quick-colors`. |
| `side.group.thickness` | Panel Draw card only; thickness is contextual style-pill state. |
| `side.group.eraser-mode` | Panel Draw card only; eraser mode is contextual style-pill state. |
| `side.group.polygon-sides` | Panel Draw card only; polygon sides are contextual style-pill state. |
| `side.group.arrow-labels` | Panel Draw card only; arrow labels are contextual style-pill state. |
| `side.group.step-markers` | Panel Draw card only; step-marker reset is contextual style-pill state. |
| `side.group.marker-opacity` | Panel Draw card only; opacity is contextual style-pill state. |
| `side.group.text-size` | Panel Draw card only; text size is contextual style-pill state. |
| `side.group.font` | Panel Draw card only; font controls are contextual style-pill state. |
| `side.group.settings` | Panel container only; Settings is an unconditional top popover host. |
| `side.group.session` | Panel container only; Session is an unconditional top popover host. |
| `side.actions.undo` | Panel button only; the top control uses `top.utility.undo`. |
| `side.actions.redo` | Panel button only; the top control uses `top.utility.redo`. |
| `side.actions.clear-canvas` | Panel button only; the top control uses `top.utility.clear-canvas`. |
| `side.boards.rename` | No unified-top button or toolbar event consumer. |
| `side.tool-options.color` | Panel Draw control only; the style pill uses its typed tool model. |
| `side.tool-options.thickness` | Panel Draw control only; the style pill uses its typed tool model. |
| `side.tool-options.marker-opacity` | Panel Draw control only; the style pill uses its typed tool model. |
| `side.tool-options.eraser-mode` | Panel Draw control only; the style pill uses its typed tool model. |
| `side.tool-options.font-size` | Panel Draw control only; the style pill uses its typed tool model. |
| `side.tool-options.font-family` | Panel Draw control only; the style pill uses its typed tool model. |
| `side.tool-options.polygon-sides` | Panel Draw control only; the style pill uses its typed tool model. |
| `side.tool-options.arrow-labels` | Panel Draw control only; the style pill uses its typed tool model. |
| `side.tool-options.step-marker-reset` | Panel Draw control only; the style pill uses its typed tool model. |

## Retired order fields

The unified toolbar has only the `top_tools` and `top_controls` order groups.
The following panel-era fields are preserved as authored raw TOML, reported by
their exact path as `RetiredSetting`, and omitted from the typed schema and
configurator. They have no aliases.

| Source TOML path | Former consumer | Disposition and proof |
| --- | --- | --- |
| `ui.toolbar.items.order.side_sections` | Side-palette section renderer and its reorder UI | Retired with the panel; covered before and after an unrelated save by `retired_toolbar_settings_are_preserved_and_diagnosed`. |
| `ui.toolbar.items.order.actions` | Typed order resolver and runtime preference map; no unified-top renderer or reorder UI consumed it | Retired raw; covered by the same preservation/diagnostic fixture. |
| `ui.toolbar.items.order.pages` | Typed order resolver and runtime preference map; no unified-top renderer or reorder UI consumed it | Retired raw; covered by the same preservation/diagnostic fixture. |
| `ui.toolbar.items.order.boards` | Typed order resolver and runtime preference map; no unified-top renderer or reorder UI consumed it | Retired raw; covered by the same preservation/diagnostic fixture. |
| `ui.toolbar.items.order.presets` | Typed order resolver and runtime preference map; no unified-top renderer or reorder UI consumed it | Retired raw; `side.group.presets` remains a separate active visibility ID. Covered by the same preservation/diagnostic fixture. |
| `ui.toolbar.items.order.tool_options` | Typed order resolver and runtime preference map; no unified-top renderer or reorder UI consumed it | Retired raw with the unused `side.tool-options.*` namespace; covered by the same preservation/diagnostic fixture. |
| `ui.toolbar.items.order.sessions` | Typed order resolver and runtime preference map; no unified-top renderer or reorder UI consumed it | Retired raw; covered by the same preservation/diagnostic fixture. |

The exact-path classifier lives in
[`document.rs`](../src/config/document.rs). The document fixture also proves
that active `top_tools` and `top_controls` orders retain their runtime effect;
the shared-model test proves both orders reach their unified-top controls.
