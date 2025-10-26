//! Analyzers collect statistics about rendered test output for comparison.
//!
//! This currently only provides a [primitive comparison algorithm][comp] and
//! [diff image rendering][diff].
//!
//! [comp]: compare_page_simple
//! [diff]: render_page_diff

use std::cmp::Ordering;
use std::fmt::Debug;
use std::fmt::Display;

use ecow::EcoVec;
use thiserror::Error;
use tiny_skia::BlendMode;
use tiny_skia::FilterQuality;
use tiny_skia::Pixmap;
use tiny_skia::PixmapPaint;
use tiny_skia::Transform;
use tytanic_utils::fmt::Term;

/// The factor used to convert pixel per pt to pixel per inch.
pub const PPP_TO_PPI_FACTOR: f32 = 72.0;

// NOTE(tinger): This doesn't seem to be quite exactly 2, so we use this to
// ensure we get the same default value as typst-cli, this avoids spurious
// failures when people migrate between the old and new version.

/// The default pixel per pt value used for rendering pages to pixel buffers.
pub const DEFAULT_PIXEL_PER_PT: f32 = 144.0 / PPP_TO_PPI_FACTOR;

/// Converts a pixel-per-pt ratio to a pixel-per-inch ratio.
pub fn ppp_to_ppi(pixel_per_pt: f32) -> f32 {
    pixel_per_pt * PPP_TO_PPI_FACTOR
}

/// Converts a pixel-per-inch ratio to a pixel-per-pt ratio.
pub fn ppi_to_ppp(pixel_per_inch: f32) -> f32 {
    pixel_per_inch / PPP_TO_PPI_FACTOR
}

/// The origin of a documents page, this is used for comparisons of pages with
/// different dimensions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Origin {
    /// The origin of pages on the top left corner, this is the default and used
    /// in left-to-right read documents.
    #[default]
    TopLeft,

    /// The origin of pages on the top right corner, this is used in
    /// left-to-right read documents.
    TopRight,

    /// The origin of pages on the bottom left corner, this is included for
    /// completeness.
    BottomLeft,

    /// The origin of pages on the bottom right corner, this is included for
    /// completeness.
    BottomRight,
}

impl Origin {
    /// Whether this origin is at the left.
    pub fn is_left(&self) -> bool {
        matches!(self, Self::TopLeft | Self::BottomLeft)
    }

    /// Whether this origin is at the right.
    pub fn is_right(&self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
    }

    /// Whether this origin is at the top.
    pub fn is_top(&self) -> bool {
        matches!(self, Self::TopLeft | Self::TopRight)
    }

    /// Whether this origin is at the bottom.
    pub fn is_bottom(&self) -> bool {
        matches!(self, Self::BottomLeft | Self::BottomRight)
    }
}

/// Render the visual diff of two sequences of pages.
///
/// See [`render_page_diff`] for more info.
pub fn render_pages_diff<'a, P, R>(primary: P, reference: R, origin: Origin) -> EcoVec<Pixmap>
where
    R: IntoIterator<Item = &'a Pixmap>,
    P: IntoIterator<Item = &'a Pixmap>,
{
    std::iter::zip(primary, reference)
        .map(|(primary, reference)| render_page_diff(primary, reference, origin))
        .collect()
}

/// Render the visual diff of two pages. If the pages do not have matching
/// dimensions, then the origin is used to align them, regions without overlap
/// will simply be colored black.
///
/// The difference is created by `primary` on top of `refernce` using a difference
/// filter, i.e. `reference` is treated as the expected and `primary` as the
/// altered image.
pub fn render_page_diff(primary: &Pixmap, reference: &Pixmap, origin: Origin) -> Pixmap {
    fn aligned_offset((a, b): (u32, u32), end: bool) -> (i32, i32) {
        match Ord::cmp(&a, &b) {
            Ordering::Less if end => (u32::abs_diff(a, b) as i32, 0),
            Ordering::Greater if end => (0, u32::abs_diff(a, b) as i32),
            _ => (0, 0),
        }
    }

    let mut diff = Pixmap::new(
        Ord::max(reference.width(), primary.width()),
        Ord::max(reference.height(), primary.height()),
    )
    .expect("must be larger than zero");

    let (base_x, change_x) =
        aligned_offset((reference.width(), primary.width()), origin.is_right());
    let (base_y, change_y) =
        aligned_offset((reference.height(), primary.height()), origin.is_right());

    diff.draw_pixmap(
        base_x,
        base_y,
        reference.as_ref(),
        &PixmapPaint {
            opacity: 1.0,
            blend_mode: BlendMode::Source,
            quality: FilterQuality::Nearest,
        },
        Transform::identity(),
        None,
    );

    diff.draw_pixmap(
        change_x,
        change_y,
        primary.as_ref(),
        &PixmapPaint {
            opacity: 1.0,
            blend_mode: BlendMode::Difference,
            quality: FilterQuality::Nearest,
        },
        Transform::identity(),
        None,
    );

    diff
}

