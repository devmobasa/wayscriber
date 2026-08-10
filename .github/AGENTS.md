# AGENTS.md

## Scope
- Applies to GitHub workflows, issue templates, and repository automation under `.github/`.

## Architecture
- Workflows should mirror local CI and release/package validation.
- Release automation depends on version checks, packaging manifests, Nix/package checks, artifact upload, and repository publishing behavior.
- `ISSUE_TEMPLATE/` uses GitHub issue forms (YAML), not Markdown templates, so
  reporters get fields instead of a wall of text to edit.

## Invariants
- Keep CI aligned with `./tools/lint-and-test.sh`.
- Issue-form field `id`s are a public API: `https://wayscriber.com/report`
  prefills them by query parameter, and the About dialog in every shipped
  binary points at that page. Renaming or removing an id silently drops the
  prefill for binaries that can never be updated retroactively.
- Keep Linux system dependencies aligned with real build needs for Wayland, Cairo, Pango, GTK4/libadwaita, D-Bus, packaging, and Nix checks.
- Do not weaken release checks or skip package layout/version validation without a documented reason.

## Coupled Changes
- Workflow dependency changes may require updates to `tools/`, `packaging/`, `flake.nix`, and setup docs.
- Release workflow changes may require updates to version scripts and packaging manifests.
- Issue-form field ids are coupled with `wayscriber-website/report/index.html`
  and with `REPORT_URL` in `src/about_window/content.rs`.

## Validation
- Prefer local script validation before changing CI.
- For workflow-only edits, run `git diff --check` and inspect YAML carefully.
- After editing an issue form, confirm the prefill contract still holds:
  `issues/new?template=bug_report.yml&diagnostics=hello` must show `hello` in
  the Diagnostics box.
