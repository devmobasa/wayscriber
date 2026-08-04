# AGENTS.md

## Scope
- Applies to GitHub workflows, issue templates, and repository automation under `.github/`.

## Architecture
- Workflows should mirror local CI and release/package validation.
- Release automation depends on version checks, packaging manifests, Nix/package checks, artifact upload, and repository publishing behavior.

## Invariants
- Keep CI aligned with `./tools/lint-and-test.sh`. Both call `./tools/run-cargo-consumer.py`, so the Cargo matrix is changed in `tools/cargo-lanes.json`, never in a workflow step.
- A new raw `cargo` command in a workflow must become a lane operation or an `allowed_non_lane_cargo` entry with a reason; `./tools/check-cargo-lanes.py` fails otherwise, and it also fails when an allowlisted step such as a gtk4-layer-shell linkage proof is deleted.
- Keep Linux system dependencies aligned with real build needs for Wayland, Cairo, Pango, GTK4/libadwaita, D-Bus, packaging, and Nix checks.
- Do not weaken release checks or skip package layout/version validation without a documented reason.
- Every AUR clone in the release workflow happens in one step, before any channel is mutated, and none of them may be made optional with `|| true`. The all-or-nothing boundary is what keeps a failed clone from half-publishing a release.
- The `Configurator modern (Arch)` job owns the canonical `.SRCINFO` validation for both `packaging/` and the configurator AUR templates. Retiring that job requires re-homing those checks first; they are unrelated to `adw-modern`.

## Coupled Changes
- Workflow dependency changes may require updates to `tools/`, `packaging/`, `flake.nix`, and setup docs.
- Release workflow changes may require updates to version scripts and packaging manifests.

## Validation
- Prefer local script validation before changing CI.
- For workflow-only edits, run `git diff --check` and inspect YAML carefully.
