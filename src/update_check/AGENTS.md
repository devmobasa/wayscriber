# AGENTS.md

## Scope
- Applies to the update *notice* under `src/update_check/`: version comparison, the
  release manifest, transport, and the on-disk cache.

## Architecture
- `version.rs` owns semver-ish parsing/ordering, so only a genuinely newer release counts.
- `manifest.rs` owns the published document at `https://wayscriber.com/latest.json`, the
  URL trust rules, and the install-source docs anchor.
- `fetch.rs` asks the pre-lock process broker to run the system `curl`/`wget`; there is
  deliberately no HTTP or TLS crate in the dependency tree.
- `cache.rs` owns `$XDG_CACHE_HOME/wayscriber/update-check.json` (last result, throttle
  timestamp, notification dedupe).
- `mod.rs` is the only surface other modules use: `cached_status`, `check_now`,
  watcher-owned `CheckThrottle`, `notification_pending`/`claim_notification`,
  `background_checks_enabled`.

## Invariants
- Wayscriber never installs, downloads, or executes an update. This module reports and
  links; that is all.
- The request carries no Wayscriber or user identifier, Wayscriber version string, query
  parameters, or cookies; suppress the HTTP client's version and credential files too. That
  requires suppressing user configuration — curl `--disable` (**must be argument one**),
  wget `--no-config` — since a `.curlrc`/`.wgetrc` can otherwise add headers, cookie jars,
  extra URLs, output files, or the verbose stderr the pipe handling assumes stays small.
- Two timestamps plus an explicit outcome: `last_attempt_unix` throttles,
  `last_success_unix` answers "checked N ago", and `last_attempt_outcome` determines
  `Freshness::last_attempt_failed`. Do not infer the outcome by ordering second-resolution
  timestamps: a same-second retry or a clock adjustment makes that ambiguous.
- Every attempt persists its timestamp, including a failed explicit one (`--check-update`,
  About's "Check now"): the request was already made, and the daemon reads the same file to
  decide whether another is due.
- The daemon watcher owns a `CheckThrottle` with its process-local attempt time, because an
  unwritable cache would otherwise make every wakeup look like the first check. Keep this
  state owned by the watcher; do not turn it back into ambient global synchronization.
- All read-modify-write cycles go through `cache::update`, which holds an advisory lock:
  the daemon, About, and `--check-update` are separate processes on one file, and an
  unsynchronized cycle silently erases `notified_version` or restores a superseded result.
  Plain reads need no lock — writes are whole-file and atomic. If the lock cannot be
  acquired within the bounded wait, skip the mutation; never proceed with an unlocked
  read-modify-write.
- Manifest URLs are accepted only as HTTPS on an exact trusted host, validated both at
  parse time **and** on cache read — the cache file is user-writable and these strings
  reach `xdg-open`.
- Responses are size-capped **while streaming**, not after buffering: `wget` has no
  `--max-filesize`, so the process broker's bounded complete-output mode abandons the transfer
  past the cap. Do not bypass the broker or replace that path with `Command::output()`.
- One check is one request: a client that is installed but fails ends the check. Falling
  through to the next client is only for a missing executable.
- The background watcher fails closed — an unreadable or unparseable config disables the
  check rather than falling back to the enabled default, since the setting it cannot read
  may be the one that switched it off.
- Three independent off switches, strongest first: `WAYSCRIBER_NO_UPDATE_CHECK` at build
  time (`compiled_out()`), `WAYSCRIBER_DISABLE_UPDATE_CHECK` at runtime, `[updates] check`
  in config. An explicit `--check-update` overrides the latter two but not the first.
- A failed check still records the attempt, so an offline machine retries on the next
  interval rather than on every wakeup.
- Notifications are claimed only after successful delivery, and at most once per version.
- Nothing here blocks a protocol handler or the overlay: the daemon's watcher thread and
  the About window's event loop own the blocking calls.

## Coupled Changes
- Manifest/URL changes must stay aligned with `website/latest.json`, `website/release.sh`,
  and the heading anchors in `website/docs-src/src/getting-started/updating.md`
  (released binaries deep-link into those anchors and cannot be fixed retroactively).
- Surfaces: `src/about_window/` (update card), `src/daemon/update_watch.rs` (background
  check, tray notice, desktop notification), `src/app/mod.rs` (`--check-update`).
- Config keys live in `src/config/types/updates.rs` and are clamped in
  `src/config/validate/updates.rs`; document them in `config.example.toml`,
  `docs/CONFIG.md`, and the website config reference.

## Validation
- `cargo test --lib update_check` covers parsing, ordering, trust rules, and throttling.
- Also build with `WAYSCRIBER_NO_UPDATE_CHECK=1` when touching the opt-out paths; the
  test suite asserts different behavior in that configuration.
- Never add a test that performs a real network request.
