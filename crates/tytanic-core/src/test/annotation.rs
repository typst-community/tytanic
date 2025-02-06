//! Test annotations are used to add information to a test for `tytanic` to pick
//! up on.
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
//! ///
//! /// Synopsis:
//! /// ...
//!
//! #set page("a4")
//! ...
//! ```

use std::str::FromStr;

use ecow::{EcoString, EcoVec};
use thiserror::Error;

/// An error which may occur while parsing an annotation.
#[derive(Debug, Error)]
pub enum ParseAnnotationError {
    /// The delimiter were missing or unclosed.
    #[error("the annotation had only one or no delimiter")]
    MissingDelimiter,

    /// The annotation identifier is unknown, invalid or empty.
    #[error("unknown or invalid annotation identifier: {0:?}")]
    Unknown(EcoString),

    /// The annotation was otherwise malformed.
    #[error("the annotation was malformed")]
    Other,
}

/// A test annotation used to configure test specific behavior.
///
/// Test annotations are placed on doc comments at the top of a test's source
/// file:
///
/// Each annotation is on it's own line.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Annotation {
    /// The ignored annotation, this can be used to exclude a test by virtue of
    /// the `ignored` test set.
    Skip,
}

impl Annotation {
    /// Collects all annotations found within a test's source code.
    pub fn collect(source: &str) -> Result<EcoVec<Self>, ParseAnnotationError> {
        // skip regular comments and leading empty lines
        let lines = source.lines().skip_while(|line| {
            line.strip_prefix("//")
                .is_some_and(|rest| !rest.starts_with('/'))
                || line.trim().is_empty()
        });

        // then collect all consecutive doc comment lines
        let lines = lines.map_while(|line| line.strip_prefix("///").map(str::trim));

        // ignore empty ones
        let lines = lines.filter(|line| !line.is_empty());

        // take only those which start with an annotation deimiter
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

        let id = rest.trim();

        match id {
            "skip" => Ok(Annotation::Skip),
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
