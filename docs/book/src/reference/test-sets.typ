#import "/src/packages.typ": *

#import "/book.typ": book-page
#show: book-page.with(title: "Test Set Language")

The test set language is an expression based language, top level expression can be built up form smaller expressions consisting of binary and unary operators and built-in functions.
They form sets which are used to specify which test should be selected for various operations.

Read the #shiroa.cross-link("/src/guides/test-sets.typ")[guide], if you want to see examples of how to use test sets.

= Sections
- #shiroa.cross-link("/src/reference/test-sets/grammar.typ")[Grammar] outlines operators and syntax.
- #shiroa.cross-link("/src/reference/test-sets/eval.typ")[Evaluation] explains the evaluation of test set expressions.
- #shiroa.cross-link("/src/reference/test-sets/built-in.typ")[Built-in Test Sets] lists built-in test sets and functions.
