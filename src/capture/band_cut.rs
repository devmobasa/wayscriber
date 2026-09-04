//! Sequential row/column band cuts on tightly packed ARGB32 rasters.
//!
//! Cuts are expressed in the coordinate system of the raster produced by every
//! preceding cut. The engine is backend-neutral: it does not know about Wayland,
//! Cairo, source tokens, or capture destinations.

use crate::screen_pixels::PackedArgb32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CutAxis {
    Columns,
    Rows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CutBand {
    axis: CutAxis,
    start: u32,
    end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BandCutError {
    EmptyRange,
    OutOfBounds,
    EmptyOutput,
    SizeOverflow,
    InvalidPixels,
}

impl std::fmt::Display for BandCutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRange => formatter.write_str("cut range is empty"),
            Self::OutOfBounds => formatter.write_str("cut range leaves the current output"),
            Self::EmptyOutput => formatter.write_str("cut would remove the entire remaining axis"),
            Self::SizeOverflow => formatter.write_str("cut output size overflows"),
            Self::InvalidPixels => formatter.write_str("cut pixels are invalid"),
        }
    }
}

impl CutBand {
    pub(crate) fn new(axis: CutAxis, start: u32, end: u32) -> Result<Self, BandCutError> {
        if start >= end {
            return Err(BandCutError::EmptyRange);
        }
        Ok(Self { axis, start, end })
    }

    /// Normalize a pair of pixel edges so backward drags match forward drags.
    pub(crate) fn from_unordered_edges(
        axis: CutAxis,
        first: u32,
        second: u32,
    ) -> Result<Self, BandCutError> {
        Self::new(axis, first.min(second), first.max(second))
    }

    pub(crate) const fn axis(self) -> CutAxis {
        self.axis
    }

    pub(crate) const fn start(self) -> u32 {
        self.start
    }

