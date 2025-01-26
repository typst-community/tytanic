# Contributing
Thank you for considering to contribute to `tytanic`.
Any contributions are welcome, from implementing large features to fixing small typos.

**If you're contributing for the first time to `tytanic`, please familiarize yourself with the workflow below.**

1. When you open a PR as a draft, it can be as messy as you want.
1. Once you request review, make sure you have cleaned up the PR:
   - It should have a linear history, rebase on main to update your branch instead of merging main into it.
   - Each commit should be atomic, they should compile and pass tests on their own.
   - Each commit should have a clear commit message with a short and long description (if applicable).
   - It should not contain any `fixup` or `review` commits, each commit should be a meaningful change to the code base.

   Each commit is reviewed in isolation, which is why clear commit messages and atomic commits are important.
1. When a change is requested, address this change by rewriting the commits and force pushing the branch, do not add `fixup` or `review` commits.
  You may add new commits if there are other atomic changes to do which are unrelated to the previous ones but must land in this PR.
1. Once reviewed it is either squashed and merged, or rebased on main.

## Commit Messages
Each of the final commits should have a clear commit message starting with a prefix indicating what was done:
- `fix` for fixing bugs
- `docs` for changing, fixing or adding new documentation
- `feat` for a new feature of any kind
- `chore` for anything else like cleanups or refactors, these should not have any externally observable change in behavior (other than speed ups)
- `cli` for change to the cli crate
- `lib` for changes to the library crate
- `book` for changes to the book
- `ci` for changes to ci workflows
- `vcs` for changes to features regarding VCS
- etc.

Prefixes can be chanined like `fix: cli: ...` to make it clear that a fix was cli specific.
This prefix is generally optional, but helps developers filter out commits when bug hunting.

This prefix should be followed by a short concise sumamry of the change.
The separated by an empty line a more elaborate description can and should be added, but of course there are commits that don't need this like `book: fix typo`.
Commit messages are is documentation for developers to understand reasoning behind changes, so don't be afraid to write an essay here, the more complicated a refactor or bug fix, the more elaborate the commit message should likely be.
Here's a commit message from the early code base:

```
docs(book): Add installation chapter

Adds an installation chapter to the quick start part of the book, this section
outlines methods of installation, as well as system dependencies such as
openssl.
```

This should give you an idea of what your commits should look like.

## Atomic Commits
Each commit in a PR should be self contained and pass all tests.
It's ok if this isn't always possible for big rewrites, the commits should simply be squashed down then

None of these restrictions can easily be enforced by CI, it is up to you as the contributor to uphold that.
A good way to ensure this is to think about the changes before writing them and rewriting your commit history by amending, splitting or reordering.

Here are some general guidelines:
- Keep refactors required for a feature separate to the feature itself.
- Code documentation, test adjustments or new test cases and features all belong in the same commit, do not split these up.
- If you find a bug while adding a new feature, add a bug fix in a separate commit.

## Linear History
`tytanic` imposes a linear history on its main branch, this means that PR's are not merged, but either squashed or rebased on top of main.
This means that the commits landing on main must likewise be a linear history, at least once added to main.

There are two was to add PRs to main:
1. The squash workflow: Squash all commits and add them to main.
   It's fairly easy to get a single commit on main which is atomic, but such commits may get unnecessarily large.
   This is avoided where possible.
1. The rebase workflow: All commits are added individually on main.
   This is done most often and the main reasons your commits should all individually be valid states of the repository.

## Addressing Review
Say you created a PR with this history:

```txt
◆ A fix(vcs): Avoid panic on UNC paths within escape check
│
◆ B feat(vcs): Add mercurial support
│
◆ C docs(book): Add mercurial chapter to cli reference
```

You open the PR and on review you agree to rename one of the Types introduced in `B`.
Instead of adding a new commit which does nothing substantial other than renaming a type, you amend the changes to the commit `B` and force push the branch.
It is up to you how you accomplish this, but one way to do it is to add a new commit and using `git rebase --interactive` to squash it into `B`.

I personally use [jj] for all my repos, which makes history rewriting and commit-fu very easy.
I can recommend it especially for this type of workflow.
If you prefer a GUI or TUI, there's also [gg] and [lazyjj] among others.

[jj]: https://github.com/martinvonz/jj
[gg]: https://github.com/gulbanana/gg
[lazyjj]: https://github.com/Cretezy/lazyjj
