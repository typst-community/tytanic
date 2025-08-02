#import "/book.typ": book-page
#show: book-page.with(title: "Watching For Changes")

Tytanic does not currently support a `watch` sub command the same way `typst` does.
However, you can work around this by using #link("https://watchexec.github.io/")[`watchexec`] or an equivalent tool which re-runs Tytanic whenever a file in your project changes.

Let's look at a concrete example with `watchexec`.
Navigate to your project root directory, i.e. that which contains your `typst.toml` manifest and run:
```shell
watchexec \
  --watch . \
  --clear \
  --ignore 'tests/**/diff/**' \
  --ignore 'tests/**/out/**' \
  --ignore 'tests/**/ref/**' \
  "tt run"
```

Of course a shell alias or task runner definition makes this more convenient.
While this is running, any change to a file in your project which is not excluded by the patterns provided using the `--ignore` flag will trigger a re-run of `tt run`.

If you have other files you may edit which don't influence the outcome of your test suite, then you should ignore them too.

#[//<div class="warning">
  Keep in mind that `tt run`, will run _all_ on every change, so this may not be appropriate for you if you have a large test suite.
]
