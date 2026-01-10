# Release Process
This document describes the release process of Tytanic, it is most relevant to core maintainers.

New minor versions are introduced by a feature cut, after which a new branch is created.
The new branch only receives bugfixes from main since the feature cut (backported if necessary).

All crates are released in tandem, even if a crate had no meaningful changes for a release.

Creating a full release requires push access to the crates on `crates.io` and maintainer access to the repository.

## Branches and Tags
Once a feature cut for a minor version is announced the new minor branch (e.g. `v0.4`) is created.
While this release is active the branch may not point to a release, but unreleased bugfixes or a release candidate.
This means all prior to the last active minor release should have minor branches pointing to the latest [SemVer]-compatible patch release.

An example of this is listed below, on the left are the tags and on the right are the branches.
| patch tag     | minor branch | comment                                |
| ------------- | ------------ | -------------------------------------- |
| `v0.1.0`      |              |                                        |
| `v0.1.1`      |              |                                        |
| `v0.1.2-rc.1` |              |                                        |
| `v0.1.2`      | `v0.1`       | Inactive, no further updates expected. |
| `v0.2.0-rc.1` | `v0.2`       | Active, not technically `v0.2`.        |

No major versions are currently provided as Tytanic is pre-1.0.

The `latest` branch should be moved to the latest non-release-candidate patch release, in the example above it points to `v0.1.0`.

> [!note]
> Prior to version `v0.4` these branches were part of `main`, which meant that sometimes patch versions also received new features.

[Jujutsu] is recommended to keep the process of moving branches, or in jj-jargon "bookmarks", simple.

## Release Checklist
If this release creates a new minor version, create the branch pointing to the feature cut and push it.
Any related release commit should target this branch in its PR.

### Prepare the Release Commit
Create a single commit with the topic `release:` that updates all version references.

**Files to update:**
- [ ] `Cargo.toml` - Update the `version` field and all internal workspace dependencies (i.e. `tytanic-*`).
- [ ] `Cargo.lock` - Run `cargo check` to update the lockfile.
- [ ] `docs/CHANGELOG.md` - Move current items from `[unreleased]` to a new version section.
- [ ] `docs/book/src/quickstart/install.md` - Update the version in install commands.
- [ ] `docs/book/src/guides/ci.md` - Update the version in CI workflow examples.
- [ ] `docs/book/src/reference/compat.md` - Add the new version to the compatibility table.
- [ ] `assets/workflows/ci.yml` - Update the version in the example workflow.

### Create the PR
Create a Pull Request targeting the minor branch with the new commit and ensure CI passes.
Once CI passes and reviews are complete, merge the PR into the minor branch.

### Create the GitHub release
1. Go to [GitHub Releases](https://github.com/typst-community/tytanic/releases).
1. Click "Draft a new release".
1. Enter the new version as the tag and select "Create new tag".
1. Select the minor branch as the target.
1. Set the release title to the tag name.
1. Copy the changelog section for this version into the release notes and add release highlights.
1. Publish the release. The release pipeline will automatically create binaries and Docker images.

### Update Git References
If necessary, update the `latest` branch to point to the new release, this should only be done for non-release-candidates.

### Publish to crates.io
Publish the crates on crates.io in order.
```shell
cargo publish -p tytanic-utils
cargo publish -p tytanic-filter
cargo publish -p tytanic-core
cargo publish -p tytanic
```

> [!note]
> With Rust 1.90 this can be done in one step using the `--workflow` flag.

> [!note]
> With crates.io trusted builds this will also be ported to the release workflow soon.

## Yanking a Release
If a release has critical bugs (including that of transitive dependencies) a patch version must be released and the broken release must be yanked:

1. Create a patch release with the fixes.
1. Yank the broken release from crates.io. As with releasing, this should be done for all crates.
1. Add a warning to the changelog and release entry:
   ```md
   > [!important]
   > This release was yanked, see vX.Y.Z.
   ```

[SemVer]: https://semver.org/
[Jujutsu]: https://github.com/jj-vcs/jj/
