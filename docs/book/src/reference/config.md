# Config
There are two kinds of configs, system configs and the project config, these have different but overlapping.

## Project Config
The project config is specified in the `typst.toml` manifest under the `tool.tytanic` section.

|Key|Default|Description|
|---|---|---|
|`tests`|`"tests"`|The path in which unit tests are found, relative to the project root.|
|`default.use-system-fonts`|`false`|Sets whether to use system fonts. Can be overridden pe test using an annotation.|
|`default.use-system-datetime`|`false`|Sets whether to use system date and time. Can be overridden pe test using an annotation.|
|`default.use-augmented-library`|`true` for unit tests, `false` otherwise|Sets whether the augmented standard library is available. Can be overridden pe test using an annotation.|
|`timestamp`|`1970-01-01T00:00:00+00:00` (`UNIX_EPOCH`)|Sets the timestamp, the argument is parsed from the [RFC#3339][rfc] format. Can be overridden per test using an annotation.|
|`default.allow-packages`|`true`|Sets whether to allow external packages. Can be overridden per test using an annotation.|
|`default.dir`|`ltr`|Sets the default direction used for creating difference documents, expects either `ltr` or `rtl` as an argument. Can be overridden per test using an annotation.|
|`default.ppi`|`144.0`|Sets the default pixel per inch used for exporting and comparing documents, expects a floating point value as an argument. Can be overridden per test using an annotation.|
|`default.max-delta`|`1`|Sets the default maximum allowed per-pixel delta, expects an integer between 0 and 255 as an argument. Can be overridden per test using an annotation.|
|`default.max-deviations`|`0`|Sets the default maximum allowed deviations, expects an integer as an argument. Can be overridden per test using an annotation.|

## System Config
There are currently no system config options and the config is not yet loaded.