    pub(crate) const fn end(self) -> u32 {
        self.end
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AxisSpan {
    start: u32,
    end: u32,
}

impl AxisSpan {
    fn len(self) -> Result<u32, BandCutError> {
        self.end
            .checked_sub(self.start)
            .ok_or(BandCutError::SizeOverflow)
    }
}

fn initial_spans(extent: u32) -> Vec<AxisSpan> {
    if extent == 0 {
        Vec::new()
    } else {
        vec![AxisSpan {
            start: 0,
            end: extent,
        }]
    }
}

fn axis_extent(spans: &[AxisSpan]) -> Result<u32, BandCutError> {
    spans.iter().try_fold(0u32, |acc, span| {
        acc.checked_add(span.len()?)
            .ok_or(BandCutError::SizeOverflow)
    })
}

fn tight_row(width: u32) -> Result<(i32, usize), BandCutError> {
    let row_bytes_u32 = width.checked_mul(4).ok_or(BandCutError::SizeOverflow)?;
    let stride = i32::try_from(row_bytes_u32).map_err(|_| BandCutError::SizeOverflow)?;
    let row_bytes = usize::try_from(row_bytes_u32).map_err(|_| BandCutError::SizeOverflow)?;
    Ok((stride, row_bytes))
}

fn checked_buffer_len(row_bytes: usize, height: u32) -> Result<usize, BandCutError> {
    usize::try_from(height)
        .ok()
        .and_then(|height| row_bytes.checked_mul(height))
        .ok_or(BandCutError::SizeOverflow)
}

fn checked_row_offset(row: u32, stride: usize) -> Result<usize, BandCutError> {
    usize::try_from(row)
        .ok()
        .and_then(|row| row.checked_mul(stride))
        .ok_or(BandCutError::SizeOverflow)
}

fn checked_pixel_bytes(offset: u32) -> Result<usize, BandCutError> {
    usize::try_from(offset)
        .ok()
        .and_then(|offset| offset.checked_mul(4))
        .ok_or(BandCutError::SizeOverflow)
}

fn remove_logical_range(
    spans: &mut Vec<AxisSpan>,
    start: u32,
    end: u32,
) -> Result<(), BandCutError> {
    if start >= end {
        return Err(BandCutError::EmptyRange);
    }
    let extent = axis_extent(spans)?;
    if end > extent {
        return Err(BandCutError::OutOfBounds);
    }

    let mut kept = Vec::new();
    let mut logical = 0u32;
    for span in spans.iter().copied() {
        let span_len = span.len()?;
        let logical_end = logical
            .checked_add(span_len)
            .ok_or(BandCutError::SizeOverflow)?;
        if start > logical {
            let keep_end = start.min(logical_end);
            if keep_end > logical {
                let keep_len = keep_end - logical;
                kept.push(AxisSpan {
                    start: span.start,
                    end: span
                        .start
                        .checked_add(keep_len)
                        .ok_or(BandCutError::SizeOverflow)?,
                });
            }
        }
        if end < logical_end {
            let keep_start = end.max(logical);
            if keep_start < logical_end {
                let skip = keep_start - logical;
                kept.push(AxisSpan {
                    start: span
                        .start
                        .checked_add(skip)
                        .ok_or(BandCutError::SizeOverflow)?,
                    end: span.end,
                });
            }
        }
        logical = logical_end;
    }
    if axis_extent(&kept)? == 0 {
        return Err(BandCutError::EmptyOutput);
    }
    *spans = kept;
    Ok(())
}

fn retained_axes(
    source: (u32, u32),
    cuts: &[CutBand],
) -> Result<(Vec<AxisSpan>, Vec<AxisSpan>), BandCutError> {
    let mut columns = initial_spans(source.0);
    let mut rows = initial_spans(source.1);
    for cut in cuts {
        match cut.axis {
            CutAxis::Columns => remove_logical_range(&mut columns, cut.start, cut.end)?,
            CutAxis::Rows => remove_logical_range(&mut rows, cut.start, cut.end)?,
        }
    }
    Ok((columns, rows))
}

/// Replay `cuts` against `source` and return the remaining native size.
pub(crate) fn output_size(
    source: (u32, u32),
    cuts: &[CutBand],
) -> Result<(u32, u32), BandCutError> {
    let (columns, rows) = retained_axes(source, cuts)?;
    let width = axis_extent(&columns)?;
    let height = axis_extent(&rows)?;
    if width == 0 || height == 0 {
        return Err(BandCutError::EmptyOutput);
    }
    Ok((width, height))
}

fn copy_retained(
    source: &PackedArgb32,
    columns: &[AxisSpan],
    rows: &[AxisSpan],
) -> Result<PackedArgb32, BandCutError> {
    let output_width = axis_extent(columns)?;
    let output_height = axis_extent(rows)?;
    if output_width == 0 || output_height == 0 {
        return Err(BandCutError::EmptyOutput);
    }
    let (stride, row_bytes) = tight_row(output_width)?;
    let length = checked_buffer_len(row_bytes, output_height)?;
    let source_stride = usize::try_from(source.stride()).map_err(|_| BandCutError::SizeOverflow)?;
    let source_data = source.data();
    let mut output = vec![0u8; length];
    let mut dest_row = 0usize;
    for row_span in rows {
        let mut source_y = row_span.start;
        while source_y < row_span.end {
            let source_row = checked_row_offset(source_y, source_stride)?;
            let dest_row_offset = dest_row
                .checked_mul(row_bytes)
                .ok_or(BandCutError::SizeOverflow)?;
            let mut dest_bytes = 0usize;
            for column_span in columns {
                let span_bytes = checked_pixel_bytes(column_span.len()?)?;
                let source_start = source_row
                    .checked_add(checked_pixel_bytes(column_span.start)?)
                    .ok_or(BandCutError::SizeOverflow)?;
                let source_end = source_start
                    .checked_add(span_bytes)
                    .ok_or(BandCutError::SizeOverflow)?;
                let dest_start = dest_row_offset
                    .checked_add(dest_bytes)
                    .ok_or(BandCutError::SizeOverflow)?;
                let dest_end = dest_start
                    .checked_add(span_bytes)
                    .ok_or(BandCutError::SizeOverflow)?;
                let source_run = source_data
                    .get(source_start..source_end)
                    .ok_or(BandCutError::InvalidPixels)?;
                let dest_run = output
                    .get_mut(dest_start..dest_end)
                    .ok_or(BandCutError::SizeOverflow)?;
                dest_run.copy_from_slice(source_run);
                dest_bytes = dest_bytes
                    .checked_add(span_bytes)
                    .ok_or(BandCutError::SizeOverflow)?;
            }
            dest_row = dest_row.checked_add(1).ok_or(BandCutError::SizeOverflow)?;
            source_y = source_y.checked_add(1).ok_or(BandCutError::SizeOverflow)?;
        }
    }
    PackedArgb32::new(output_width, output_height, stride, output)
        .ok_or(BandCutError::InvalidPixels)
}

/// Apply ordered band cuts to a tightly packed raster, allocating one output.
pub(crate) fn apply_band_cuts(
    source: &PackedArgb32,
    cuts: &[CutBand],
) -> Result<PackedArgb32, BandCutError> {
    if cuts.is_empty() {
        return Ok(source.clone());
    }
    let (columns, rows) = retained_axes((source.width(), source.height()), cuts)?;
    copy_retained(source, &columns, &rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(bytes: [u8; 4]) -> u32 {
        u32::from_ne_bytes(bytes)
    }

    fn unique_raster(width: u32, height: u32) -> PackedArgb32 {
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                data.extend_from_slice(&[
                    x as u8,
                    y as u8,
                    (y.wrapping_mul(width).wrapping_add(x)) as u8,
                    0x80 | ((x + y) as u8 & 0x3F),
                ]);
            }
        }
        PackedArgb32::new(width, height, (width * 4) as i32, data).expect("unique raster")
    }

    fn raster_from_ids(width: u32, height: u32, ids: &[u32]) -> PackedArgb32 {
        assert_eq!(ids.len(), (width * height) as usize);
        let data = ids
            .iter()
            .flat_map(|id| id.to_ne_bytes())
            .collect::<Vec<_>>();
        PackedArgb32::new(width, height, (width * 4) as i32, data).expect("id raster")
    }

    fn ids_of(pixels: &PackedArgb32) -> Vec<u32> {
        pixels
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| u32::from_ne_bytes(*chunk))
            .collect()
    }

