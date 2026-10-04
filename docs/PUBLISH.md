# Release publishing

The existing `scripts/set-version.ts` owns the version for Cargo, npm and the
MCP manifest. The crates workflow does not set a version or create another
release. Native binaries, npm, MCP and container publication remain in the
shared release job in `.github/workflows/release.yml`.

## Merge-group review gate

The required `ci` aggregate includes `review-stamp` on `merge_group`.
It calls the SHA-pinned shared action with trusted creator ID `8020099` and
repository scope in `.github/review-stamp.json`: workflows, actions, and that
scope file. repomap is a local tool with no hosted auth or billing paths.
Explicit trusted failure, error, or pending statuses block every queued PR,
including earlier entries carried by the group. Missing-stamp enforcement is
enabled for every PR. Security/money/migration classes (mandatory shared
globs/labels plus local scope) require an Ops success with description prefix
`PASS`; other changes require the owning lane's independent Opus final
reviewer. The product trusts creator `[8020099]` as data. Desk lanes share that
identity, so reviewer/builder independence is an owning-lane process
requirement, not provable by GitHub ID.

## Registry package verification

The same workflow verifies registry packages on pull requests, merge groups
and main. `python3 scripts/release_crates.py check` reads Cargo metadata,
orders publishable workspace crates by their dependencies, and copies tracked
sources into a temporary directory. Only those temporary manifests gain exact
registry version constraints on internal path dependencies. The tracked
manifests, lockfile and version setter are unchanged.

Cargo packages both crates together for `crates-io`, verifies builds from the
packages, and writes the `.crate` archives to the `registry-packages` artifact.
This covers the CLI's committed HTML, JavaScript and CSS assets as well as its
core dependency. Rust packaging/build verification runs in CI, not on the desk.
Pure Python guard tests can run locally with
`python3 scripts/test_release_crates.py`.

## One-time owner setup before publication

Publication is off unless repository variable `CRATES_IO_PUBLISH_ENABLED` is
exactly `true`. Do not enable it before the release owner authorizes publication
and confirms registry ownership and both trusted-publisher records. Enabling
it permits the next main push or manual release dispatch to publish the current
workspace version, including a version absent from crates.io even when npm has
already published it.

Required owner chores:

- Confirm availability and register the company packages `sylphx-repomap-core`
  and `sylphx-repomap`. The unprefixed packages belong to unrelated users and
  must not be published by this workflow. Both prefixed sparse-index paths
  returned HTTP 404 when checked; this does not reserve either name.
- Register a GitHub Actions trusted publisher for each package with GitHub
  owner `SylphxAI`, repository `repomap`, workflow filename `release.yml`, and
  environment `crates-io`. These are the required configuration values, not a
  claim that records already exist.
- Configure the GitHub environment `crates-io` consistently with those records,
  restrict deployment to main, and then enable the repository variable only
  after authorization. No long-lived registry secret is required.

The publish job requests a short-lived token using the official crates.io
OIDC action. Its post hook revokes the token. The script publishes
`sylphx-repomap-core` before `sylphx-repomap`, waits for each exact version to appear in the
sparse index, and skips an already-published exact version on a later dispatch.
A yanked exact version or registry error fails rather than being treated as
missing. If a publish is interrupted, dispatch the existing workflow again;
it resumes with the missing packages and never overwrites a published version.

The working source installation is
`cargo install --git https://github.com/SylphxAI/repomap sylphx-repomap`.
Only after successful registry publication is `cargo install sylphx-repomap`
usable. Until then, generated installation copy must retain the Git command.
The executable remains `repomap`, and the core Rust library remains
`repomap_core` through its explicit library target and dependency alias.
The existing npm and native installations are unchanged.