/// A struct representing page size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Size {
    /// The width of the page.
    pub width: u32,

    /// The height of the page.
    pub height: u32,
}

impl Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// Compares two sequences of pages page by page using a simple pixel-by-pixel
/// comparison.
///
/// See [`compare_page_simple`] for more info.
pub fn compare_pages_simple<'a, P, R>(
    primary: R,
    reference: P,
    max_delta: u8,
    max_deviations: usize,
) -> Result<(), Error>
where
    R: IntoIterator<Item = &'a Pixmap>,
    P: IntoIterator<Item = &'a Pixmap>,
{
    let mut primary = primary.into_iter();
    let mut reference = reference.into_iter();

    let mut common_count = 0;
    let mut page_errors: Vec<_> = std::iter::zip(primary.by_ref(), reference.by_ref())
        .enumerate()
        .inspect(|_| common_count += 1)
        .filter_map(|(n, (primary, reference))| {
            compare_page_simple(primary, reference, max_delta, max_deviations)
                .err()
                .map(|err| (n, err))
        })
        .collect();

    let primary_more = primary.count();
    let reference_more = reference.count();

    if !page_errors.is_empty() || primary_more != reference_more {
        page_errors.shrink_to_fit();
        return Err(Error {
            primary_page_count: common_count + primary_more,
            reference_page_count: common_count + reference_more,
            page_errors,
        });
    }

    Ok(())
}

/// Compares two pages individually using a simple pixel-by-pixel comparison.
///
/// Each pixel is compared to its reference pixel, if any of their RGBA
/// components differ by more than `max_delta` the pixel is counted towards a
/// deviation counter. If this counter is greater than `max_deviation` then.
pub fn compare_page_simple(
    output: &Pixmap,
    reference: &Pixmap,
    max_delta: u8,
    max_deviations: usize,
) -> Result<(), PageError> {
    if output.width() != reference.width() || output.height() != reference.height() {
        return Err(PageError::Dimensions {
            output: Size {
                width: output.width(),
                height: output.height(),
            },
            reference: Size {
                width: reference.width(),
                height: reference.height(),
            },
        });
    }

    let deviations = tracing::trace_span!("comparing pages with simple strategy")
        .entered()
        .in_scope(|| {
            Iterator::zip(output.pixels().iter(), reference.pixels().iter())
                .filter(|(a, b)| {
                    u8::abs_diff(a.red(), b.red()) > max_delta
                        || u8::abs_diff(a.green(), b.green()) > max_delta
                        || u8::abs_diff(a.blue(), b.blue()) > max_delta
                        || u8::abs_diff(a.alpha(), b.alpha()) > max_delta
                })
                .count()
        });

    if deviations > max_deviations {
        return Err(PageError::Deviations { deviations });
    }

    Ok(())
}

/// Returned by [`compare_pages_simple`].
#[derive(Debug, Clone)]
pub struct Error {
    primary_page_count: usize,
    reference_page_count: usize,

    page_errors: Vec<(usize, PageError)>,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.primary_page_count != self.reference_page_count {
            write!(
                f,
                "page count differed (out {} != ref {})",
                self.primary_page_count, self.reference_page_count,
            )?;
        }

        if self.primary_page_count != self.reference_page_count && self.page_errors.is_empty() {
            write!(f, " and ")?;
        }

        if self.page_errors.is_empty() {
            write!(
                f,
                "{} {} differed at indices: {:?}",
                self.page_errors.len(),
                Term::simple("page").with(self.page_errors.len()),
                self.page_errors.iter().map(|(n, _)| n).collect::<Vec<_>>()
            )?;
        }

        Ok(())
    }
}

impl Error {
    /// The amount of pages in the primary document.
    pub fn primary_page_count(&self) -> usize {
        self.primary_page_count
    }

    /// The amount of pages in the reference document.
    pub fn reference_page_count(&self) -> usize {
        self.reference_page_count
    }

    /// The individual page errors with their associated 0-based page index.
    pub fn page_errors(&self) -> &[(usize, PageError)] {
        &self.page_errors
    }
}

/// Returned by [`compare_page_simple`].
#[derive(Debug, Clone, Error)]
pub enum PageError {
    /// The dimensions of the pages did not match.
    #[error("dimensions differed: out {output} != ref {reference}")]
    Dimensions {
        /// The size of the output page.
        output: Size,

        /// The size of the reference page.
        reference: Size,
    },

