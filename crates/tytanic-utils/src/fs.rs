//! Helper functions and types for filesystem interactions, including unit test
//! helpers.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::{fs, io};

use tempdir::TempDir;

use crate::result::{io_not_found, ResultEx};

/// The prefix used for temporary directories in [`TempTestEnv`].
pub const TEMP_DIR_PREFIX: &str = "tytanic-utils";

/// Creates a new directory and its parent directories if `all` is specified,
/// but doesn't fail if it already exists.
///
/// # Example
/// ```no_run
/// # use tytanic_utils::fs::create_dir;
/// create_dir("foo", true)?;
/// create_dir("foo", true)?; // second time doesn't fail
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn create_dir<P>(path: P, all: bool) -> io::Result<()>
where
    P: AsRef<Path>,
{
    fn inner(path: &Path, all: bool) -> io::Result<()> {
        let res = if all {
            fs::create_dir_all(path)
        } else {
            fs::create_dir(path)
        };
        res.ignore_default(|e| e.kind() == io::ErrorKind::AlreadyExists)
    }

    inner(path.as_ref(), all)
}

/// Removes a file, but doesn't fail if it doesn't exist.
///
/// # Example
/// ```no_run
/// # use tytanic_utils::fs::remove_file;
/// remove_file("foo.txt")?;
/// remove_file("foo.txt")?; // second time doesn't fail
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn remove_file<P>(path: P) -> io::Result<()>
where
    P: AsRef<Path>,
{
    fn inner(path: &Path) -> io::Result<()> {
        std::fs::remove_file(path).ignore_default(io_not_found)
    }

    inner(path.as_ref())
}

/// Removes a directory, but doesn't fail if it doesn't exist.
///
/// # Example
/// ```no_run
/// # use tytanic_utils::fs::remove_dir;
/// remove_dir("foo", true)?;
/// remove_dir("foo", true)?; // second time doesn't fail
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn remove_dir<P>(path: P, all: bool) -> io::Result<()>
where
    P: AsRef<Path>,
{
    fn inner(path: &Path, all: bool) -> io::Result<()> {
        let res = if all {
            fs::remove_dir_all(path)
        } else {
            fs::remove_dir(path)
        };

        res.ignore_default(|e| {
            if io_not_found(e) {
                let parent_exists = path
                    .parent()
                    .and_then(|p| p.try_exists().ok())
                    .is_some_and(|b| b);

                if !parent_exists {
                    tracing::error!(?path, "tried removing dir, but parent did not exist");
                }

                parent_exists
            } else {
                false
            }
        })
    }

    inner(path.as_ref(), all)
}

/// Creates an empty directory, removing any content if it already existed. The
/// `all` argument is passed through to [`std::fs::create_dir`].
///
/// # Example
/// ```no_run
/// # use tytanic_utils::fs::ensure_empty_dir;
/// ensure_empty_dir("foo", true)?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn ensure_empty_dir<P>(path: P, all: bool) -> io::Result<()>
where
    P: AsRef<Path>,
{
    fn inner(path: &Path, all: bool) -> io::Result<()> {
        let res = remove_dir(path, true);
        if all {
            // if there was nothing to clear, then we simply go on to creation
            res.ignore_default(io_not_found)?;
        } else {
            res?;
        }

        create_dir(path, all)
    }

    inner(path.as_ref(), all)
}

/// Creates a temporary test environment in which files and directories can be
/// prepared and checked against after the test ran.
#[derive(Debug)]
pub struct TempTestEnv {
    root: TempDir,
    found: BTreeMap<PathBuf, Option<Vec<u8>>>,
    expected: BTreeMap<PathBuf, Option<Option<Vec<u8>>>>,
}

/// Set up the project structure.
///
/// See [`TempTestEnv::run`] and [`TempTestEnv::run_no_check`].
pub struct Setup(TempTestEnv);

