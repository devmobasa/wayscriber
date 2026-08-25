# AGENTS.md

## Scope
- Applies to screen text recognition under `src/ocr/`: both `Copy text from screen` and the text-row geometry the marker's snap mode uses.
- Covers the two capacity-one controllers, the Tesseract/`wl-copy` adapters, and the typed outcomes the event loop consumes.
- Region selection, capture ownership, and toasts live in `src/backend/wayland/state/ocr.rs` and `src/backend/wayland/state/marker_snap.rs`, not here.

## Architecture
- `mod.rs` owns the request/outcome vocabulary and `run_request`, which encodes, recognizes, and publishes inside one worker stack frame.
- `controller.rs` is the identified capacity-one transport: one in-flight request, a terminal worker message, and an event-loop wake.
- `tesseract.rs` is the production adapter pair; tests substitute `TextRecognizer`/`OcrTextPublisher` fakes. Its temp-file, PATH, and error-classification helpers are shared with the layout path.
- `text_layout.rs` is the second controller: it scans the whole displayed screen image for text-row geometry and never touches the clipboard. Separate from `controller.rs` on purpose, so an unprompted snap scan can never make an explicit `Copy text from screen` wait.
- `text_lines.rs` parses Tesseract TSV into per-line boxes. The box comes from the level-4 line row, confirmed by at least one level-5 word above a confidence floor.

## Invariants
- Recognized text must never reach application state, a log line, or a `Debug` rendering. `RecognizedText` redacts its own `Debug`; keep it that way.
- The layout path is stricter: TSV stdout carries the recognized words, and only geometry may leave `text_lines.rs`. The `text` column is read to test for blankness and never stored or returned.
- The confidence floor and the line-row-over-word-union choice in `text_lines.rs` were measured against Tesseract 5.5.3, not guessed; the reasons are in its module docs and the tests carry the captured values. Re-measure before changing either.
- Never invoke a shell. Tesseract takes an explicit argument vector through the process broker's `HelperKind::Tesseract` allowlist.
- Language values arrive already validated by `config::validate_ocr_languages`; this module does not decide what is acceptable.
- The temporary PNG is deleted on every path, including failures and panics.
- Capacity one is deliberate: report busy rather than queueing a screen region the user has moved on from.

## Coupled Changes
- Failure categories couple to the toasts in `src/backend/wayland/state/ocr.rs`.
- The invocation policy couples to `src/process_broker/manifest.rs` and its tests.
- `capture.ocr_languages` couples to `src/config/types/capture.rs`, the configurator Capture page, `config.example.toml`, and `docs/CONFIG.md`. Both controllers use it.
- Text-row geometry couples to `src/input/text_snap.rs` (the pure snapping rules), `src/input/state/core/marker_snap.rs` (mode and lock), and `src/backend/wayland/state/marker_snap.rs` (scan lifecycle and screen-source validity).
- Packaging guidance for Tesseract lives in `packaging/` and `README.md`.

## Validation
- `cargo test ocr` covers both controllers, the adapters, the TSV parser, and the privacy contract.
- `cargo test marker_snap` and `cargo test text_snap` cover the snapping rules and the scan lifecycle.
- Run `./tools/lint-and-test.sh` for changes that touch the broker policy or config surface.
- Engine-dependent behavior (missing language data, real recognition quality) needs the live Wayland check; the unit tests use fakes.
