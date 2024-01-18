use std::fmt::{Debug, Display};
use std::io;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use image::{ImageResult, RgbImage};

use super::{
    CleanupFailure, CompareFailure, ComparePageFailure, CompileFailure, PrepareFailure, Stage,
    Test, TestFailure, TestResult,
};
use crate::project::Project;
use crate::util;

#[derive(Debug)]
pub struct Context<'p> {
    project: &'p Project,
    typst: PathBuf,
    fail_fast: bool,
}

#[derive(Debug)]
pub struct TestContext<'c, 'p, 't> {
    project_context: &'c Context<'p>,
    _test: &'t Test,
    test_file: PathBuf,
    out_dir: PathBuf,
    ref_dir: PathBuf,
    diff_dir: PathBuf,
}

impl<'p> Context<'p> {
    pub fn new(project: &'p Project, typst: PathBuf, fail_fast: bool) -> Self {
        Self {
            project,
            typst,
            fail_fast,
        }
    }

    pub fn test<'c, 't>(&'c self, test: &'t Test) -> TestContext<'c, 'p, 't> {
        let out_dir = self.project.out_dir(&test);
        let ref_dir = self.project.ref_dir(&test);
        let diff_dir = self.project.diff_dir(&test);
        let test_file = self.project.test_file(&test);

        tracing::trace!(test = ?test.name(), "establishing test context");
        TestContext {
            project_context: self,
            _test: test,
            test_file,
            out_dir,
            ref_dir,
            diff_dir,
        }
    }

    #[tracing::instrument(skip(self))]
    pub fn prepare(&self) -> Result<(), Error> {
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn cleanup(&self) -> Result<(), Error> {
        Ok(())
    }
}

