#import "src/packages.typ": *
#import shiroa: *

#show: book

#shiroa.book-meta(
  title: "tytanic",
  description: "A guide and reference document for tytanic.",
  repository: "https://github.com/typst-community/tytanic",
  repository-edit: "https://github.com/typst-community/tytanic/edit/main/docs/book/{path}",
  authors: ("tingerrr <tinger@tinger.dev>",),
  language: "en",
  summary: [
    #prefix-chapter("src/intro.typ")[Introduction]

    = Quickstart
    - #chapter("src/quickstart/install.typ")[Installation]
    - #chapter("src/quickstart/usage.typ")[Usage]

    = Guides
    - #chapter("src/guides/tests.typ")[Writing Tests]
    - #chapter("src/guides/test-sets.typ")[Using Test Sets]
    - #chapter("src/guides/watching.typ")[Watching for Changes]
    - #chapter("src/guides/ci.typ")[Setting Up CI]

    = Reference
    - #chapter("src/reference/compat.typ")[Typst Compatibility]
    - #chapter("src/reference/tests.typ")[Tests]
      - #chapter("src/reference/tests/unit.typ")[Unit tests]
      - #chapter("src/reference/tests/template.typ")[Template tests]
      - #chapter(none)[Documentation tests]
      - #chapter("src/reference/tests/annotations.typ")[Annotations]
      - #chapter("src/reference/tests/lib.typ")[Test Library]
    - #chapter("src/reference/test-sets.typ")[Test Set Language]
      - #chapter("src/reference/test-sets/grammar.typ")[Grammar]
      - #chapter("src/reference/test-sets/eval.typ")[Evaluation]
      - #chapter("src/reference/test-sets/built-in.typ")[Built-in Test Sets]
    - #chapter("src/reference/config.typ")[Configuration Schema]
  ],
)

#build-meta(
  dest-dir: "build",
)

#import "template/template.typ": project, heading-reference
#let book-page = project
#let cross-link = cross-link
#let heading-reference = heading-reference
