# Typst Compatibility
Tytanic tries to stay close to Typst's release cycle, this means that for each Typst minor version, there should be at least one corresponding minor version of Tytanic.
Release candidates may be exposed via nightly, but not as a corresponding Tytanic release as of yet.

Tytanic will backport patch releases where necessary, but only to the latest corresponding minor version.
Assuming that both Tytanic `v0.15.0` and Tytanic `v0.16.0` target Typst `v0.42.0`, then a new patch version of Typst `v0.42.1` would only be backported as `v0.16.1`, but _not_ to `v0.15.1`.
See the table below for the correspondance of Typst's and Tytanic's versions.

<div class="warning">

If you really need to use Tytanic `v0.15.0` with such a patch of Typst `v0.42.1`, then an installation method like `cargo install` _without_ the `--locked` flag may already be enough.
So, even though these patches may not be explicitly released, they can most often be installed without any issues by compiling from source.

</div>

This one-to-many correspondence doesn't necessarily mean that each Tytanic version can only compile tests for the Typst verison it targets, but simply that it makes no guarantees about supporting more than that version.
This is mostly relevant for the stability of its test output, as changes in Typst's default styles and fonts may change the output of a test.

Tytanic was first released when Typst was at version `v0.12.0` and does not provide any versions for Typst `<= v0.11.1`.

<div class="warning">

Note that this may change in the future, if compatibility with multiple versions becomes desirable.
For example, if Typst releases `v1.0`, Tytanic may start to support more than one Typst version per Tytanic version.

</div>

The following table describes all Tytanic and Typst verions and how they correspond to each other.

|Typst|Tytanic|Note|
|---|---|---|
|`<= v0.11.1`|none|unsupported|
|`== v0.12.0`|`v0.1.0 .. v0.1.3`|
|`== v0.13.0-rc1`|`v0.2.0-rc1`|
|`== v0.13.0`|`v0.2.0 .. v0.2.2`|
|`>= v0.13.0`|none|unsupported|

