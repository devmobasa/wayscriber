# AGENTS.md

## Scope
- Applies to reusable configurator field models under `configurator/src/models/fields/`.

## Architecture
- Field modules wrap editable values, validation state, and conversion for UI forms.
- Tool, toolbar, status, session, pressure, presenter, font, export, eraser, and toggle fields support model/view/update workflows.

## Invariants
- Invalid input should stay visible and report a form error; do not silently coerce or drop user input.
- Keep field defaults and validation aligned with core config validation and configurator views.

## Coupled Changes
- Field changes may affect config draft conversion, views, updates, search labels, docs, and tests.
- Tool field changes must stay aligned with `src/input/tool/`, config defaults, action metadata, and toolbar UI.

## Validation
- Add focused field tests under this subtree.
- Run configurator model tests for field behavior changes.