    /// The pages differed more than the configured delta and deviation maxima.
    #[error(
        "content differed in at least {} {}",
        deviations,
        Term::simple("pixel").with(*deviations)
    )]
    Deviations {
        /// The amount of visual deviations, i.e. the amount of pixels which did
        /// not match according to the maximum delta and deviation thresholds.
        deviations: usize,
    },
}

#[cfg(test)]
mod tests {
    use tiny_skia::PremultipliedColorU8;

    use super::*;

    fn images() -> [Pixmap; 2] {
        let a = Pixmap::new(10, 1).unwrap();
        let mut b = Pixmap::new(10, 1).unwrap();

        let red = PremultipliedColorU8::from_rgba(128, 0, 0, 128).unwrap();
        b.pixels_mut()[0] = red;
        b.pixels_mut()[1] = red;
        b.pixels_mut()[2] = red;
        b.pixels_mut()[3] = red;

        [a, b]
    }

    #[test]
    fn test_page_simple_below_max_delta() {
        let [a, b] = images();
        assert!(compare_page_simple(&a, &b, 128, 0).is_ok())
    }

    #[test]
    fn test_page_simple_below_max_devitation() {
        let [a, b] = images();
        assert!(compare_page_simple(&a, &b, 0, 5).is_ok());
    }

    #[test]
    fn test_page_simple_above_max_devitation() {
        let [a, b] = images();
        assert!(matches!(
            compare_page_simple(&a, &b, 0, 0),
            Err(PageError::Deviations { deviations: 4 })
        ))
    }

    #[test]
    fn test_page_diff_top_left() {
        let mut base = Pixmap::new(10, 10).unwrap();
        let mut change = Pixmap::new(15, 5).unwrap();
        let mut diff = Pixmap::new(15, 10).unwrap();

        base.fill(tiny_skia::Color::from_rgba8(255, 255, 255, 255));
        change.fill(tiny_skia::Color::from_rgba8(255, 0, 0, 255));

        let is_in = |x, y, pixmap: &Pixmap| x < pixmap.width() && y < pixmap.height();

        for y in 0..10 {
            for x in 0..15 {
                let idx = diff.width().checked_mul(y).unwrap().checked_add(x).unwrap();
                let px = diff.pixels_mut().get_mut(idx as usize).unwrap();

                // NOTE(tinger): Despite some of these being invalid according
                // to PremultipliedColorU8::new, this is indeed what is
                // internally created when inverting.
                //
                // That's not surprising, but not allowing us to create those
                // pixels when they're valid is.
                *px = bytemuck::cast(match (is_in(x, y, &base), is_in(x, y, &change)) {
                    // Proper difference where both are in bounds.
                    (true, true) => [0u8, 255, 255, 255],
                    // No difference to base where change is out of bounds.
                    (true, false) => [255, 255, 255, 255],
                    // No difference to change where base is out of bounds.
                    (false, true) => [255, 0, 0, 255],
                    // Dead area from size mismatch.
                    (false, false) => [0, 0, 0, 0],
                });
            }
        }

        assert_eq!(
            render_page_diff(&base, &change, Origin::TopLeft).data(),
            diff.data()
        );
    }

    #[test]
    fn test_page_diff_bottom_right() {
        let mut base = Pixmap::new(10, 10).unwrap();
        let mut change = Pixmap::new(15, 5).unwrap();
        let mut diff = Pixmap::new(15, 10).unwrap();

        base.fill(tiny_skia::Color::from_rgba8(255, 255, 255, 255));
        change.fill(tiny_skia::Color::from_rgba8(255, 0, 0, 255));

        // similar as above, but mirrored across both axes
        let is_in =
            |x, y, pixmap: &Pixmap| (15 - x) <= pixmap.width() && (10 - y) <= pixmap.height();

        for y in 0..10 {
            for x in 0..15 {
                let idx = diff.width().checked_mul(y).unwrap().checked_add(x).unwrap();
                let px = diff.pixels_mut().get_mut(idx as usize).unwrap();

                // NOTE(tinger): Despite some of these being invalid according
                // to PremultipliedColorU8::new, this is indeed what is
                // internally created when inverting.
                //
                // That's not surprising, but not allowing us to create those
                // pixels when they're valid is.
                *px = bytemuck::cast(match (is_in(x, y, &base), is_in(x, y, &change)) {
                    // Proper difference where both are in bounds.
                    (true, true) => [0u8, 255, 255, 255],
                    // No difference to base where change is out of bounds.
                    (true, false) => [255, 255, 255, 255],
                    // No difference to change where base is out of bounds.
                    (false, true) => [255, 0, 0, 255],
                    // Dead area from size mismatch.
                    (false, false) => [0, 0, 0, 0],
                });
            }
        }

        assert_eq!(
            render_page_diff(&base, &change, Origin::BottomRight).data(),
            diff.data()
        );
    }
}
