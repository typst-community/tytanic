# [unreleased](https://github.com/tingerrr/tytanic/releases/tags/)
## Added
- cli: Add `--[no-]compare`
- cli: Add `--[no-]fail-fast`
- cli: Add `--[no-]skip` (renamed from `--no-implicit-skip`)
- cli: Add `--[no-]optimize-refs` (renamed from `--no-optimize-references`)
- cli: Add `--[no-]export-ephemeral`
- cli: Add `--warnings` enum
  - `ignore` to ignore all warnings
  - `emit` (default) to emit all warnings
  - `promote` to promote all warnings to errors (from `--promote-warnings`)
- cli: new: Add `--type`
  - `persistent` (default)
  - `ephemeral`
  - `compile-only`
- cli: new: Add `--persistent`

## Removed
- cli: Remove `--no-optimize-references` (renamed to `--[no-]optimize-refs`)
- cli: Remove `--no-implicit-skip` (renamed to `--[no-]skip`)
- cli: Remove `--promote-warnings` (renamed to `--warnings`)
- cli: run: Remove `--no-export` (renamed to `--[no-]export-ephemeral`)

## Changed
- cli: Deprecated `add` in favor of `new`
- cli: Deprecated `remove` in favor of `delete`

## Fixed
- filter: Don't ignore single test argument (#97)
- test: Update default test reference (#98)
- vcs: fix detection of git repository
- docs: fix some typos
- cli: add: Don't add references for non-pereistent tests (#96)

---

# [v0.1.1](https://github.com/tingerrr/tytanic/releases/tags/v0.1.1)
## Added
- `util vcs ignore` command to regenerate ignore files
- Show if template is detected in `tt status`

## Removed

## Changed
- `Vcs::ignore` and `Vcs::unignore` API
- `doc::compare` API, removed fail_fast parameter
- Changed location of ignore files to be in the test directory itself.

## Fixed
- Write correct header for mercurial ignore files
- Removed a dead asset path from the `flake.nix`.
- Panic when optimizing reference documents
- Panic when running `tt add` with faulty template.
- `--no-fail-fast` not being repsected for test failures
- `--no-fail-fast` causing partial confusing comparison diagostics

---

# [v0.1.0](https://github.com/tingerrr/tytanic/releases/tags/v0.1.0)
This changelog entry is a litte more detailed than the future one's will be because many of the changes were not documented or resolved through issues and PRs.

## Added
- CLI
  - a test set DSL for filtering tests
  - new option for `tt update` and `tt run`:
    - `--font-path` for adding font paths to search
    - `--ignore-system-fonts` for disabling system fonts
    - `--creation-timestamp` for disabling system fonts
  - new options for `tt run`:
    - `--max-deviations` and `--min-delta` for configuring comparison thresholds
  - new options for `tt add`:
    - `--compile-only` for creating tests which aren't compared
    - `--ephemeral` for creating tests which are comparewd to another script
  - new options for `tt list`, `tt status` and `tt fonts`:
    - `--json` to print the output to stdout as JSON
- Tests
  - in-source annotations for skipping tests
  - regression test templates using `tests/template.typ`
  - augmented standard library with special helpers for regression test
- Docs
  - Added an mdbook containing various guides and reference documents.

## Removed
- CLI
  - `config` as it was unnecessarily complicated and restrictive
  - `init` as it added a redundant step before running `add`
  - `uninit` because it can be achieved by an equivalent of `rm -rf tests`
  - `--format` because it was only useful for some commands

## Changed
- the directory structure no longer permits nested tests, `tt util migrate` can be used to migrate to the new directory structure

## Fixed
- panic on `tt add` with non-default test template
- removal of vcs ignore files when running some commands
- non-compression of reference images when running `tt update`