impl Setup {
    /// Create a directory and all its parents within the test root.
    ///
    /// May panic if io errros are encountered.
    pub fn setup_dir<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        let abs_path = self.0.root.path().join(path.as_ref());
        create_dir(abs_path, true).unwrap();
        self
    }

    /// Create a file and all its parent directories within the test root.
    ///
    /// May panic if io errros are encountered.
    pub fn setup_file<P: AsRef<Path>>(&mut self, path: P, content: impl AsRef<[u8]>) -> &mut Self {
        let abs_path = self.0.root.path().join(path.as_ref());
        let parent = abs_path.parent().unwrap();
        if parent != self.0.root.path() {
            create_dir(parent, true).unwrap();
        }

        let content = content.as_ref();
        std::fs::write(&abs_path, content).unwrap();
        self
    }

    /// Create a directory and all its parents within the test root.
    ///
    /// May panic if io errros are encountered.
    pub fn setup_file_empty<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        let abs_path = self.0.root.path().join(path.as_ref());
        let parent = abs_path.parent().unwrap();
        if parent != self.0.root.path() {
            create_dir(parent, true).unwrap();
        }

        std::fs::write(&abs_path, "").unwrap();
        self
    }
}

/// Specify what you expect to see after the test concluded.
///
/// See [`TempTestEnv::run`].
pub struct Expect(TempTestEnv);

impl Expect {
    /// Ensure a directory exists after a test ran.
    pub fn expect_dir<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        self.0.add_expected(path.as_ref().to_path_buf(), None);
        self
    }

    /// Ensure a file exists after a test ran.
    pub fn expect_file<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        self.0.add_expected(path.as_ref().to_path_buf(), Some(None));
        self
    }

    /// Ensure a file with the given content exists after a test ran.
    pub fn expect_file_content<P: AsRef<Path>>(
        &mut self,
        path: P,
        content: impl AsRef<[u8]>,
    ) -> &mut Self {
        let content = content.as_ref();
        self.0
            .add_expected(path.as_ref().to_path_buf(), Some(Some(content.to_owned())));
        self
    }

    /// Ensure an empty file exists after a test ran.
    pub fn expect_file_empty<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        self.0.add_expected(path.as_ref().to_path_buf(), None);
        self
    }
}

impl TempTestEnv {
    /// Create a test enviroment and run the given test in it.
    ///
    /// The given closures for `setup` and `expect` set up the test environment
    /// and configure the expected end state respectively.
    pub fn run(
        setup: impl FnOnce(&mut Setup) -> &mut Setup,
        test: impl FnOnce(&Path),
        expect: impl FnOnce(&mut Expect) -> &mut Expect,
    ) {
        let dir = Self {
            root: TempDir::new(TEMP_DIR_PREFIX).unwrap(),
            found: BTreeMap::new(),
            expected: BTreeMap::new(),
        };

        let mut s = Setup(dir);
        setup(&mut s);
        let Setup(dir) = s;

        test(dir.root.path());

        let mut e = Expect(dir);
        expect(&mut e);
        let Expect(mut dir) = e;

        dir.collect();
        dir.assert();
    }

    /// Create a test enviroment and run the given test in it.
    ///
    /// This is the same as [`TempTestEnv::run`], but does not check the
    /// resulting directory structure.
    pub fn run_no_check(setup: impl FnOnce(&mut Setup) -> &mut Setup, test: impl FnOnce(&Path)) {
        let dir = Self {
            root: TempDir::new(TEMP_DIR_PREFIX).unwrap(),
            found: BTreeMap::new(),
            expected: BTreeMap::new(),
        };

        let mut s = Setup(dir);
        setup(&mut s);
        let Setup(dir) = s;

        test(dir.root.path());
    }
}

impl TempTestEnv {
    fn add_expected(&mut self, expected: PathBuf, content: Option<Option<Vec<u8>>>) {
        for ancestor in expected.ancestors() {
            self.expected.insert(ancestor.to_path_buf(), None);
        }
        self.expected.insert(expected, content);
    }