    fn cut(axis: CutAxis, start: u32, end: u32) -> CutBand {
        CutBand::new(axis, start, end).expect("valid cut")
    }

    fn reference_one(pixels: &PackedArgb32, cut: CutBand) -> Result<PackedArgb32, BandCutError> {
        let width = pixels.width();
        let height = pixels.height();
        let source = pixels.data();
        let source_stride = usize::try_from(pixels.stride()).unwrap();
        match cut.axis {
            CutAxis::Columns => {
                if cut.start >= cut.end {
                    return Err(BandCutError::EmptyRange);
                }
                if cut.end > width {
                    return Err(BandCutError::OutOfBounds);
                }
                let removed = cut.end - cut.start;
                let output_width = width
                    .checked_sub(removed)
                    .ok_or(BandCutError::SizeOverflow)?;
                if output_width == 0 {
                    return Err(BandCutError::EmptyOutput);
                }
                let (stride, row_bytes) = tight_row(output_width)?;
                let mut data = Vec::with_capacity(checked_buffer_len(row_bytes, height)?);
                for y in 0..height {
                    let row = checked_row_offset(y, source_stride)?;
                    for x in 0..width {
                        if x >= cut.start && x < cut.end {
                            continue;
                        }
                        let start = row + checked_pixel_bytes(x)?;
                        data.extend_from_slice(&source[start..start + 4]);
                    }
                }
                PackedArgb32::new(output_width, height, stride, data)
                    .ok_or(BandCutError::InvalidPixels)
            }
            CutAxis::Rows => {
                if cut.start >= cut.end {
                    return Err(BandCutError::EmptyRange);
                }
                if cut.end > height {
                    return Err(BandCutError::OutOfBounds);
                }
                let removed = cut.end - cut.start;
                let output_height = height
                    .checked_sub(removed)
                    .ok_or(BandCutError::SizeOverflow)?;
                if output_height == 0 {
                    return Err(BandCutError::EmptyOutput);
                }
                let (stride, row_bytes) = tight_row(width)?;
                let mut data = Vec::with_capacity(checked_buffer_len(row_bytes, output_height)?);
                for y in 0..height {
                    if y >= cut.start && y < cut.end {
                        continue;
                    }
                    let row = checked_row_offset(y, source_stride)?;
                    data.extend_from_slice(&source[row..row + row_bytes]);
                }
                PackedArgb32::new(width, output_height, stride, data)
                    .ok_or(BandCutError::InvalidPixels)
            }
        }
    }

