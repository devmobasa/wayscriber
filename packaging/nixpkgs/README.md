# nixpkgs recipe

`package.nix` is our copy of the Wayscriber recipe that lives in `nixpkgs` at
`pkgs/by-name/wa/wayscriber/package.nix`.

We do not own that file — `nixpkgs` does. This copy exists so that:

- packaging changes here (new system libraries, new installed files) are visible
  in the same commit as the change that requires them, and
- `tools/check-nixpkgs-recipe.py` can fail CI when a default Cargo feature needs
  a system library the `nixpkgs` build does not declare.

## How versions reach nixpkgs

Version bumps are opened automatically by the `nixpkgs-update` bot (R. Ryantm)
because the recipe carries `passthru.updateScript = nix-update-script { }`.
Every bump since `init at 0.7.2` has come from that bot, usually within a few
weeks of a release. We do not need to send routine version bumps ourselves.

`nixpkgs-unstable` follows releases. Stable NixOS branches (`nixos-26.05`, …)
keep whatever version existed when the branch was cut; version bumps are not
backported. Point stable-channel users at the project flake instead.

What the bot cannot do is change the build itself. When a release adds a system
dependency or a new installed file, someone has to send a real PR — otherwise
the next bot PR fails to build and stalls.

## Submitting a change

1. Fork and clone `NixOS/nixpkgs`, then copy this file over
   `pkgs/by-name/wa/wayscriber/package.nix`.
2. Fill in the hashes (`hash` and `cargoHash` are `lib.fakeHash` here):

   ```bash
   nix-shell -p nix-update --run "nix-update --version <version> wayscriber"
   ```

3. Build and smoke-test it:

   ```bash
   nix-build -A wayscriber
   ./result/bin/wayscriber --version
   ```

4. Commit with the `nixpkgs` convention, one logical change per commit:

   ```
   wayscriber: add GTK4 toolbar dependencies
   wayscriber: 0.9.21 -> 0.9.22
   ```

5. Open the PR against `master`. Do not target a stable release branch.

## Getting review notifications

`meta.maintainers` currently lists only `leiserfg`. Adding ourselves means the
bot's bump PRs request our review, which is the most effective way to keep the
package current. It is a two-part change in the same PR:

1. Add an entry to `maintainers/maintainer-list.nix` (name, GitHub handle,
   GitHub ID).
2. Add that handle to `meta.maintainers` here.

Adding the handle to `meta.maintainers` without the maintainer-list entry breaks
evaluation, so both parts must land together.

## Differences from the flake

`flake.nix` in the repository root builds from the working tree and also builds
`wayscriber-configurator`. This recipe builds a tagged release and ships the
main binary only — `nixpkgs` has no Configurator package today.
