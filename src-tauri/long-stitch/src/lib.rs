use image::RgbaImage;

const MIN_APPEND_ROWS: u32 = 1;
const MAX_OUTPUT_HEIGHT: u32 = 30_000;
const MAX_OUTPUT_PIXELS: u64 = 24_000_000;
// Screen captures across a scroll can differ slightly due to compositor
// antialiasing and subpixel text rasterization. A tighter threshold drops
// valid overlaps and leaves the final image incomplete.
const MATCH_THRESHOLD: f64 = 12.0;
// Scores this close belong to the same edge (sub-pixel) or a real tie.
// A uniquely better score is accepted even when the page jumped most of a
// viewport; tied scores prefer the smallest movement so a repeating pattern
// cannot splice in a huge duplicate or a gap.
const MATCH_MARGIN: f64 = 0.75;

#[derive(Debug, PartialEq)]
pub enum AppendResult {
    Appended(u32),
    Unchanged,
    NoMatch,
    Ambiguous,
    Limit,
}

pub struct StitchedImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl StitchedImage {
    pub fn from_frame(frame: &RgbaImage) -> Result<Self, String> {
        let (width, height) = frame.dimensions();
        if width == 0
            || height == 0
            || height > MAX_OUTPUT_HEIGHT
            || u64::from(width) * u64::from(height) > MAX_OUTPUT_PIXELS
        {
            return Err("selected region is too large".into());
        }
        Ok(Self {
            width,
            height,
            pixels: frame.as_raw().clone(),
        })
    }

    fn can_append(&self, rows: u32) -> bool {
        let height = self.height.saturating_add(rows);
        height <= MAX_OUTPUT_HEIGHT
            && u64::from(self.width) * u64::from(height) <= MAX_OUTPUT_PIXELS
    }

    fn append(&mut self, frame: &RgbaImage, new_rows: u32) {
        let row_bytes = self.width as usize * 4;
        let start_row = frame.height().saturating_sub(new_rows);
        let start = start_row as usize * row_bytes;
        self.pixels.extend_from_slice(&frame.as_raw()[start..]);
        self.height += new_rows;
    }
}

/// Smallest band of rows that can still prove two frames show the same page.
/// Fast flicks move most of a viewport between captures; requiring half the
/// frame made those captures fail and left a hole in the stitch.
fn min_overlap_rows(height: u32) -> u32 {
    if height <= 1 {
        return 1;
    }
    (height / 10).max(12).min(height - 1)
}

/// Average grayscale difference of one vertical placement of `frame` over
/// `output`. `align` is the output row where the frame's top row lands and
/// may be negative when the frame extends above the stitched image.
fn alignment_score(
    output: &[u8],
    output_height: u32,
    frame: &[u8],
    width: u32,
    frame_height: u32,
    align: i32,
    dense: bool,
) -> f64 {
    let out_h = output_height as i32;
    let frame_h = frame_height as i32;
    let overlap_top = align.max(0);
    let overlap_bottom = (align + frame_h).min(out_h);
    if width == 0 || overlap_bottom <= overlap_top {
        return f64::MAX;
    }
    let overlap = (overlap_bottom - overlap_top) as usize;
    let frame_top = (overlap_top - align) as usize;
    let row_bytes = width as usize * 4;
    let margin = if width > 8 { width / 25 } else { 0 };
    let usable_start = margin.min(width - 1) as usize;
    let usable_end = width.saturating_sub(margin).max(margin + 1) as usize;
    let usable = usable_end.saturating_sub(usable_start).max(1);
    let cap = if dense { 96 } else { 64 };
    let cols = usable.min(cap).max(1);
    let rows = overlap.min(cap).max(1);
    let mut total = 0u64;
    let mut count = 0u64;
    // Abandon a placement once it is already worse than anything we can accept.
    let fail_at = ((MATCH_THRESHOLD + MATCH_MARGIN) * (rows * cols) as f64) as u64 + 1;

    for row_index in 0..rows {
        let sample_row = if rows == 1 {
            0
        } else {
            row_index * (overlap - 1) / (rows - 1)
        };
        let out_off = (overlap_top as usize + sample_row) * row_bytes;
        let frame_off = (frame_top + sample_row) * row_bytes;
        for col_index in 0..cols {
            let x = if cols == 1 {
                usable_start
            } else {
                usable_start + col_index * (usable - 1) / (cols - 1)
            };
            let offset = x * 4;
            let left = &output[out_off + offset..out_off + offset + 3];
            let right = &frame[frame_off + offset..frame_off + offset + 3];
            let left_luma = u16::from(left[0]) + u16::from(left[1]) + u16::from(left[2]);
            let right_luma = u16::from(right[0]) + u16::from(right[1]) + u16::from(right[2]);
            total += u64::from(left_luma.abs_diff(right_luma) / 3);
            count += 1;
        }
        if !dense && total > fail_at {
            return MATCH_THRESHOLD + MATCH_MARGIN + 1.0;
        }
    }
    if count == 0 {
        f64::MAX
    } else {
        total as f64 / count as f64
    }
}

