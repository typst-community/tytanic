//! Helper functions and types for paths.

use std::path::Path;

/// Returns the lexical common ancestor of two paths if there is any. This means
/// it will not canonicalize paths.
///
/// # Example
/// ```no_run
/// # use std::path::Path;
/// # use tytanic_utils::path::common_ancestor;
/// assert_eq!(
///     common_ancestor(Path::new("foo/bar"), Path::new("foo/baz")),
///     Some(Path::new("foo")),
/// );
/// assert_eq!(
///     common_ancestor(Path::new("foo/bar"), Path::new("/foo/baz")),
///     None,
/// );
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn common_ancestor<'a>(p: &'a Path, q: &'a Path) -> Option<&'a Path> {
    let mut paths = [p, q];
    paths.sort_by_key(|p| p.as_os_str().len());
    let [short, long] = paths;

    // find the longest match where long starts with short
    short.ancestors().find(|a| long.starts_with(a))
}

/// Returns whether `base` is an ancestor of `path` lexically. This means it
/// will not canonicalize paths and may not always be correct (e.g. in the
/// presence of symlinks or filesystem mounts).
///
/// # Example
/// ```no_run
/// # use tytanic_utils::path::is_ancestor_of;
/// assert_eq!(is_ancestor_of("foo/", "foo/baz"), true);
/// assert_eq!(is_ancestor_of("foo/", "/foo/baz"), false);
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn is_ancestor_of<P: AsRef<Path>, Q: AsRef<Path>>(base: P, path: Q) -> bool {
    fn inner(base: &Path, path: &Path) -> bool {
        common_ancestor(base, path).is_some_and(|common| common == base)
    }

    inner(base.as_ref(), path.as_ref())
}
