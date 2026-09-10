# Releasing Wayscriber

## Prepare the release

Land tooling fixes, test routing, and release documentation as focused commits,
then commit the version bump separately immediately before tagging. An uncommitted
release preparation may contain these groups together; stage them separately at handoff.

1. Run `./tools/bump-version.sh X.Y.Z`. It updates both workspace versions and
   package metadata without refreshing locked dependencies. Review `Cargo.lock`:
   a version-only release should change only the two workspace package versions.
   Prefetch dependencies first if the local Cargo cache is empty.
2. Run `./tools/lint-and-test.sh` and `./tools/test-gtk-widgets.sh`. The canonical
   gate serializes the Rust test harness to avoid the observed parallel native-font
   crashes. It also runs each context-menu and board-picker retained-text rendering regression
   in a separate process under both feature configurations. This preserves coverage around the native
   Cairo/FreeType concurrency issue; it does not establish a native-library fix.
3. Exercise the affected desktop flows on Wayland, including capture, keyboard
   focus, save/reopen, and configurator edits. Automated checks do not prove
   installed-binary or compositor behavior.
4. Prepare the website's `docs-src/src/release-notes.md` and run its
   `build-docs.sh`. Keep the published `latest.json` on the previous release
   until the new downloads and package channels are available.
5. If the tarball file layout changes, verify and publish the matching Arch
   installer before tagging, as described in [the packaging guide](../tools/README.md#packaging).

## Publish and verify

After reviewing and committing the release changes, push the branch and wait for
its GitHub checks. Use `./tools/publish-release-tag.sh --version X.Y.Z` only when
ready to publish. It creates and pushes the tag; the tag starts the Release workflow.

Verify the whole Release workflow, including the GitHub assets, AUR recipes, and
apt/rpm repository deployment. AUR waits for successful GitHub asset publication.
Check that repository deployment actually ran: its configured-host step can be
skipped even when repository generation succeeds.

Then publish the prepared website notes through the website's deployment
workflow, excluding `/latest.json` from the upload so a stale checkout cannot
downgrade the live notice. Verify that exclusion before deploying. Finally, use
the private website's existing update-manifest publisher as
documented in that checkout: preview the proposed notice, publish it, and verify
`https://wayscriber.com/latest.json`. The notice must name the published version,
use its actual GitHub publication date, and link to the current release notes
and update instructions. Neither a pushed tag nor successful AUR publication
updates this notice automatically.

The website publisher updates its local manifest as well, but ordinary website
sync must still exclude the notice. Keep this final step after
all install channels and notes are ready. Four-part packaging hotfix versions
are unsupported by released update checkers; do not publish them as update notices.