fn prepend_rows(output: &mut StitchedImage, frame: &RgbaImage, rows: u32) {
    let row_bytes = output.width as usize * 4;
    let prefix_len = rows as usize * row_bytes;
    let mut pixels = Vec::with_capacity(output.pixels.len() + prefix_len);
    pixels.extend_from_slice(&frame.as_raw()[..prefix_len]);
    pixels.extend_from_slice(&output.pixels);
    output.pixels = pixels;
    output.height += rows;
}

pub fn integrate(output: &mut StitchedImage, frame: &RgbaImage) -> AppendResult {
    let frame_height = frame.height();
    let frame_width = frame.width();
    if frame_width == 0
        || frame_height == 0
        || output.width != frame_width
        || output.height < frame_height
    {
        return AppendResult::NoMatch;
    }
    let row_bytes = output.width as usize * 4;
    let tail = (output.height - frame_height) as usize * row_bytes;
    if output.pixels[tail..] == *frame.as_raw() {
        return AppendResult::Unchanged;
    }

    let min_overlap = min_overlap_rows(frame_height) as i32;
    let height = frame_height as i32;
    let stitched = output.height as i32;
    // A couple of viewports above the tail covers scrolling back through
    // content that is already in the image. Negative alignments prepend
    // rows that were above the original top.
    let align_min = (stitched - height - height.saturating_mul(3)).max(min_overlap - height);
    let align_max = stitched - min_overlap;
    if align_min > align_max {
        return AppendResult::NoMatch;
    }

    let frame_bytes = frame.as_raw();
    let mut best_score = f64::MAX;
    let mut matches: Vec<(i32, f64)> = Vec::new();
    for align in align_min..=align_max {
        let score = alignment_score(
            &output.pixels,
            output.height,
            frame_bytes,
            frame_width,
            frame_height,
            align,
            false,
        );
        if score <= MATCH_THRESHOLD {
            matches.push((align, score));
            if score < best_score {
                best_score = score;
            }
        }
    }
    if matches.is_empty() || best_score > MATCH_THRESHOLD {
        return AppendResult::NoMatch;
    }

    let anchor = stitched - height;
    let align = matches
        .iter()
        .filter(|(_, score)| *score <= best_score + MATCH_MARGIN)
        .min_by(|left, right| {
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| (left.0 - anchor).abs().cmp(&(right.0 - anchor).abs()))
        })
        .map(|(align, _)| *align)
        .unwrap_or(anchor);

    let confirmed = alignment_score(
        &output.pixels,
        output.height,
        frame_bytes,
        frame_width,
        frame_height,
        align,
        true,
    );
    if confirmed > MATCH_THRESHOLD {
        return AppendResult::NoMatch;
    }

    let frame_bottom = align + height;
    if align >= 0 && frame_bottom <= stitched {
        // Scrolling back (or not at all) lands on pixels already stitched.
        return AppendResult::Unchanged;
    }
    if align >= 0 && align < stitched && frame_bottom > stitched {
        let new_rows = (frame_bottom - stitched) as u32;
        if new_rows < MIN_APPEND_ROWS {
            return AppendResult::Unchanged;
        }
        if !output.can_append(new_rows) {
            return AppendResult::Limit;
        }
        output.append(frame, new_rows);
        return AppendResult::Appended(new_rows);
    }
    if align < 0 && frame_bottom > 0 {
        let new_rows = (-align) as u32;
        if new_rows < MIN_APPEND_ROWS {
            return AppendResult::Unchanged;
        }
        if !output.can_append(new_rows) {
            return AppendResult::Limit;
        }
        prepend_rows(output, frame, new_rows);
        return AppendResult::Appended(new_rows);
    }
    AppendResult::NoMatch
}

#[cfg(test)]
mod tests {
    use super::{integrate, AppendResult, StitchedImage};
    use image::{Rgba, RgbaImage};

