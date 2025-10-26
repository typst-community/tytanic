//! Annotations are used to override the behavior of the executing test runner.
//!
//! Annotations may be placed on a leading doc comment block (indicated by
//! `///`), such a doc comment block can be placed after initial empty or
//! regular comment lines, but must come before any content. All annotations in
//! such a block must be at the start, once non-annotation content is
//! encountered parsing stops.
//!
//! ```typst
//! // SPDX-License-Identifier: MIT
//!
//! /// [skip]
//! /// [max-delta: 10]
//! ///
//! /// Synopsis:
//! /// ...
//!
//! #set page("a4")
//! ...
//! ```

use std::str::FromStr;

use chrono::DateTime;
use chrono::Utc;
use ecow::EcoString;
use ecow::EcoVec;
use thiserror::Error;

use crate::config::Direction;
use crate::config::Warnings;

/// An error which may occur while parsing an annotation.
#[derive(Debug, Error)]
pub enum ParseAnnotationError {
    /// The delimiter were missing or unclosed.
    #[error("the annotation had only one or no delimiter")]
    MissingDelimiter,

    /// The annotation identifier is unknown, invalid, or empty.
    #[error("unknown or invalid annotation identifier: {0:?}")]
    Unknown(EcoString),

    /// The annotation expected no argument, but received one.
    #[error("the annotation {0} expected no argument, but received one")]
    UnexpectedArg(&'static str),

    /// The annotation expected an argument, but received none.
    #[error("the annotation {0} expected an argument, but received none")]
    MissingArg(&'static str),

    #[error("the annotation {0} requires a key-value separator")]
    MissingInputSeparator(EcoString),

    /// An error occurred while parsing the annotation.
    #[error("an error occurred while parsing the annotation")]
    Other(#[source] Box<dyn std::error::Error + Sync + Send + 'static>),
}

/// An annotation used to configure test specific behavior.
#[derive(Debug, Clone, PartialEq)]
pub enum Annotation {
    /// The skip annotation, this adds a test to the built in `skip` test set.
    Skip,

    /// Whether to run the compare stage.
    Compare(bool),

    /// Whether to use system fonts.
    UseSystemFonts(bool),

    /// Whether to use the system date time.
    UseSystemDatetime(bool),

    /// Whether to use the augmented standard library.
    UseAugmentedLibrary(bool),

    /// The timestamp to use for the test.
    Timestamp(DateTime<Utc>),

    /// Whether to allow package imports for the test.
    AllowPackages(bool),

    /// How to handle warnings emitted by the test.
    Warnings(Warnings),

    /// The direction to use for diffing the documents.
    Dir(Direction),

    /// The pixel per inch to use for exporting the documents.
    Ppi(f32),

    /// The maximum allowed per pixel delta to use for comparison.
    MaxDelta(u8),

    /// The maximum allowed amount of deviations to use for comparison.
    MaxDeviations(usize),

    /// A key-value pair to expose in `sys.inputs` for the code running the test.
    Input { key: EcoString, value: EcoString },
}

impl Annotation {
    /// Collects all annotations found within a test's source code.
    pub fn collect(source: &str) -> Result<EcoVec<Self>, ParseAnnotationError> {
        // Skip regular comments and leading empty lines.
        let lines = source.lines().skip_while(|line| {
            line.strip_prefix("//")
                .is_some_and(|rest| !rest.starts_with('/'))
                || line.trim().is_empty()
        });

        // Then collect all consecutive doc comment lines.
        let lines = lines.map_while(|line| line.strip_prefix("///").map(str::trim));

        // Ignore empty ones.
        let lines = lines.filter(|line| !line.is_empty());

        // Take only those which start with an annotation delimiter.
        let lines = lines.take_while(|line| line.starts_with('['));

        lines.map(str::parse).collect()
    }
}

impl FromStr for Annotation {
    type Err = ParseAnnotationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some(rest) = s.strip_prefix('[') else {
            return Err(ParseAnnotationError::MissingDelimiter);
        };

        let Some(rest) = rest.strip_suffix(']') else {
            return Err(ParseAnnotationError::MissingDelimiter);
        };

        let (id, arg) = match rest.split_once(':') {
            Some((id, arg)) => (id, Some(arg.trim())),
            None => (rest, None),
        };

        match id.trim() {
            "skip" => {
                if arg.is_some() {
                    Err(ParseAnnotationError::UnexpectedArg("skip"))
                } else {
                    Ok(Annotation::Skip)
                }
            }
            "compare" => match arg {
                Some(arg) => match arg.trim().parse() {
                    Ok(arg) => Ok(Annotation::UseSystemFonts(arg)),
                    Err(err) => Err(ParseAnnotationError::Other(err.into())),
                },
                None => Err(ParseAnnotationError::MissingArg("use-system-fonts")),
            },
            "use-system-fonts" => match arg {
                Some(arg) => match arg.trim().parse() {
                    Ok(arg) => Ok(Annotation::UseSystemFonts(arg)),
                    Err(err) => Err(ParseAnnotationError::Other(err.into())),
                },
                None => Err(ParseAnnotationError::MissingArg("use-system-fonts")),
            },
            "use-system-datetime" => match arg {
                Some(arg) => match arg.trim().parse() {
                    Ok(arg) => Ok(Annotation::UseSystemDatetime(arg)),
                    Err(err) => Err(ParseAnnotationError::Other(err.into())),
                },
                None => Err(ParseAnnotationError::MissingArg("use-system-datetime")),
            },
            "use-augmented-library" => match arg {
                Some(arg) => match arg.trim().parse() {
                    Ok(arg) => Ok(Annotation::UseAugmentedLibrary(arg)),
                    Err(err) => Err(ParseAnnotationError::Other(err.into())),
                },
                None => Err(ParseAnnotationError::MissingArg("use-augmented-library")),
            },
            "timestamp" => match arg {
                Some(arg) => match arg.trim().parse() {
                    Ok(arg) => Ok(Annotation::Timestamp(arg)),
                    Err(err) => Err(ParseAnnotationError::Other(err.into())),
                },
                None => Err(ParseAnnotationError::MissingArg("timestamp")),
            },
            "allow-packages" => match arg {
                Some(arg) => match arg.trim().parse() {
                    Ok(arg) => Ok(Annotation::AllowPackages(arg)),
                    Err(err) => Err(ParseAnnotationError::Other(err.into())),
                },
                None => Err(ParseAnnotationError::MissingArg("allow-packages")),
            },
            "warnings" => match arg {
                Some(arg) => match arg.trim() {
                    "ignore" => Ok(Annotation::Dir(Direction::Ltr)),
                    "emit" => Ok(Annotation::Dir(Direction::Rtl)),
                    "promote" => Ok(Annotation::Dir(Direction::Rtl)),
                    _ => Err(ParseAnnotationError::Other(
                        format!("invalid warning configuration {arg:?}, expected one of ignore, emit or promote").into(),
                    )),
                },
                None => Err(ParseAnnotationError::MissingArg("warnings")),
            },
            "dir" => match arg {
                Some(arg) => match arg.trim() {
                    "ltr" => Ok(Annotation::Dir(Direction::Ltr)),
                    "rtl" => Ok(Annotation::Dir(Direction::Rtl)),
                    _ => Err(ParseAnnotationError::Other(
                        format!("invalid direction {arg:?}, expected one of ltr or rtl").into(),
                    )),
                },
                None => Err(ParseAnnotationError::MissingArg("dir")),
            },
            "ppi" => match arg {
                Some(arg) => match arg.trim().parse() {
                    Ok(arg) => Ok(Annotation::Ppi(arg)),
                    Err(err) => Err(ParseAnnotationError::Other(err.into())),
                },
                None => Err(ParseAnnotationError::MissingArg("ppi")),
            },
            "max-delta" => match arg {
                Some(arg) => match arg.trim().parse() {
                    Ok(arg) => Ok(Annotation::MaxDelta(arg)),
                    Err(err) => Err(ParseAnnotationError::Other(err.into())),
                },
                None => Err(ParseAnnotationError::MissingArg("max-delta")),
            },
            "max-deviations" => match arg {
                Some(arg) => match arg.trim().parse() {
                    Ok(arg) => Ok(Annotation::MaxDeviations(arg)),
                    Err(err) => Err(ParseAnnotationError::Other(err.into())),
                },
                None => Err(ParseAnnotationError::MissingArg("max-deviations")),
            },
            "input" => match arg {
                Some(arg) => match arg.trim().split_once('=') {
                    Some((key, value)) => Ok(Annotation::Input {
                        key: key.into(),
                        value: value.into(),
                    }),
                    None => Err(ParseAnnotationError::MissingInputSeparator(arg.into())),
                },
                None => Err(ParseAnnotationError::MissingArg("input")),
            },
            _ => Err(ParseAnnotationError::Unknown(id.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotation_from_str() {
        assert_eq!(Annotation::from_str("[skip]").unwrap(), Annotation::Skip);
        assert_eq!(Annotation::from_str("[ skip  ]").unwrap(), Annotation::Skip);

        assert!(Annotation::from_str("[ skip  ").is_err());
        assert!(Annotation::from_str("[unknown]").is_err());
    }

    #[test]
    fn test_annotation_unexpected_arg() {
        assert!(Annotation::from_str("[skip:]").is_err());
        assert!(Annotation::from_str("[skip: 10]").is_err());
    }

    #[test]
    fn test_annotation_expected_arg() {
        assert!(Annotation::from_str("[ppi]").is_err());
        assert!(Annotation::from_str("[max-delta:]").is_err());
    }

    #[test]
    fn test_annotation_arg() {
        assert_eq!(
            Annotation::from_str("[max-deviations: 20]").unwrap(),
            Annotation::MaxDeviations(20)
        );
        assert_eq!(
            Annotation::from_str("[ppi: 42.5]").unwrap(),
            Annotation::Ppi(42.5)
        );
    }

    #[test]
    fn test_annotation_input() {
        assert_eq!(
            Annotation::from_str("[input: FOO=BAR]").unwrap(),
            Annotation::Input {
                key: "FOO".into(),
                value: "BAR".into()
            },
        );

        let ret = Annotation::from_str("[input: NO_VALUE]");
        assert!(matches!(
            ret,
            Err(ParseAnnotationError::MissingInputSeparator(_))
        ));
    }

    #[test]
    fn test_annotation_multiple() {
        let source = r#"
/// [input: THIS=should]
/// [input: WORK = well]
"#;
        dbg!(&source);

        assert_eq!(
            Annotation::collect(source).unwrap(),
            [
                Annotation::Input {
                    key: "THIS".into(),
                    value: "should".into()
                },
                Annotation::Input {
                    key: "WORK ".into(),
                    value: " well".into()
                },
            ]
        );
    }

    #[test]
    fn test_collect_book_example() {
        let source = "\
        /// [skip]    \n\
        ///           \n\
        /// Synopsis: \n\
        /// ...       \n\
                      \n\
        #import \"/src/internal.typ\": foo \n\
        ...";

        assert_eq!(Annotation::collect(source).unwrap(), [Annotation::Skip]);
    }

    #[test]
    fn test_collect_issue_109() {
        assert_eq!(
            Annotation::collect("///[skip]").unwrap(),
            [Annotation::Skip]
        );
        assert_eq!(Annotation::collect("///").unwrap(), []);
        assert_eq!(
            Annotation::collect("/// [skip]").unwrap(),
            [Annotation::Skip]
        );
        assert_eq!(
            Annotation::collect("///[skip]\n///").unwrap(),
            [Annotation::Skip]
        );
    }
}
