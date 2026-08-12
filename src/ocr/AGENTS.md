# AGENTS.md

## Scope
- Applies to screen text recognition (`Copy text from screen`) under `src/ocr/`.
- Covers the capacity-one controller, the Tesseract/`wl-copy` adapters, and the typed outcomes the event loop consumes.
- Region selection, capture ownership, and toasts live in `src/backend/wayland/state/ocr.rs`, not here.

## Architecture
- `mod.rs` owns the request/outcome vocabulary and `run_request`, which encodes, recognizes, and publishes inside one worker stack frame.
- `controller.rs` is the identified capacity-one transport: one in-flight request, a terminal worker message, and an event-loop wake.
- `tesseract.rs` is the production adapter pair; tests substitute `TextRecognizer`/`OcrTextPublisher` fakes.

## Invariants
- Recognized text must never reach application state, a log line, or a `Debug` rendering. `RecognizedText` redacts its own `Debug`; keep it that way.
- Never invoke a shell. Tesseract takes an explicit argument vector through the process broker's `HelperKind::Tesseract` allowlist.
- Language values arrive already validated by `config::validate_ocr_languages`; this module does not decide what is acceptable.
- The temporary PNG is deleted on every path, including failures and panics.
- Capacity one is deliberate: report busy rather than queueing a screen region the user has moved on from.

## Coupled Changes
- Failure categories couple to the toasts in `src/backend/wayland/state/ocr.rs`.
- The invocation policy couples to `src/process_broker/manifest.rs` and its tests.
- `capture.ocr_languages` couples to `src/config/types/capture.rs`, the configurator Capture page, `config.example.toml`, and `docs/CONFIG.md`.
- Packaging guidance for Tesseract lives in `packaging/` and `README.md`.

## Validation
- `cargo test ocr` covers the controller, adapters, and privacy contract.
- Run `./tools/lint-and-test.sh` for changes that touch the broker policy or config surface.
- Engine-dependent behavior (missing language data, real recognition quality) needs the live Wayland check; the unit tests use fakes.
