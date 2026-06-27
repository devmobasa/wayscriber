# AGENTS.md

## Scope
- Applies to GitHub workflows, issue templates, and repository automation under `.github/`.

## Architecture
- Workflows should mirror local CI and release/package validation.
- Release automation depends on version checks, packaging manifests, Nix/package checks, artifact upload, and repository publishing behavior.

## Invariants
- Keep CI aligned with `./tools/lint-and-test.sh`.
- Keep Linux system dependencies aligned with real build needs for Wayland, Cairo, Pango, Iced, D-Bus, packaging, and Nix checks.
- Do not weaken release checks or skip package layout/version validation without a documented reason.

## Coupled Changes
- Workflow dependency changes may require updates to `tools/`, `packaging/`, `flake.nix`, and setup docs.
- Release workflow changes may require updates to version scripts and packaging manifests.

## Validation
- Prefer local script validation before changing CI.
- For workflow-only edits, run `git diff --check` and inspect YAML carefully.
