#import "/src/packages.typ": *

#import "/book.typ": book-page
#show: book-page.with(title: "Tests")

There are three types of tests:
- Unit tests, which are similar to unit or integration tests in other languages and are mostly used to test the API of a package and visual regressions through comparison with reference documents.
  Unit tests are standalone files in a `tests` directory inside the project root and have additional features available inside Typst using a custom standard library.
- Template tests, these are automatically created from a template package's template directory and may not access the augmented standard library.
  Note that there are also unit tests which can access the template directory assets.
  Instead, they receive access to the template assets.
- Doc tests, example code in documentation comments which are compiled but not compared.

#[//<div class="warning">
  Tytanic can currently not collect doc tests.

  These will be added in the future, see #link("https://github.com/typst-community/tytanic/issues/34")[#34].
]

Any unit test may use #shiroa.cross-link("/src/reference/tests/annotations.typ")[annotations] for configuration.

Read the #shiroa.cross-link("/src/guides/tests.typ")[guide], if you want to see some examples on how to write and run various tests.

= Sections
- #shiroa.cross-link("/src/reference/tests/unit.typ")[Unit tests] explains the structure of unit tests.
- #shiroa.cross-link("/src/reference/tests/template.typ")[Template tests] the usage of template tests.
- #shiroa.cross-link("/src/reference/tests/lib.typ")[Test library] lists the declarations of the custom standard library.
- #shiroa.cross-link("/src/reference/tests/annotations.typ")[Annotations] lists the syntax for annotations and which are available.