    fn test_frame(width: u32, height: u32, offset: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            let absolute_y = y + offset;
            Rgba([
                ((absolute_y * 37 + x * 11) % 251) as u8,
                ((absolute_y * 73 + x * 19) % 241) as u8,
                ((absolute_y * 13 + x * 43) % 239) as u8,
                255,
            ])
        })
    }

    fn stitch_one(previous: &RgbaImage, next: &RgbaImage) -> AppendResult {
        let mut output = StitchedImage::from_frame(previous).unwrap();
        integrate(&mut output, next)
    }

    #[test]
    fn overlapping_frames_append_exact_new_rows() {
        let previous = test_frame(160, 120, 0);
        let next = test_frame(160, 120, 42);
        assert_eq!(stitch_one(&previous, &next), AppendResult::Appended(42));

        let mut output = StitchedImage::from_frame(&previous).unwrap();
        integrate(&mut output, &next);
        assert_eq!(output.height, 162);
        let expected = test_frame(160, 162, 0);
        assert_eq!(&output.pixels, expected.as_raw());
    }

    #[test]
    fn unchanged_and_unrelated_frames_do_not_append() {
        let previous = test_frame(160, 120, 0);
        assert_eq!(stitch_one(&previous, &previous), AppendResult::Unchanged);
        let unrelated = RgbaImage::from_fn(160, 120, |x, y| {
            Rgba([(x * 3) as u8, (y * 5) as u8, ((x + y) * 7) as u8, 255])
        });
        assert_eq!(stitch_one(&previous, &unrelated), AppendResult::NoMatch);
    }

    #[test]
    fn fast_scroll_with_small_overlap_appends_exact_rows() {
        let previous = test_frame(180, 200, 0);
        // 176 new rows, 24 rows of overlap — under the old half-frame minimum.
        let next = test_frame(180, 200, 176);
        assert_eq!(stitch_one(&previous, &next), AppendResult::Appended(176));
        let mut output = StitchedImage::from_frame(&previous).unwrap();
        integrate(&mut output, &next);
        assert_eq!(output.height, 376);
        assert_eq!(&output.pixels, test_frame(180, 376, 0).as_raw());
    }

    #[test]
    fn scrolling_back_slightly_does_not_duplicate_or_drop_rows() {
        let mut output = StitchedImage::from_frame(&test_frame(140, 160, 0)).unwrap();
        integrate(&mut output, &test_frame(140, 160, 90));
        assert_eq!(output.height, 250);
        let stitched = output.pixels.clone();
        // The tail was rows 90..250. This frame is rows 60..220, already inside.
        let scrolled_back = test_frame(140, 160, 60);
        assert_eq!(
            integrate(&mut output, &scrolled_back),
            AppendResult::Unchanged
        );
        assert_eq!(output.height, 250);
        assert_eq!(output.pixels, stitched);
        assert_eq!(&output.pixels, test_frame(140, 250, 0).as_raw());
    }

    #[test]
    fn scrolling_above_the_start_prepends_without_a_gap() {
        let mut output = StitchedImage::from_frame(&test_frame(120, 100, 40)).unwrap();
        let revealed = test_frame(120, 100, 10);
        assert_eq!(
            integrate(&mut output, &revealed),
            AppendResult::Appended(30)
        );
        assert_eq!(output.height, 130);
        assert_eq!(&output.pixels, test_frame(120, 130, 10).as_raw());
    }

    #[test]
    fn successive_frames_including_a_backstep_match_the_source_strip() {
        let width = 96;
        let view = 80;
        let mut output = StitchedImage::from_frame(&test_frame(width, view, 0)).unwrap();
        let steps = [50u32, 110, 70, 150, 190];
        for offset in steps {
            let frame = test_frame(width, view, offset);
            integrate(&mut output, &frame);
        }
        let expected_height = 190 + view;
        assert_eq!(output.height, expected_height);
        assert_eq!(
            &output.pixels,
            test_frame(width, expected_height, 0).as_raw()
        );
    }

    #[test]
    fn a_jump_with_no_overlap_leaves_the_image_unchanged() {
        let mut output = StitchedImage::from_frame(&test_frame(80, 100, 0)).unwrap();
        let before = output.pixels.clone();
        let jumped = test_frame(80, 100, 100 + 40);
        assert_eq!(integrate(&mut output, &jumped), AppendResult::NoMatch);
        assert_eq!(output.pixels, before);
    }

    #[test]
    fn small_capture_noise_still_finds_the_same_offset() {
        let previous = test_frame(150, 140, 0);
        let mut next = test_frame(150, 140, 55);
        for pixel in next.chunks_mut(4) {
            pixel[0] = pixel[0].saturating_add(6);
            pixel[1] = pixel[1].saturating_add(3);
            pixel[2] = pixel[2].saturating_sub(2);
        }
        assert_eq!(stitch_one(&previous, &next), AppendResult::Appended(55));
    }
}
