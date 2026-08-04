# Cargo lane fixtures

Inputs for `./tools/check-cargo-lanes.py --self-test`. They let the guard prove
each of its rules rejects what it claims to reject without editing the working
tree or running Cargo.

- `feature-cases.json` — one healthy case plus five negative fixtures:
  `default` gaining `adw-modern`, `adw-modern` moving to
  `libadwaita/v1_8`, the direct libadwaita edge moving to `v1_5`, a declared
  feature no lane closure enables, and a transitive dependency raising the
  baseline lane's resolved floor to `v1_5`.
- `entry-point-cases.json` — cases for both kinds of entry point. For the driver
  kind: one contract-complete workflow plus texts that each break one rule (a
  deleted linkage step, a reintroduced raw command, a duplicated driver call, a
  missing Arch gate, a driver call for a consumer the manifest does not declare,
  and raw Cargo commands in four spellings — plain, environment-prefixed, behind
  a wrapper with its own operands, and path-qualified). For the loader kind: one
  checker text that imports `tools/cargo_lanes.py` and names its consumer, one
  with both gone, and one where the consumer name survives in a label after the
  import was deleted.
- `manifest-cases.json` — whole manifests replayed through
  `tools/cargo_lanes.py`: the smallest one the loader accepts, plus two whose
  operation argv appends its own package or feature selection after the lane
  arguments (once as `--features adw-modern`, once as `--package=` past a bare
  `--`). Those two compile something the lane they are labeled with never
  describes, which every guard that reads `lane.args` would keep missing.
- `metadata/*.json` — `cargo metadata` documents trimmed to the fields the guard
  reads. `healthy-packages.json` is the `--no-deps` shape; `*-lane-*.json` are
  resolved graphs for one lane each.
- `manifests/*.json` — whole lane manifests for the schema rules, which reject a
  document rather than a metadata graph or an entry-point text.
- `entry-points/*.yml` — trimmed workflow texts checked against the live
  `.github/workflows/ci.yml` contract in `tools/cargo-lanes.json`.
- `entry-points/*.py` — trimmed checker texts checked against the live
  `tools/check-rust-source-coverage.py` contract, the entry point that consumes
  its lane vectors through the loader rather than the driver because it needs
  the streamed Cargo JSON messages. They are fixtures, never imported or run.

Every case declares `expect` (`pass` or `fail`) and, when failing,
`expect_error_contains`. A negative case that starts passing, or that fails with
an unrelated message, fails the self-test: that is how the fixtures stay honest
when a rule's wording changes.

Lane coverage is not optional. A feature case must supply resolved metadata for
every lane that compiles the configurator, so adding a lane to
`tools/cargo-lanes.json` without extending these fixtures fails the self-test.
