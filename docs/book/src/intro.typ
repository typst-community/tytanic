#import "/src/packages.typ": *

#import "/book.typ": book-page
#show: book-page.with(title: "Introduction")

Tytanic is a test runner for #link("https://typst.app/")[Typst] projects.
It helps you worry less about regressions and speeds up your development.

// TODO(tinger): Either use prequery or raw elements or whatever.
// <a href="https://asciinema.org/a/rW9HGUBbtBnmkSddgbKb7hRlI" target="_blank"><img src="https://asciinema.org/a/rW9HGUBbtBnmkSddgbKb7hRlI.svg" /></a>

 Bird's-Eye View
Out of the box Tytanic supports the following features:
- compile and compare tests
- manage tests of various types
- manage and update reference documents when tests change
- filter tests effectively for concise test runs

= A Closer Look
This book contains a few sections aimed at answering the most common questions right out the gate:
- #shiroa.cross-link("/src/quickstart/install.typ")[Installation] outlines various ways to install Tytanic.
- #shiroa.cross-link("/src/quickstart/usage.typ")[Usage] goes over some basic commands to get started.

After the quick start, a few guides delve deeper into some advanced topics, such as
- #shiroa.cross-link("/src/guides/tests.typ")[Writing Tests] shows how tests work and how you can add, remove, and update them.
- #shiroa.cross-link("/src/guides/test-sets.typ")[Using Test Sets] delves into the test set language and how it can be used to isolate tests and speed up your TDD workflow.
- #shiroa.cross-link("/src/guides/watching.typ")[Watching for Changes] explains a workaround for how you can run tests repeatedly on changes to your project files.
- #shiroa.cross-link("/src/guides/ci.typ")[Setting Up CI] shows how to set up Tytanic in your CI.

The later sections of the book are a technical reference to Tytanic and its various features or concepts:
- #shiroa.cross-link("/src/reference/compat.typ")[Typst Compatibility] shows which versions of Typst are currently supported and in which version of Tytanic.
- #shiroa.cross-link("/src/reference/tests.typ")[Tests] explains all features of tests in-depth.
- #shiroa.cross-link("/src/reference/test-sets.typ")[Test Set Language] explains the ins and outs of the test set language, listing its operators, built-in bindings and syntactic and semantic intricacies.
- #shiroa.cross-link("/src/reference/config.typ")[Configuration Schema] lists all existing config options, their expected types and default values.

