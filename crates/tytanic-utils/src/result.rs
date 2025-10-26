//! Extensions for the [`Result`] type.

use std::error::Error;
use std::fmt::Display;
use std::io;
use std::path::PathBuf;

use crate::private::Sealed;

/// An error with an associated path.
#[derive(Debug)]
pub struct PathError<E> {
    /// The path associated with the error.
    pub path: PathBuf,

    /// The inner error.
    pub error: E,
}

impl<E> Display for PathError<E>
where
    E: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation on {:?} failed", self.path)
    }
}

impl<E> Error for PathError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}

/// Extensions for the [`Result`] type.
#[allow(private_bounds)]
pub trait ResultEx<T, E>: Sealed {
    /// Ignores the subset of the error for which the `check` returns true,
    /// returning `None` instead.
    ///
    /// # Examples
    /// ```no_run
    /// # use std::fs;
    /// # use std::io::ErrorKind;
    /// use tytanic_utils::result::ResultEx;
    /// // if foo doesn't exist we get None
    /// // if another error is returned it is propagated
    /// assert_eq!(
    ///     fs::read_to_string("foo.txt").ignore(
    ///         |e| e.kind() == ErrorKind::NotFound,
    ///     )?,
    ///     Some(String::from("foo")),
    /// );
    /// assert_eq!(
    ///     fs::read_to_string("not-found.txt").ignore(
    ///         |e| e.kind() == ErrorKind::NotFound,
    ///     )?,
    ///     None,
    /// );
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    fn ignore<F>(self, check: F) -> Result<Option<T>, E>
    where
        F: FnOnce(&E) -> bool;

    /// Ignores the subset of the error for which the `check` returns true,
    /// returning `Default::default` instead.
    ///
    /// # Examples
    /// ```no_run
    /// # use std::fs;
    /// # use std::io::ErrorKind;
    /// use tytanic_utils::result::ResultEx;
    /// // if foo doesn't exist we get ""
    /// // if another error is returned it is propagated
    /// assert_eq!(
    ///     fs::read_to_string("foo.txt").ignore_default(
    ///         |e| e.kind() == ErrorKind::NotFound,
    ///     )?,
    ///     String::from("foo"),
    /// );
    /// assert_eq!(
    ///     fs::read_to_string("not-found.txt").ignore_default(
    ///         |e| e.kind() == ErrorKind::NotFound,
    ///     )?,
    ///     String::new(),
    /// );
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    fn ignore_default<F>(self, check: F) -> Result<T, E>
    where
        T: Default,
        F: FnOnce(&E) -> bool;

    /// Ignores the subset of the error for which the `check` returns true,
    /// returning the result of `value` instead.
    ///
    /// # Examples
    /// ```no_run
    /// # use std::fs;
    /// # use std::io::ErrorKind;
    /// use tytanic_utils::result::ResultEx;
    /// // if foo doesn't exist we get "foo"
    /// // if another error is returned it is propagated
    /// assert_eq!(
    ///     fs::read_to_string("foo.txt").ignore_with(
    ///         |e| e.kind() == ErrorKind::NotFound,
    ///         |_| String::from("foo"),
    ///     )?,
    ///     String::from("foo"),
    /// );
    /// assert_eq!(
    ///     fs::read_to_string("not-found.txt").ignore_with(
    ///         |e| e.kind() == ErrorKind::NotFound,
    ///         |_| String::from("foo"),
    ///     )?,
    ///     String::from("bar"),
    /// );
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    fn ignore_with<F, G>(self, check: F, value: G) -> Result<T, E>
    where
        F: FnOnce(&E) -> bool,
        G: FnOnce(&E) -> T;

    /// Attaches a path to this result in the error case.
    ///
    /// # Example
    /// ```no_run
    /// # use std::fs;
    /// use tytanic_utils::result::ResultEx;
    /// // if foo doesn't exist we get an error with the path attached to it
    /// assert_eq!(
    ///     fs::read_to_string("foo.txt").path("foo.txt")?,
    ///     String::from("foo"),
    /// );
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    fn path<P>(self, path: P) -> Result<T, PathError<E>>
    where
        P: Into<PathBuf>;

    /// Attaches a path to this result in the error case.
    ///
    /// # Example
    /// ```no_run
    /// # use std::fs;
    /// use tytanic_utils::result::ResultEx;
    /// // if foo doesn't exist we get an error with the path attached to it
    /// assert_eq!(
    ///     fs::read_to_string("foo.txt").path_with(|| "foo.txt".into())?,
    ///     String::from("foo"),
    /// );
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    fn path_with<F>(self, f: F) -> Result<T, PathError<E>>
    where
        F: FnOnce() -> PathBuf;
}

impl<T, E> ResultEx<T, E> for Result<T, E> {
    fn ignore<F>(self, check: F) -> Result<Option<T>, E>
    where
        F: FnOnce(&E) -> bool,
    {
        self.map(Some).ignore_with(check, |_| None)
    }

    fn ignore_default<F>(self, check: F) -> Result<T, E>
    where
        T: Default,
        F: FnOnce(&E) -> bool,
    {
        self.ignore_with(check, |_| T::default())
    }

    fn ignore_with<F, G>(self, check: F, value: G) -> Result<T, E>
    where
        F: FnOnce(&E) -> bool,
        G: FnOnce(&E) -> T,
    {
        match self {
            Err(err) if check(&err) => Ok(value(&err)),
            x => x,
        }
    }

    fn path<P>(self, path: P) -> Result<T, PathError<E>>
    where
        P: Into<PathBuf>,
    {
        self.map_err(|error| PathError {
            path: path.into(),
            error,
        })
    }

    fn path_with<F>(self, f: F) -> Result<T, PathError<E>>
    where
        F: FnOnce() -> PathBuf,
    {
        self.map_err(|error| PathError { path: f(), error })
    }
}

/// A check for [`ResultEx`] methods which ignores [`io::ErrorKind::NotFound`].
///
/// # Examples
/// ```no_run
/// # use std::fs;
/// # use std::io::ErrorKind;
/// use tytanic_utils::result::ResultEx;
/// use tytanic_utils::result::io_not_found;
/// assert_eq!(
///     fs::read_to_string("found.txt").ignore_default(io_not_found)?,
///     String::from("foo"),
/// );
/// assert_eq!(
///     fs::read_to_string("not-found.txt").ignore_default(io_not_found)?,
///     String::from(""),
/// );
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn io_not_found(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::NotFound
}
