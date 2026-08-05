# Config
There are two kinds of configs, system configs and the project config, these have different but overlapping.

## Project Config
The project config is specified in the `typst.toml` manifest under the `tool.tytanic` section.

|Key|Default|Description|
|---|---|---|
|`tests`|`"tests"`|The path in which unit tests are found, relative to the project root.|
|`default.dir`|`ltr`|Sets the default direction used for creating difference documents, expects either `ltr` or `rtl` as an argument. Can be overridden per test using an annotation.|
|`default.ppi`|`144.0`|Sets the default pixel per inch used for exporting and comparing documents, expects a floating point value as an argument. Can be overridden per test using an annotation.|
|`default.max-delta`|`1`|Sets the default maximum allowed per-pixel delta, expects an integer between 0 and 255 as an argument. Can be overridden per test using an annotation.|
|`default.max-deviations`|`0`|Sets the default maximum allowed deviations, expects an integer as an argument. Can be overridden per test using an annotation.|
|`default.use-system-fonts`|`false`|Sets whether system fonts are used by default. Can be overridden using `--use-system-fonts` or `--no-use-system-fonts`.|

> [!WARNING]
> Even small font version variations can create sub-pixel differences that will be very hard to debug. Only use the system fonts if you are certain your environment is completely reproducible to all contributors.

## System Config
There are currently no system config options and the config is not yet loaded.
