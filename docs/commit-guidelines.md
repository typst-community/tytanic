# Commit Guidelines
This document outlines the commit guidelines for Tytanic.
These guidelines are mostly there to make reviewing and bug hunting in the future easier
While these aren't hard requirements, it would be greatly appreciated if you follow them.

## Atomic Commits
Try to keep commits atomic and self contained.

Each commit should ideally do one thing and pass CI on its own.
For example, if you need to refactor a function in order to add a new feature cleanly, put the refactoring in one commit and the new feature in a different commit.
Include tests and documentation in the same commit as the code they test and document, these are not distinct pieces of work.

Each commit should pass the CI (see the `ci` recipe in the [Justfile] for running a subset of CI locally).

Squash any fixup commits before marking your PR as ready for review.

## Linear History
Keep your history linear.

Rebase on main to update your branch instead of merging main into it.
You can avoid annoying conflicts by doing this frequently keeping each conflict resolution small and self contained.

> [!note]
> Take a look at [mergiraf] to get automated syntax-tree-based merge resolution, this won't fix all conflicts, but can help you get started.
> On a related note, [difftastic] is great for viewing syntax-tree-aware diffs.

## Commit Descriptions
Write comprehensive descriptions and short summaries.

Start your commits with a topic prefix followed by a short summary.
For non trivial commits add a blank line followed by a description, try to keep the summary below 50 characters and wrap the description at 72 characters.
Describe the "what" and "why", not the "how" of a patch in the description.
Here's a good write-up of [how to write good commit messages][git-commit].

### Topic Prefixes
Use a prefix that matches the area of the codebase you're changing:

| Prefix     | Area                                            |
| ---------- |------------------------------------------------ |
| `cli:`     | CLI argument parsing, commands, terminal output |
| `core:`    | Core library (`tytanic-core`)                   |
| `filter:`  | Test set expression DSL (`tytanic-filter`)      |
| `runner:`  | Test execution logic                            |
| `ui:`      | Terminal UI formatting                          |
| `docs:`    | Documentation (book, markdown files)            |
| `tests:`   | Test infrastructure and integration tests       |
| `deps:`    | Dependency updates                              |
| `nix:`     | Nix flake and packaging                         |
| `docker:`  | Dockerfile and packaging                        |
| `github:`  | GitHub Actions, issue templates                 |
| `release:` | Release commits                                 |

This is not an exhaustive list, if something doesn't fit, the reviewer will tell you.
For changes spanning multiple areas, use the most relevant prefix or ask a contributor or maintainer.

### Link to Issues
Link to issues that are related to a commit.

When a commit fixes a bug, its description should link to that issue.
GitHub picks these up automatically when you place `Closes #42` or `Resolves #42, #45 and #46.` in your commit descriptions.

## Tools
If you have trouble squashing, amending or reordering commits try out [jj].
It makes history rewriting very easy and is useful for git beginners and power-users alike.
If you prefer a GUI or TUI, there's also [gg] and [lazyjj] among others.

[Justfile]: ../Justfile
[difftastic]: https://difftastic.wilfred.me.uk/
[gg]: https://github.com/gulbanana/gg
[git-commit]: https://cbea.ms/git-commit/
[jj]: https://github.com/martinvonz/jj
[lazyjj]: https://github.com/Cretezy/lazyjj
[mergiraf]: https://mergiraf.org/