    fn reference_apply(
        pixels: &PackedArgb32,
        cuts: &[CutBand],
    ) -> Result<PackedArgb32, BandCutError> {
        let mut current = pixels.clone();
        for cut in cuts {
            current = reference_one(&current, *cut)?;
        }
        Ok(current)
    }

    #[test]
    fn no_cuts_returns_an_equal_raster() {
        let source = unique_raster(4, 3);
        let output = apply_band_cuts(&source, &[]).expect("no cuts");
        assert_eq!(output, source);
        assert_eq!(output.stride(), 16);
        assert_eq!(output_size((4, 3), &[]), Ok((4, 3)));
    }

    #[test]
    fn removing_a_middle_column_joins_the_neighbours() {
        let source = raster_from_ids(5, 1, &[1, 2, 3, 4, 5]);
        let output = apply_band_cuts(&source, &[cut(CutAxis::Columns, 2, 3)]).unwrap();
        assert_eq!(ids_of(&output), vec![1, 2, 4, 5]);
        assert_eq!(output.stride(), 16);
    }

    #[test]
    fn first_and_last_column_bands_are_removed() {
        let source = raster_from_ids(6, 1, &[1, 2, 3, 4, 5, 6]);
        let output = apply_band_cuts(
            &source,
            &[cut(CutAxis::Columns, 0, 1), cut(CutAxis::Columns, 4, 5)],
        )
        .unwrap();
        // After removing the first column: 2 3 4 5 6. The second cut uses that
        // output, so [4,5) removes 6.
        assert_eq!(ids_of(&output), vec![2, 3, 4, 5]);
    }

    #[test]
    fn removing_a_middle_row_slides_content_up() {
        let source = raster_from_ids(2, 4, &[1, 2, 3, 4, 5, 6, 7, 8]);
        let output = apply_band_cuts(&source, &[cut(CutAxis::Rows, 1, 2)]).unwrap();
        assert_eq!(ids_of(&output), vec![1, 2, 5, 6, 7, 8]);
        assert_eq!((output.width(), output.height()), (2, 3));
    }

    #[test]
    fn first_and_last_row_bands_are_removed() {
        let source = raster_from_ids(1, 5, &[1, 2, 3, 4, 5]);
        let output = apply_band_cuts(
            &source,
            &[cut(CutAxis::Rows, 0, 1), cut(CutAxis::Rows, 3, 4)],
        )
        .unwrap();
        assert_eq!(ids_of(&output), vec![2, 3, 4]);
    }

    #[test]
    fn backward_edges_normalize_to_the_same_band() {
        assert_eq!(
            CutBand::from_unordered_edges(CutAxis::Columns, 5, 2).unwrap(),
            CutBand::from_unordered_edges(CutAxis::Columns, 2, 5).unwrap()
        );
    }