impl TestContext<'_, '_, '_> {
    #[tracing::instrument(skip(self))]
    pub fn run(&self, compare: bool) -> ContextResult<TestFailure> {
        macro_rules! bail_inner {
            ($err:expr) => {
                let err: TestFailure = $err.into();
                return Ok(Err(err));
            };
        }

        if let Err(err) = self.prepare()? {
            bail_inner!(err);
        }

        if let Err(err) = self.compile()? {
            bail_inner!(err);
        }

        if compare {
            if let Err(err) = self.compare()? {
                bail_inner!(err);
            }
        }

        if let Err(err) = self.cleanup()? {
            bail_inner!(err);
        }

        Ok(Ok(()))
    }

    #[tracing::instrument(skip_all)]
    pub fn prepare(&self) -> ContextResult<PrepareFailure> {
        let dirs = [
            ("out", true, &self.out_dir),
            ("ref", false, &self.ref_dir),
            ("diff", true, &self.diff_dir),
        ];

        for (name, clear, path) in dirs {
            if clear {
                tracing::trace!(?path, "clearing {name} dir");
                util::fs::create_empty_dir(path, false).map_err(|e| {
                    Error::io(e)
                        .at(Stage::Preparation)
                        .context(format!("clearing {} dir: {:?}", name, path))
                })?;
            } else {
                tracing::trace!(?path, "creating {name} dir");
                util::fs::create_dir(path, false).map_err(|e| {
                    Error::io(e)
                        .at(Stage::Preparation)
                        .context(format!("creating {} dir: {:?}", name, path))
                })?;
            }
        }

        Ok(Ok(()))
    }

    #[tracing::instrument(skip_all)]
    pub fn cleanup(&self) -> ContextResult<CleanupFailure> {
        Ok(Ok(()))
    }

    #[tracing::instrument(skip_all)]
    pub fn compile(&self) -> ContextResult<CompileFailure> {
        let mut typst = Command::new(&self.project_context.typst);
        typst.args(["compile", "--root"]);
        typst.arg(self.project_context.project.root());
        typst.arg(&self.test_file);
        typst.arg(self.out_dir.join("{n}").with_extension("png"));

        tracing::trace!(args = ?[&typst], "running typst");
        let output = typst.output().map_err(|e| {
            match e.kind() {
                ErrorKind::NotFound => Error::missing_typst(),
                _ => Error::io(e),
            }
            .at(Stage::Compilation)
            .context("executing typst")
        })?;

        if !output.status.success() {
            return Ok(Err(CompileFailure {
                args: typst.get_args().map(ToOwned::to_owned).collect(),
                output,
            }));
        }

        Ok(Ok(()))
    }

    #[tracing::instrument(skip_all)]
    pub fn compare(&self) -> ContextResult<CompareFailure> {
        let err_fn = |n, e, p| {
            Error::io(e)
                .at(Stage::Comparison)
                .context(format!("reading {} dir: {:?}", n, p))
        };

        tracing::trace!(path = ?self.out_dir, "reading out dir");
        let mut out_entries = util::fs::collect_dir_entries(&self.out_dir)
            .map_err(|e| err_fn("out", e, &self.out_dir))?;

        tracing::trace!(path = ?self.ref_dir, "reading ref dir");
        let mut ref_entries = util::fs::collect_dir_entries(&self.ref_dir)
            .map_err(|e| err_fn("ref", e, &self.ref_dir))?;

        if out_entries.is_empty() {
            return Ok(Err(CompareFailure::MissingOutput));
        }

        if ref_entries.is_empty() {
            return Ok(Err(CompareFailure::MissingReferences));
        }

        out_entries.sort_by_key(|t| t.file_name());
        ref_entries.sort_by_key(|t| t.file_name());

        if out_entries.len() != ref_entries.len() {
            return Ok(Err(CompareFailure::PageCount {
                output: out_entries.len(),
                reference: ref_entries.len(),
            }));
        }

        let mut pages = vec![];

        for (idx, (out_entry, ref_entry)) in out_entries.into_iter().zip(ref_entries).enumerate() {
            let p = idx + 1;
            if let Err(err) = self.compare_page(p, &out_entry.path(), &ref_entry.path())? {
                pages.push((p, err));
                if self.project_context.fail_fast {
                    return Ok(Err(CompareFailure::Page { pages }));
                }
            }
        }

        if !pages.is_empty() {
            return Ok(Err(CompareFailure::Page { pages }));
        }

        Ok(Ok(()))
    }

    #[tracing::instrument(skip_all, fields(page = ?page_number))]
    pub fn compare_page(
        &self,
        page_number: usize,
        out_file: &Path,
        ref_file: &Path,
    ) -> ContextResult<ComparePageFailure> {
        let err_fn = |n, e, f| {
            Error::image(e)
                .at(Stage::Comparison)
                .context(format!("reading {n} image: {f:?}"))
        };

        tracing::trace!(path = ?out_file, "reading out file");
        let out_image = image::open(out_file)
            .map_err(|e| err_fn("out", e, out_file))?
            .into_rgb8();

        tracing::trace!(path = ?ref_file, "reading ref file");
        let ref_image = image::open(ref_file)
            .map_err(|e| err_fn("ref", e, ref_file))?
            .into_rgb8();

        if out_image.dimensions() != ref_image.dimensions() {
            return Ok(Err(ComparePageFailure::Dimensions {
                output: out_image.dimensions(),
                reference: ref_image.dimensions(),
            }));
        }

        for (out_px, ref_px) in out_image.pixels().zip(ref_image.pixels()) {
            if out_px != ref_px {
                self.save_diff_page(page_number, &out_image, &ref_image)
                    .map_err(|e| Error::image(e).at(Stage::Comparison))?;
                return Ok(Err(ComparePageFailure::Content));
            }
        }

        Ok(Ok(()))
    }

    #[tracing::instrument(skip_all, fields(page = ?page_number))]
    pub fn save_diff_page(
        &self,
        page_number: usize,
        out_image: &RgbImage,
        ref_image: &RgbImage,
    ) -> ImageResult<()> {
        let mut diff_image = out_image.clone();

        for (out_px, ref_px) in diff_image.pixels_mut().zip(ref_image.pixels()) {
            out_px.0[0] = u8::abs_diff(out_px.0[0], ref_px.0[0]);
            out_px.0[1] = u8::abs_diff(out_px.0[1], ref_px.0[1]);
            out_px.0[2] = u8::abs_diff(out_px.0[2], ref_px.0[2]);
        }

        let path = self
            .diff_dir
            .join(page_number.to_string())
            .with_extension("png");

        tracing::debug!(?path, "saving diff image");
        diff_image.save(path)?;

        Ok(())
    }
}

pub type ContextResult<E = TestFailure> = Result<TestResult<E>, Error>;

#[derive(Debug)]
enum ErrorImpl {
    Io(io::Error),
    Image(image::ImageError),
    MissingTypst,
}

pub struct Error {
    inner: ErrorImpl,
    context: Option<String>,
    stage: Option<Stage>,
}

impl Error {
    fn io(error: io::Error) -> Self {
        Self {
            inner: ErrorImpl::Io(error),
            context: None,
            stage: None,
        }
    }

    fn image(error: image::ImageError) -> Self {
        Self {
            inner: ErrorImpl::Image(error),
            context: None,
            stage: None,
        }
    }

    fn missing_typst() -> Self {
        Self {
            inner: ErrorImpl::MissingTypst,
            context: None,
            stage: None,
        }
    }

    fn at(mut self, stage: Stage) -> Self {
        self.stage = Some(stage);
        self
    }

    fn context<S: Into<String>>(mut self, context: S) -> Self {
        self.context = Some(context.into());
        self
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(stage) = &self.stage {
            write!(f, "{} stage failed", stage)?;
        } else {
            write!(f, "failed")?;
        }

        if let Some(ctx) = &self.context {
            write!(f, " while {ctx}")?;
        }

        if matches!(self.inner, ErrorImpl::MissingTypst) {
            write!(f, ": typst could not be run. Please make sure a valid 'typst' executable is in your PATH, or specify its path through the '--typst' option to this command.")?;
        }

        Ok(())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match &self.inner {
            ErrorImpl::Io(e) => e,
            ErrorImpl::Image(e) => e,
            ErrorImpl::MissingTypst => return None,
        })
    }
}
