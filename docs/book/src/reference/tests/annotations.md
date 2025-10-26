# Annotations
Test annotations are used to add information to a test for Tytanic to pick up on.

Annotations may be placed on a leading doc comment block (indicated by `///`), such a doc comment block can be placed after initial empty or regular comment lines, but must come before any content.
All annotations in such a block must be at the start, once non-annotation content is encountered parsing stops.

For ephemeral regression tests only the main test file will be checked for annotations, the reference file will be ignored.

<div class="warning">

The syntax for annotations may change if Typst adds first class annotation or documentation comment syntax.

</div>

```typst
// SPDX-License-Identifier: MIT

/// [skip]
/// [max-delta: 5]
///
/// Synopsis:
/// ...

#import "/src/internal.typ": foo
...
```

The following annotations are available:

|Annotation|Description|
|---|---|
|`skip`|Marks the test as part of the `skip()` test set.|
|`use-system-fonts`|Sets whether to use system fonts.|
|`use-system-datetime`|Sets whether to use system date and time.|
|`use-augmented-library`|Sets whether the augmented standard library is available.|
|`timestamp`|Sets the timestamp, the argument is parsed from the [RFC#3339][rfc] format.|
|`allow-packages`|Sets whether to allow external packages.|
|`dir`|Sets the direction used for creating difference documents, expects either `ltr` or `rtl` as an argument.|
|`ppi`|Sets the pixel per inch used for exporting and comparing documents, expects a floating point value as an argument.|
|`max-delta`|Sets the maximum allowed per-pixel delta, expects an integer between 0 and 255 as an argument.|
|`max-deviations`|Sets the maximum allowed deviations, expects an integer as an argument.|

The defaults can be configured in the `tool.tytanic.default` section in the `typst.toml` manifest.

## Skip
The skip annotation adds a test to the `skip()` test set, this is a special test set that is automatically wrapped around the `--expression` option `(...) ~ skip()`.
This implicit skip set can be disabled using `--no-skip`.

[rfc]: https://datatracker.ietf.org/doc/html/rfc3339

