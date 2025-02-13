# Template Test
Template tests are automatically created for template packages, they receive a special identifier `@template` and cannot be added, updated or removed.
They act like compile-only tests and are part of the `template()` test set.

## Import Translation
The import for the package itself is automatically resolved to the local project directory.
This way, template test can run on unpublished versions without installing the package locally for every change.
At the moment this re-routing of the import is done by comparing the package version and name of the import with that of the current project, assuming the project itself will be published on the `preview` namespace.
