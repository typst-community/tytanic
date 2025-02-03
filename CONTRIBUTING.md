# Contributing
Thank you for considering to contribute to `tytanic`.
Any contributions are welcome, from implementing large features to fixing small typos.
If there's an issue you're working on let others know by leaving a comment, don't be afraid to ask for help if you're new to contributing.

**If you're contributing for the first time to `tytanic`, please familiarize yourself with the workflow below.**

## Opening a PR
When you open a PR, it can be as messy as you want, but once you request review make sure you've brought it into a state at which you would merge it yourself.
Please link to the issues you're fixing in the PR description using `Closes #42` or `Resolves #42, #45 and #46.`, GitHub picks this up automatically.
Try to follow the guidelines outlined below.

### Linear History
Keep your history linear, rebase on main to update your branch instead of merging main into it.
You can avoid annoying conflicts by doing this frequently keeping each conflict resolution small and self contained.

### Atomic Commits
Each commit should ideally do one thing.
For example, if you need to refactor a function in order to add a new feature cleanly, put the refactoring in one commit and the new feature in a different commit.
If the refactoring itself consists of many parts, try to separate those out into separate commits.
Include tests and documentation in the same commit as the code they test and document.

Each commit should pass the CI (see the justfile `ci` recipe for running CI locally).

Squash any fixup commits before marking your PR as ready for review.

### Commit Descriptions
Start your commits with a topic like `cli: add:` or `runner:`, take a look at the other commits or ask a contributor if you're unsure.

Document your commits, add a short summary and describe the what and why, not the how of a patch.
Here's a good writeup of [how to write good commit messages](https://cbea.ms/git-commit/).

---

If you have trouble squashing, amending or reordering commits try out [jj].
It which makes history rewriting very easy and is useful for git beginners and power-users alike.
If you prefer a GUI or TUI, there's also [gg] and [lazyjj] among others.

[jj]: https://github.com/martinvonz/jj
[gg]: https://github.com/gulbanana/gg
[lazyjj]: https://github.com/Cretezy/lazyjj