    #[test]
    fn sequential_same_axis_cuts_use_current_output_coordinates() {
        // A B C D E F G H, cut [2,4), then [3,5) → A B E H
        let labels = [10, 20, 30, 40, 50, 60, 70, 80];
        let source = raster_from_ids(8, 1, &labels);
        let output = apply_band_cuts(
            &source,
            &[cut(CutAxis::Columns, 2, 4), cut(CutAxis::Columns, 3, 5)],
        )
        .unwrap();
        assert_eq!(ids_of(&output), vec![10, 20, 50, 80]);
    }

    #[test]
    fn mixed_row_and_column_cuts_preserve_source_pixels() {
        let mut ids = Vec::new();
        for y in 0..4u32 {
            for x in 0..4u32 {
                ids.push(y * 10 + x);
            }
        }
        let source = raster_from_ids(4, 4, &ids);
        let output = apply_band_cuts(
            &source,
            &[cut(CutAxis::Columns, 1, 2), cut(CutAxis::Rows, 2, 3)],
        )
        .unwrap();
        assert_eq!(ids_of(&output), vec![0, 2, 3, 10, 12, 13, 30, 32, 33]);
        assert_eq!((output.width(), output.height()), (3, 3));
    }

    #[test]
    fn undo_last_then_redo_reproduces_identical_bytes() {
        let source = unique_raster(6, 5);
        let first = cut(CutAxis::Columns, 1, 3);
        let second = cut(CutAxis::Rows, 2, 4);
        let both = apply_band_cuts(&source, &[first, second]).unwrap();
        let undone = apply_band_cuts(&source, &[first]).unwrap();
        let redone = apply_band_cuts(&source, &[first, second]).unwrap();
        assert_eq!(redone, both);
        assert_ne!(undone, both);
    }

    #[test]
    fn all_four_native_order_bytes_are_copied_unchanged() {
        let bytes = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let source = PackedArgb32::new(2, 1, 8, bytes.to_vec()).unwrap();
        let output = apply_band_cuts(&source, &[cut(CutAxis::Columns, 0, 1)]).unwrap();
        assert_eq!(output.data(), &bytes[4..]);
        assert_eq!(
            pixel(output.data().try_into().unwrap()),
            pixel([0x55, 0x66, 0x77, 0x88])
        );
    }

    #[test]
    fn one_pixel_wide_or_high_output_is_valid() {
        let source = unique_raster(4, 3);
        let column = apply_band_cuts(&source, &[cut(CutAxis::Columns, 1, 4)]).unwrap();
        assert_eq!((column.width(), column.height()), (1, 3));
        let row = apply_band_cuts(&source, &[cut(CutAxis::Rows, 0, 2)]).unwrap();
        assert_eq!((row.width(), row.height()), (4, 1));
    }

    #[test]
    fn empty_half_open_range_is_rejected() {
        assert_eq!(
            CutBand::new(CutAxis::Columns, 3, 3),
            Err(BandCutError::EmptyRange)
        );
        assert_eq!(
            CutBand::from_unordered_edges(CutAxis::Rows, 4, 4),
            Err(BandCutError::EmptyRange)
        );
    }

    #[test]
    fn end_beyond_current_extent_is_rejected() {
        let source = unique_raster(3, 2);
        assert_eq!(
            apply_band_cuts(&source, &[cut(CutAxis::Columns, 1, 4)]),
            Err(BandCutError::OutOfBounds)
        );
        assert_eq!(
            output_size((3, 2), &[cut(CutAxis::Rows, 0, 3)]),
            Err(BandCutError::OutOfBounds)
        );
    }