    fn add_found(&mut self, found: PathBuf, content: Option<Vec<u8>>) {
        for ancestor in found.ancestors() {
            self.found.insert(ancestor.to_path_buf(), None);
        }
        self.found.insert(found, content);
    }

    fn read(&mut self, path: PathBuf) {
        let rel = path.strip_prefix(self.root.path()).unwrap().to_path_buf();
        if path.metadata().unwrap().is_file() {
            let content = std::fs::read(&path).unwrap();
            self.add_found(rel, Some(content));
        } else {
            let mut empty = true;
            for entry in path.read_dir().unwrap() {
                let entry = entry.unwrap();
                self.read(entry.path());
                empty = false;
            }

            if empty && self.root.path() != path {
                self.add_found(rel, None);
            }
        }
    }

    fn collect(&mut self) {
        self.read(self.root.path().to_path_buf())
    }

    fn assert(mut self) {
        let mut not_found = BTreeSet::new();
        let mut not_matched = BTreeMap::new();
        for (expected_path, expected_value) in self.expected {
            if let Some(found) = self.found.remove(&expected_path) {
                let expected = expected_value.unwrap_or_default();
                let found = found.unwrap_or_default();
                if let Some(expected) = expected {
                    if expected != found {
                        not_matched.insert(expected_path, (found, expected));
                    }
                }
            } else {
                not_found.insert(expected_path);
            }
        }

        let not_expected: BTreeSet<_> = self.found.into_keys().collect();

        let mut mismatch = false;
        let mut msg = String::new();
        if !not_found.is_empty() {
            mismatch = true;
            writeln!(&mut msg, "\n=== Not found ===").unwrap();
            for not_found in not_found {
                writeln!(&mut msg, "/{}", not_found.display()).unwrap();
            }
        }

        if !not_expected.is_empty() {
            mismatch = true;
            writeln!(&mut msg, "\n=== Not expected ===").unwrap();
            for not_expected in not_expected {
                writeln!(&mut msg, "/{}", not_expected.display()).unwrap();
            }
        }

        if !not_matched.is_empty() {
            mismatch = true;
            writeln!(&mut msg, "\n=== Content matched ===").unwrap();
            for (path, (found, expected)) in not_matched {
                writeln!(&mut msg, "/{}", path.display()).unwrap();
                match (std::str::from_utf8(&found), std::str::from_utf8(&expected)) {
                    (Ok(found), Ok(expected)) => {
                        writeln!(&mut msg, "=== Expected ===\n>>>\n{}\n<<<\n", expected).unwrap();
                        writeln!(&mut msg, "=== Found ===\n>>>\n{}\n<<<\n", found).unwrap();
                    }
                    _ => {
                        writeln!(&mut msg, "Binary data differed").unwrap();
                    }
                }
            }
        }

        if mismatch {
            panic!("{msg}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temp_env_run() {
        TempTestEnv::run(
            |test| {
                test.setup_file_empty("foo/bar/empty.txt")
                    .setup_file_empty("foo/baz/other.txt")
            },
            |root| {
                std::fs::remove_file(root.join("foo/bar/empty.txt")).unwrap();
            },
            |test| {
                test.expect_dir("foo/bar/")
                    .expect_file_empty("foo/baz/other.txt")
            },
        );
    }

    #[test]
    #[should_panic]
    fn test_temp_env_run_panic() {
        TempTestEnv::run(
            |test| {
                test.setup_file_empty("foo/bar/empty.txt")
                    .setup_file_empty("foo/baz/other.txt")
            },
            |root| {
                std::fs::remove_file(root.join("foo/bar/empty.txt")).unwrap();
            },
            |test| test.expect_dir("foo/bar/"),
        );
    }

    #[test]
    fn test_temp_env_run_no_check() {
        TempTestEnv::run_no_check(
            |test| {
                test.setup_file_empty("foo/bar/empty.txt")
                    .setup_file_empty("foo/baz/other.txt")
            },
            |root| {
                std::fs::remove_file(root.join("foo/bar/empty.txt")).unwrap();
            },
        );
    }
}
