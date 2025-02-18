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
|`dir`|Sets the direction used for creating difference documents, expects either `ltr` or `rtl` as an argument.|
|`ppi`|Sets the pixel per inch used for exporing and comparing documents, expects a floating point value as an argument.|
|`max-delta`|Sets the maximum allowed per-pixel delta, expects an integer between 0 and 255 as an argument.|
|`max-deviations`|Sets the maximum allwoed deviations, expects an integer as an argument.|