    #[test]
    fn a_cut_that_clears_an_axis_is_rejected() {
        let source = unique_raster(3, 2);
        assert_eq!(
            apply_band_cuts(&source, &[cut(CutAxis::Columns, 0, 3)]),
            Err(BandCutError::EmptyOutput)
        );
        assert_eq!(
            apply_band_cuts(&source, &[cut(CutAxis::Rows, 0, 2)]),
            Err(BandCutError::EmptyOutput)
        );
    }

    #[test]
    fn a_later_cut_invalid_under_the_earlier_size_is_rejected() {
        let source = unique_raster(4, 3);
        let cuts = [cut(CutAxis::Columns, 1, 3), cut(CutAxis::Columns, 2, 3)];
        // After the first cut the width is 2, so [2,3) is out of bounds.
        assert_eq!(
            apply_band_cuts(&source, &cuts),
            Err(BandCutError::OutOfBounds)
        );
        assert_eq!(output_size((4, 3), &cuts), Err(BandCutError::OutOfBounds));
    }

    #[test]
    fn width_times_four_and_allocation_overflow_fail_without_panic() {
        assert_eq!(tight_row(u32::MAX), Err(BandCutError::SizeOverflow));
        assert_eq!(
            tight_row((i32::MAX as u32 / 4) + 1),
            Err(BandCutError::SizeOverflow)
        );
        assert_eq!(
            checked_buffer_len(usize::MAX, 2),
            Err(BandCutError::SizeOverflow)
        );
        assert_eq!(
            checked_row_offset(2, usize::MAX),
            Err(BandCutError::SizeOverflow)
        );
        // A u32 pixel index times four always fits in 64-bit usize; the
        // helper must still refuse to panic if a later 32-bit target is used.
        if usize::BITS <= 32 {
            assert_eq!(
                checked_pixel_bytes(u32::MAX / 2),
                Err(BandCutError::SizeOverflow)
            );
        } else {
            assert_eq!(checked_pixel_bytes(u32::MAX / 2), Ok(8_589_934_588));
        }
    }

    #[test]
    fn malformed_packed_pixels_cannot_be_constructed() {
        assert_eq!(PackedArgb32::new(2, 2, 12, vec![0; 24]), None);
        assert_eq!(PackedArgb32::new(2, 2, 8, vec![0; 15]), None);
        assert_eq!(PackedArgb32::new(u32::MAX, 1, -4, Vec::new()), None);
    }

    #[test]
    fn output_stride_equals_width_times_four() {
        let source = unique_raster(7, 5);
        let output = apply_band_cuts(&source, &[cut(CutAxis::Columns, 2, 4)]).unwrap();
        assert_eq!(output.stride(), (output.width() * 4) as i32);
        assert_eq!(
            output.data().len(),
            output.stride() as usize * output.height() as usize
        );
    }

    #[test]
    fn retained_interval_engine_matches_sequential_reference() {
        for width in 1..=6 {
            for height in 1..=6 {
                let source = unique_raster(width, height);
                assert_eq!(
                    apply_band_cuts(&source, &[]).unwrap(),
                    reference_apply(&source, &[]).unwrap()
                );
                for axis in [CutAxis::Columns, CutAxis::Rows] {
                    let extent = if axis == CutAxis::Columns {
                        width
                    } else {
                        height
                    };
                    for start in 0..extent {
                        for end in (start + 1)..=extent {
                            let Ok(band) = CutBand::new(axis, start, end) else {
                                continue;
                            };
                            let optimized = apply_band_cuts(&source, &[band]);
                            let reference = reference_apply(&source, &[band]);
                            assert_eq!(optimized, reference, "{width}x{height} {band:?}");
                        }
                    }
                }
                if width >= 4 && height >= 3 {
                    let cuts = [
                        cut(CutAxis::Columns, 1, 2),
                        cut(CutAxis::Rows, 0, 1),
                        cut(CutAxis::Columns, 1, 2),
                    ];
                    assert_eq!(
                        apply_band_cuts(&source, &cuts),
                        reference_apply(&source, &cuts)
                    );
                }
            }
        }
    }
}
