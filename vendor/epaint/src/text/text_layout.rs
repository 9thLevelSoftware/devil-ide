#![expect(clippy::unwrap_used)] // TODO(emilk): remove unwraps

use std::sync::Arc;

use emath::{Align, GuiRounding as _, NumExt as _, Pos2, Rect, Vec2, pos2, vec2};

use crate::{
    Color32, Mesh, Stroke, Vertex,
    stroke::PathStroke,
    text::{
        font::{StyledMetrics, is_cjk, is_cjk_break_allowed},
        fonts::FontFaceKey,
    },
};

use super::{
    FontsImpl, Galley, Glyph, LayoutJob, LayoutSection, PlacedRow, Row, RowVisuals, TextFormat,
};

// ----------------------------------------------------------------------------

/// Represents GUI scale and convenience methods for rounding to pixels.
#[derive(Clone, Copy)]
struct PointScale {
    pub pixels_per_point: f32,
}

impl PointScale {
    #[inline(always)]
    pub fn new(pixels_per_point: f32) -> Self {
        Self { pixels_per_point }
    }

    #[inline(always)]
    pub fn pixels_per_point(&self) -> f32 {
        self.pixels_per_point
    }

    #[inline(always)]
    pub fn round_to_pixel(&self, point: f32) -> f32 {
        (point * self.pixels_per_point).round() / self.pixels_per_point
    }

    #[inline(always)]
    pub fn floor_to_pixel(&self, point: f32) -> f32 {
        (point * self.pixels_per_point).floor() / self.pixels_per_point
    }
}

// ----------------------------------------------------------------------------

/// The glyph-free state needed to continue shaping at an exact source offset.
///
/// This is deliberately separate from [`Paragraph`], whose glyph vector is
/// retained by the ordinary layout path only. A streaming continuation can
/// therefore be cloned without copying any output-sized storage.
#[derive(Clone, Copy)]
struct ShapeState {
    /// Start of the next glyph to be added. In screen-space / physical pixels.
    cursor_x_px: f32,

    /// Previous glyph identity used by pair kerning.
    last_glyph_id: Option<skrifa::GlyphId>,
}

impl Default for ShapeState {
    fn default() -> Self {
        Self {
            cursor_x_px: 0.0,
            last_glyph_id: None,
        }
    }
}

/// Temporary storage before line-wrapping.
#[derive(Clone)]
struct Paragraph {
    pub shape: ShapeState,

    /// This is included in case there are no glyphs
    pub section_index_at_start: u32,

    pub glyphs: Vec<Glyph>,

    /// In case of an empty paragraph ("\n"), use this as height.
    pub empty_paragraph_height: f32,
}

impl Paragraph {
    pub fn from_section_index(section_index_at_start: u32) -> Self {
        Self {
            shape: ShapeState::default(),
            section_index_at_start,
            glyphs: vec![],
            empty_paragraph_height: 0.0,
        }
    }
}

/// Opaque state for the bounded, unwrapped single-format layout path.
///
/// The fields intentionally remain private so callers cannot reset pen or
/// kerning state and accidentally change rendered geometry.
#[derive(Clone)]
pub struct UnwrappedLayoutContinuation {
    source_key: u128,
    expected_byte: u64,
    format: TextFormat,
    pixels_per_point: f32,
    font_identity: Arc<()>,
    shape: ShapeState,
    initial_byte: u64,
}

/// Exact result of a fully consumed unwrapped pass.
///
/// The proof fields are private so a caller cannot manufacture a summary for
/// a different source, font set, format, or scale. The summary contains no
/// text or glyph storage.
#[derive(Clone)]
pub struct UnwrappedLayoutSummary {
    source_key: u128,
    initial_byte: u64,
    final_byte: u64,
    precise_advance_px: f32,
    format: TextFormat,
    pixels_per_point: f32,
    font_identity: Arc<()>,
}

impl UnwrappedLayoutSummary {
    /// Absolute byte range covered by the completed pass.
    pub fn byte_span(&self) -> std::ops::Range<u64> {
        self.initial_byte..self.final_byte
    }

    /// Absolute starting byte of the completed pass.
    pub fn start_byte(&self) -> u64 {
        self.initial_byte
    }

    /// Absolute byte immediately after the completed pass.
    pub fn end_byte(&self) -> u64 {
        self.final_byte
    }

    /// The exact final pen advance in logical points.
    pub fn precise_width(&self) -> f32 {
        self.precise_advance_px / self.pixels_per_point
    }

    /// Validate that this summary can be consumed by a later pass over the
    /// same immutable source and layout identity.
    pub(crate) fn validate_for_layout(
        &self,
        source_key: u128,
        expected_span: std::ops::Range<u64>,
        format: &TextFormat,
        pixels_per_point: f32,
        font_identity: &Arc<()>,
    ) -> Result<(), UnwrappedLayoutError> {
        if self.source_key != source_key
            || self.byte_span() != expected_span
            || self.format != *format
            || self.pixels_per_point != pixels_per_point
            || !Arc::ptr_eq(&self.font_identity, font_identity)
        {
            return Err(UnwrappedLayoutError::ChangedLayoutKey);
        }
        Ok(())
    }
}

impl std::fmt::Debug for UnwrappedLayoutSummary {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UnwrappedLayoutSummary")
            .field("byte_span", &self.byte_span())
            .field("precise_width", &self.precise_width())
            .finish_non_exhaustive()
    }
}

pub struct UnwrappedGlyphBatch {
    pub glyphs: Vec<Glyph>,
    pub consumed_bytes: usize,
    pub continuation: Option<UnwrappedLayoutContinuation>,
    pub status: UnwrappedLayoutStatus,
    pub summary: Option<UnwrappedLayoutSummary>,
}

impl std::fmt::Debug for UnwrappedLayoutContinuation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UnwrappedLayoutContinuation")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for UnwrappedGlyphBatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UnwrappedGlyphBatch")
            .field("glyph_count", &self.glyphs.len())
            .field("consumed_bytes", &self.consumed_bytes)
            .field("has_continuation", &self.continuation.is_some())
            .field("status", &self.status)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnwrappedLayoutStatus {
    NeedMoreInput,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnwrappedLayoutError {
    InvalidFormat,
    ChangedLayoutKey,
    InputChunkTooLarge,
    OffsetOverflow,
    OutputBudgetOutOfRange,
    NoProgress,
}

/// Shape one bounded, newline-free, single-format chunk while preserving the
/// exact epaint pen/kerning/allocation state between calls.
pub(crate) fn layout_unwrapped_chunk(
    fonts: &mut FontsImpl,
    pixels_per_point: f32,
    font_identity: Arc<()>,
    format: TextFormat,
    source_key: u128,
    chunk_start_byte: u64,
    chunk: &str,
    is_final_chunk: bool,
    continuation: Option<UnwrappedLayoutContinuation>,
    max_output_glyphs: usize,
) -> Result<UnwrappedGlyphBatch, UnwrappedLayoutError> {
    const MAX_CHUNK_BYTES: usize = 96 * 1024;
    if chunk.len() > MAX_CHUNK_BYTES {
        return Err(UnwrappedLayoutError::InputChunkTooLarge);
    }
    let _chunk_end_byte = chunk_start_byte
        .checked_add(chunk.len() as u64)
        .ok_or(UnwrappedLayoutError::OffsetOverflow)?;
    if !(1..=4096).contains(&max_output_glyphs) {
        return Err(UnwrappedLayoutError::OutputBudgetOutOfRange);
    }
    if chunk.contains('\n')
        || !pixels_per_point.is_finite()
        || pixels_per_point <= 0.0
        || !format.font_id.size.is_finite()
        || format.font_id.size <= 0.0
        || !format.extra_letter_spacing.is_finite()
    {
        return Err(UnwrappedLayoutError::InvalidFormat);
    }

    let mut state = if let Some(state) = continuation {
        if state.source_key != source_key
            || state.expected_byte != chunk_start_byte
            || state.format != format
            || state.pixels_per_point != pixels_per_point
            || !Arc::ptr_eq(&state.font_identity, &font_identity)
        {
            return Err(UnwrappedLayoutError::ChangedLayoutKey);
        }
        state
    } else {
        UnwrappedLayoutContinuation {
            source_key,
            expected_byte: chunk_start_byte,
            format: format.clone(),
            pixels_per_point,
            font_identity,
            shape: ShapeState::default(),
            initial_byte: chunk_start_byte,
        }
    };

    let line_height = format.line_height.unwrap_or_else(|| {
        fonts
            .font(&format.font_id.family)
            .styled_metrics(pixels_per_point, format.font_id.size, &format.coords)
            .row_height
    });
    let mut glyphs = Vec::new();
    let consumed = shape_chars(
        &mut fonts.font(&format.font_id.family),
        pixels_per_point,
        &format,
        0,
        chunk,
        line_height,
        &mut state.shape,
        &mut glyphs,
        Some(max_output_glyphs),
    );
    if !chunk.is_empty() && consumed == 0 {
        return Err(UnwrappedLayoutError::NoProgress);
    }
    state.expected_byte = chunk_start_byte
        .checked_add(consumed as u64)
        .ok_or(UnwrappedLayoutError::OffsetOverflow)?;
    debug_assert!(state.expected_byte <= _chunk_end_byte);
    let complete = is_final_chunk && consumed == chunk.len();
    let summary = complete.then(|| UnwrappedLayoutSummary {
        source_key: state.source_key,
        initial_byte: state.initial_byte,
        final_byte: state.expected_byte,
        precise_advance_px: state.shape.cursor_x_px,
        format: state.format.clone(),
        pixels_per_point: state.pixels_per_point,
        font_identity: Arc::clone(&state.font_identity),
    });
    if let Some(summary) = &summary {
        summary.validate_for_layout(
            state.source_key,
            state.initial_byte..state.expected_byte,
            &state.format,
            state.pixels_per_point,
            &state.font_identity,
        )?;
    }
    Ok(UnwrappedGlyphBatch {
        glyphs,
        consumed_bytes: consumed,
        continuation: (!complete).then_some(state),
        status: complete
            .then_some(UnwrappedLayoutStatus::Complete)
            .unwrap_or(UnwrappedLayoutStatus::NeedMoreInput),
        summary,
    })
}

/// Layout text into a [`Galley`].
///
/// In most cases you should use [`crate::FontsView::layout_job`] instead
/// since that memoizes the input, making subsequent layouting of the same text much faster.
pub fn layout(fonts: &mut FontsImpl, pixels_per_point: f32, job: Arc<LayoutJob>) -> Galley {
    profiling::function_scope!();

    if job.wrap.max_rows == 0 {
        // Early-out: no text
        return Galley {
            job,
            rows: Default::default(),
            rect: Rect::ZERO,
            mesh_bounds: Rect::NOTHING,
            num_vertices: 0,
            num_indices: 0,
            pixels_per_point,
            elided: true,
            intrinsic_size: Vec2::ZERO,
        };
    }

    // For most of this we ignore the y coordinate:

    let mut paragraphs = vec![Paragraph::from_section_index(0)];
    for (section_index, section) in job.sections.iter().enumerate() {
        layout_section(
            fonts,
            pixels_per_point,
            &job,
            section_index as u32,
            section,
            &mut paragraphs,
        );
    }

    let point_scale = PointScale::new(pixels_per_point);

    let intrinsic_size = calculate_intrinsic_size(point_scale, &job, &paragraphs);

    let mut elided = false;
    let mut rows = rows_from_paragraphs(paragraphs, &job, pixels_per_point, &mut elided);
    if elided && let Some(last_placed) = rows.last_mut() {
        let last_row = Arc::make_mut(&mut last_placed.row);
        replace_last_glyph_with_overflow_character(fonts, pixels_per_point, &job, last_row);
        if let Some(last) = last_row.glyphs.last() {
            last_row.size.x = last.max_x();
        }
    }

    let justify = job.justify && job.wrap.max_width.is_finite();

    if justify || job.halign != Align::LEFT {
        let num_rows = rows.len();
        for (i, placed_row) in rows.iter_mut().enumerate() {
            let is_last_row = i + 1 == num_rows;
            let justify_row = justify && !placed_row.ends_with_newline && !is_last_row;
            halign_and_justify_row(
                point_scale,
                placed_row,
                job.halign,
                job.wrap.max_width,
                justify_row,
            );
        }
    }

    // Calculate the Y positions and tessellate the text:
    galley_from_rows(point_scale, job, rows, elided, intrinsic_size)
}

// Ignores the Y coordinate.
fn layout_section(
    fonts: &mut FontsImpl,
    pixels_per_point: f32,
    job: &LayoutJob,
    section_index: u32,
    section: &LayoutSection,
    out_paragraphs: &mut Vec<Paragraph>,
) {
    let LayoutSection {
        leading_space,
        byte_range,
        format,
    } = section;
    let font_size = format.font_id.size;
    let mut font = fonts.font(&format.font_id.family);
    let font_metrics = font.styled_metrics(pixels_per_point, font_size, &format.coords);
    let line_height = section
        .format
        .line_height
        .unwrap_or(font_metrics.row_height);
    let mut paragraph = out_paragraphs.last_mut().unwrap();
    if paragraph.glyphs.is_empty() {
        paragraph.empty_paragraph_height = line_height; // TODO(emilk): replace this hack with actually including `\n` in the glyphs?
    }

    paragraph.shape.cursor_x_px += leading_space * pixels_per_point;
    // Ordinary jobs reset kerning at each section, matching the pre-streaming
    // layout behavior. Bounded continuation uses the same field across calls.
    paragraph.shape.last_glyph_id = None;
    let text = &job.text[byte_range.clone()];
    let mut segment_start = 0;
    for (offset, chr) in text.char_indices() {
        if job.break_on_newline && chr == '\n' {
            shape_chars(
                &mut font,
                pixels_per_point,
                format,
                section_index,
                &text[segment_start..offset],
                line_height,
                &mut paragraph.shape,
                &mut paragraph.glyphs,
                None,
            );
            out_paragraphs.push(Paragraph::from_section_index(section_index));
            paragraph = out_paragraphs.last_mut().unwrap();
            paragraph.empty_paragraph_height = line_height;
            segment_start = offset + chr.len_utf8();
        }
    }
    shape_chars(
        &mut font,
        pixels_per_point,
        format,
        section_index,
        &text[segment_start..],
        line_height,
        &mut paragraph.shape,
        &mut paragraph.glyphs,
        None,
    );
}

/// Shared glyph shaping loop used by ordinary layout and bounded continuation
/// layout. The caller owns row breaking; this function owns pen, kerning,
/// subpixel allocation, and observable glyph geometry.
fn shape_chars(
    font: &mut crate::text::font::Font<'_>,
    pixels_per_point: f32,
    format: &TextFormat,
    section_index: u32,
    text: &str,
    line_height: f32,
    shape: &mut ShapeState,
    glyphs: &mut Vec<Glyph>,
    max_glyphs: Option<usize>,
) -> usize {
    let font_size = format.font_id.size;
    let font_metrics = font.styled_metrics(pixels_per_point, font_size, &format.coords);
    let extra_letter_spacing = format.extra_letter_spacing;
    let mut current_font = FontFaceKey::INVALID;
    let mut current_font_face_metrics = StyledMetrics::default();
    let mut consumed = 0;

    for (offset, chr) in text.char_indices() {
        if max_glyphs.is_some_and(|limit| glyphs.len() >= limit) {
            break;
        }
        let (font_id, glyph_info) = font.glyph_info(chr);
        let mut font_face = font.fonts_by_id.get_mut(&font_id);
        if current_font != font_id {
            current_font = font_id;
            current_font_face_metrics = font_face
                .as_ref()
                .map(|font_face| {
                    font_face.styled_metrics(pixels_per_point, font_size, &format.coords)
                })
                .unwrap_or_default();
        }
        if let (Some(font_face), Some(last_glyph_id), Some(glyph_id)) =
            (&font_face, shape.last_glyph_id, glyph_info.id)
        {
            shape.cursor_x_px +=
                font_face.pair_kerning_pixels(&current_font_face_metrics, last_glyph_id, glyph_id);
            shape.cursor_x_px += extra_letter_spacing * pixels_per_point;
        }
        let (glyph_alloc, physical_x) = if let Some(font_face) = font_face.as_mut() {
            font_face.allocate_glyph(
                font.atlas,
                &current_font_face_metrics,
                glyph_info,
                chr,
                shape.cursor_x_px,
            )
        } else {
            Default::default()
        };
        glyphs.push(Glyph {
            chr,
            pos: pos2(physical_x as f32 / pixels_per_point, f32::NAN),
            advance_width: glyph_alloc.advance_width_px / pixels_per_point,
            line_height,
            font_face_height: current_font_face_metrics.row_height,
            font_face_ascent: current_font_face_metrics.ascent,
            font_height: font_metrics.row_height,
            font_ascent: font_metrics.ascent,
            uv_rect: glyph_alloc.uv_rect,
            section_index,
            first_vertex: 0,
        });
        shape.cursor_x_px += glyph_alloc.advance_width_px;
        shape.last_glyph_id = Some(glyph_alloc.id);
        consumed = offset + chr.len_utf8();
    }
    consumed
}

/// Calculate the intrinsic size of the text.
///
/// The result is eventually passed to `Response::intrinsic_size`.
/// This works by calculating the size of each `Paragraph` (instead of each `Row`).
fn calculate_intrinsic_size(
    point_scale: PointScale,
    job: &LayoutJob,
    paragraphs: &[Paragraph],
) -> Vec2 {
    let mut intrinsic_size = Vec2::ZERO;
    for (idx, paragraph) in paragraphs.iter().enumerate() {
        // Use the precise cursor position instead of `last_glyph.max_x()`,
        // because glyph positions are pixel-snapped but the cursor tracks
        // the exact subpixel advance. This ensures that when two galleys are
        // placed side-by-side, the gap matches what it would be within a
        // single galley.
        let width = paragraph.shape.cursor_x_px / point_scale.pixels_per_point;
        intrinsic_size.x = f32::max(intrinsic_size.x, width);

        let mut height = paragraph
            .glyphs
            .iter()
            .map(|g| g.line_height)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(paragraph.empty_paragraph_height);
        if idx == 0 {
            height = f32::max(height, job.first_row_min_height);
        }
        intrinsic_size.y += point_scale.round_to_pixel(height);
    }
    intrinsic_size
}

// Ignores the Y coordinate.
fn rows_from_paragraphs(
    paragraphs: Vec<Paragraph>,
    job: &LayoutJob,
    pixels_per_point: f32,
    elided: &mut bool,
) -> Vec<PlacedRow> {
    let num_paragraphs = paragraphs.len();

    let mut rows = vec![];

    for (i, paragraph) in paragraphs.into_iter().enumerate() {
        if job.wrap.max_rows <= rows.len() {
            *elided = true;
            break;
        }

        let is_last_paragraph = (i + 1) == num_paragraphs;

        if paragraph.glyphs.is_empty() {
            rows.push(PlacedRow {
                pos: pos2(0.0, f32::NAN),
                row: Arc::new(Row {
                    section_index_at_start: paragraph.section_index_at_start,
                    glyphs: vec![],
                    visuals: Default::default(),
                    size: vec2(0.0, paragraph.empty_paragraph_height),
                }),
                ends_with_newline: !is_last_paragraph,
            });
        } else {
            // Use precise cursor position for width instead of pixel-snapped
            // `last_glyph.max_x()`, so that side-by-side galleys have the same
            // spacing as characters within a single galley.
            let paragraph_width = paragraph.shape.cursor_x_px / pixels_per_point;
            if paragraph_width <= job.effective_wrap_width() {
                // Early-out optimization: the whole paragraph fits on one row.
                rows.push(PlacedRow {
                    pos: pos2(0.0, f32::NAN),
                    row: Arc::new(Row {
                        section_index_at_start: paragraph.section_index_at_start,
                        glyphs: paragraph.glyphs,
                        visuals: Default::default(),
                        size: vec2(paragraph_width, 0.0),
                    }),
                    ends_with_newline: !is_last_paragraph,
                });
            } else {
                line_break(&paragraph, job, &mut rows, elided);
                let placed_row = rows.last_mut().unwrap();
                placed_row.ends_with_newline = !is_last_paragraph;
            }
        }
    }

    rows
}

fn line_break(
    paragraph: &Paragraph,
    job: &LayoutJob,
    out_rows: &mut Vec<PlacedRow>,
    elided: &mut bool,
) {
    let wrap_width = job.effective_wrap_width();

    // Keeps track of good places to insert row break if we exceed `wrap_width`.
    let mut row_break_candidates = RowBreakCandidates::default();

    let mut first_row_indentation = paragraph.glyphs[0].pos.x;
    let mut row_start_x = 0.0;
    let mut row_start_idx = 0;

    for i in 0..paragraph.glyphs.len() {
        if job.wrap.max_rows <= out_rows.len() {
            *elided = true;
            break;
        }

        let potential_row_width = paragraph.glyphs[i].max_x() - row_start_x;

        if wrap_width < potential_row_width {
            // Row break:

            if first_row_indentation > 0.0
                && !row_break_candidates.has_good_candidate(job.wrap.break_anywhere)
            {
                // Allow the first row to be completely empty, because we know there will be more space on the next row:
                // TODO(emilk): this records the height of this first row as zero, though that is probably fine since first_row_indentation usually comes with a first_row_min_height.
                out_rows.push(PlacedRow {
                    pos: pos2(0.0, f32::NAN),
                    row: Arc::new(Row {
                        section_index_at_start: paragraph.section_index_at_start,
                        glyphs: vec![],
                        visuals: Default::default(),
                        size: Vec2::ZERO,
                    }),
                    ends_with_newline: false,
                });
                row_start_x += first_row_indentation;
                first_row_indentation = 0.0;
            } else if let Some(last_kept_index) = row_break_candidates.get(job.wrap.break_anywhere)
            {
                let glyphs: Vec<Glyph> = paragraph.glyphs[row_start_idx..=last_kept_index]
                    .iter()
                    .copied()
                    .map(|mut glyph| {
                        glyph.pos.x -= row_start_x;
                        glyph
                    })
                    .collect();

                let section_index_at_start = glyphs[0].section_index;
                let paragraph_max_x = glyphs.last().unwrap().max_x();

                out_rows.push(PlacedRow {
                    pos: pos2(0.0, f32::NAN),
                    row: Arc::new(Row {
                        section_index_at_start,
                        glyphs,
                        visuals: Default::default(),
                        size: vec2(paragraph_max_x, 0.0),
                    }),
                    ends_with_newline: false,
                });

                // Start a new row:
                row_start_idx = last_kept_index + 1;
                row_start_x = paragraph.glyphs[row_start_idx].pos.x;
                row_break_candidates.forget_before_idx(row_start_idx);
            } else {
                // Found no place to break, so we have to overrun wrap_width.
            }
        }

        row_break_candidates.add(i, &paragraph.glyphs[i..]);
    }

    if row_start_idx < paragraph.glyphs.len() {
        // Final row of text:

        if job.wrap.max_rows <= out_rows.len() {
            *elided = true; // can't fit another row
        } else {
            let paragraph_min_x = paragraph.glyphs[row_start_idx].pos.x - row_start_x;
            let paragraph_max_x = paragraph.glyphs.last().unwrap().max_x() - row_start_x;

            let glyphs: Vec<Glyph> = paragraph.glyphs[row_start_idx..]
                .iter()
                .copied()
                .map(|mut glyph| {
                    glyph.pos.x -= row_start_x + paragraph_min_x;
                    glyph
                })
                .collect();

            let section_index_at_start = glyphs[0].section_index;

            out_rows.push(PlacedRow {
                pos: pos2(paragraph_min_x, 0.0),
                row: Arc::new(Row {
                    section_index_at_start,
                    glyphs,
                    visuals: Default::default(),
                    size: vec2(paragraph_max_x - paragraph_min_x, 0.0),
                }),
                ends_with_newline: false,
            });
        }
    }
}

/// Trims the last glyphs in the row and replaces it with an overflow character (e.g. `…`).
///
/// Called before we have any Y coordinates.
fn replace_last_glyph_with_overflow_character(
    fonts: &mut FontsImpl,
    pixels_per_point: f32,
    job: &LayoutJob,
    row: &mut Row,
) {
    let Some(overflow_character) = job.wrap.overflow_character else {
        return;
    };

    let mut section_index = row
        .glyphs
        .last()
        .map(|g| g.section_index)
        .unwrap_or(row.section_index_at_start);
    loop {
        let section = &job.sections[section_index as usize];
        let extra_letter_spacing = section.format.extra_letter_spacing;
        let mut font = fonts.font(&section.format.font_id.family);
        let font_size = section.format.font_id.size;

        let (font_id, glyph_info) = font.glyph_info(overflow_character);
        let mut font_face = font.fonts_by_id.get_mut(&font_id);
        let font_face_metrics = font_face
            .as_mut()
            .map(|f| f.styled_metrics(pixels_per_point, font_size, &section.format.coords))
            .unwrap_or_default();

        let overflow_glyph_x = if let Some(prev_glyph) = row.glyphs.last() {
            // Kern the overflow character properly
            let pair_kerning = font_face
                .as_mut()
                .map(|font_face| {
                    if let (Some(prev_glyph_id), Some(overflow_glyph_id)) = (
                        font_face.glyph_info(prev_glyph.chr).and_then(|g| g.id),
                        font_face.glyph_info(overflow_character).and_then(|g| g.id),
                    ) {
                        font_face.pair_kerning(&font_face_metrics, prev_glyph_id, overflow_glyph_id)
                    } else {
                        0.0
                    }
                })
                .unwrap_or_default();

            prev_glyph.max_x() + extra_letter_spacing + pair_kerning
        } else {
            0.0 // TODO(emilk): heed paragraph leading_space 😬
        };

        let replacement_glyph_width = font_face
            .as_mut()
            .and_then(|f| f.glyph_info(overflow_character))
            .map(|i| {
                i.advance_width_unscaled.0 * font_face_metrics.px_scale_factor / pixels_per_point
            })
            .unwrap_or_default();

        // Check if we're within width budget:
        if overflow_glyph_x + replacement_glyph_width <= job.effective_wrap_width()
            || row.glyphs.is_empty()
        {
            // we are done

            let (replacement_glyph_alloc, physical_x) = font_face
                .as_mut()
                .map(|f| {
                    f.allocate_glyph(
                        font.atlas,
                        &font_face_metrics,
                        glyph_info,
                        overflow_character,
                        overflow_glyph_x * pixels_per_point,
                    )
                })
                .unwrap_or_default();

            let font_metrics =
                font.styled_metrics(pixels_per_point, font_size, &section.format.coords);
            let line_height = section
                .format
                .line_height
                .unwrap_or(font_metrics.row_height);

            row.glyphs.push(Glyph {
                chr: overflow_character,
                pos: pos2(physical_x as f32 / pixels_per_point, f32::NAN),
                advance_width: replacement_glyph_alloc.advance_width_px / pixels_per_point,
                line_height,
                font_face_height: font_face_metrics.row_height,
                font_face_ascent: font_face_metrics.ascent,
                font_height: font_metrics.row_height,
                font_ascent: font_metrics.ascent,
                uv_rect: replacement_glyph_alloc.uv_rect,
                section_index,
                first_vertex: 0, // filled in later
            });
            return;
        }

        // We didn't fit - pop the last glyph and try again.
        if let Some(last_glyph) = row.glyphs.pop() {
            section_index = last_glyph.section_index;
        } else {
            section_index = row.section_index_at_start;
        }
    }
}

/// Horizontally aligned the text on a row.
///
/// Ignores the Y coordinate.
fn halign_and_justify_row(
    point_scale: PointScale,
    placed_row: &mut PlacedRow,
    halign: Align,
    wrap_width: f32,
    justify: bool,
) {
    #![expect(clippy::useless_let_if_seq)] // False positive

    let row = Arc::make_mut(&mut placed_row.row);

    if row.glyphs.is_empty() {
        return;
    }

    let num_leading_spaces = row
        .glyphs
        .iter()
        .take_while(|glyph| glyph.chr.is_whitespace())
        .count();

    let glyph_range = if num_leading_spaces == row.glyphs.len() {
        // There is only whitespace
        (0, row.glyphs.len())
    } else {
        let num_trailing_spaces = row
            .glyphs
            .iter()
            .rev()
            .take_while(|glyph| glyph.chr.is_whitespace())
            .count();

        (num_leading_spaces, row.glyphs.len() - num_trailing_spaces)
    };
    let num_glyphs_in_range = glyph_range.1 - glyph_range.0;
    assert!(num_glyphs_in_range > 0, "Should have at least one glyph");

    let original_min_x = row.glyphs[glyph_range.0].logical_rect().min.x;
    let original_max_x = row.glyphs[glyph_range.1 - 1].logical_rect().max.x;
    let original_width = original_max_x - original_min_x;

    let target_width = if justify && num_glyphs_in_range > 1 {
        wrap_width
    } else {
        original_width
    };

    let (target_min_x, target_max_x) = match halign {
        Align::LEFT => (0.0, target_width),
        Align::Center => (-target_width / 2.0, target_width / 2.0),
        Align::RIGHT => (-target_width, 0.0),
    };

    let num_spaces_in_range = row.glyphs[glyph_range.0..glyph_range.1]
        .iter()
        .filter(|glyph| glyph.chr.is_whitespace())
        .count();

    let mut extra_x_per_glyph = if num_glyphs_in_range == 1 {
        0.0
    } else {
        (target_width - original_width) / (num_glyphs_in_range as f32 - 1.0)
    };
    extra_x_per_glyph = extra_x_per_glyph.at_least(0.0); // Don't contract

    let mut extra_x_per_space = 0.0;
    if 0 < num_spaces_in_range && num_spaces_in_range < num_glyphs_in_range {
        // Add an integral number of pixels between each glyph,
        // and add the balance to the spaces:

        extra_x_per_glyph = point_scale.floor_to_pixel(extra_x_per_glyph);

        extra_x_per_space = (target_width
            - original_width
            - extra_x_per_glyph * (num_glyphs_in_range as f32 - 1.0))
            / (num_spaces_in_range as f32);
    }

    placed_row.pos.x = point_scale.round_to_pixel(target_min_x);
    let mut translate_x = -original_min_x - extra_x_per_glyph * glyph_range.0 as f32;

    for glyph in &mut row.glyphs {
        glyph.pos.x += translate_x;
        glyph.pos.x = point_scale.round_to_pixel(glyph.pos.x);
        translate_x += extra_x_per_glyph;
        if glyph.chr.is_whitespace() {
            translate_x += extra_x_per_space;
        }
    }

    // Note we ignore the leading/trailing whitespace here!
    row.size.x = target_max_x - target_min_x;
}

/// Calculate the Y positions and tessellate the text.
fn galley_from_rows(
    point_scale: PointScale,
    job: Arc<LayoutJob>,
    mut rows: Vec<PlacedRow>,
    elided: bool,
    intrinsic_size: Vec2,
) -> Galley {
    let mut first_row_min_height = job.first_row_min_height;
    let mut cursor_y = 0.0;

    for placed_row in &mut rows {
        let mut max_row_height = first_row_min_height.at_least(placed_row.height());
        let row = Arc::make_mut(&mut placed_row.row);

        first_row_min_height = 0.0;
        for glyph in &row.glyphs {
            max_row_height = max_row_height.at_least(glyph.line_height);
        }
        max_row_height = point_scale.round_to_pixel(max_row_height);

        // Now position each glyph vertically:
        for glyph in &mut row.glyphs {
            let format = &job.sections[glyph.section_index as usize].format;

            glyph.pos.y = glyph.font_face_ascent

                // Apply valign to the different in height of the entire row, and the height of this `Font`:
                + format.valign.to_factor() * (max_row_height - glyph.line_height)

                // When mixing different `FontImpl` (e.g. latin and emojis),
                // we always center the difference:
                + 0.5 * (glyph.font_height - glyph.font_face_height);

            glyph.pos.y = point_scale.round_to_pixel(glyph.pos.y);
        }

        placed_row.pos.y = cursor_y;
        row.size.y = max_row_height;

        cursor_y += max_row_height;
        cursor_y = point_scale.round_to_pixel(cursor_y); // TODO(emilk): it would be better to do the calculations in pixels instead.
    }

    let format_summary = format_summary(&job);

    let mut rect = Rect::ZERO;
    let mut mesh_bounds = Rect::NOTHING;
    let mut num_vertices = 0;
    let mut num_indices = 0;

    for placed_row in &mut rows {
        rect |= placed_row.rect();

        let row = Arc::make_mut(&mut placed_row.row);
        row.visuals = tessellate_row(point_scale, &job, &format_summary, row);

        mesh_bounds |= row.visuals.mesh_bounds.translate(placed_row.pos.to_vec2());
        num_vertices += row.visuals.mesh.vertices.len();
        num_indices += row.visuals.mesh.indices.len();

        row.section_index_at_start = u32::MAX; // No longer in use.
        for glyph in &mut row.glyphs {
            glyph.section_index = u32::MAX; // No longer in use.
        }
    }

    let mut galley = Galley {
        job,
        rows,
        elided,
        rect,
        mesh_bounds,
        num_vertices,
        num_indices,
        pixels_per_point: point_scale.pixels_per_point,
        intrinsic_size,
    };

    if galley.job.round_output_to_gui {
        galley.round_output_to_gui();
    }

    galley
}

#[derive(Default)]
struct FormatSummary {
    any_background: bool,
    any_underline: bool,
    any_strikethrough: bool,
}

fn format_summary(job: &LayoutJob) -> FormatSummary {
    let mut format_summary = FormatSummary::default();
    for section in &job.sections {
        format_summary.any_background |= section.format.background != Color32::TRANSPARENT;
        format_summary.any_underline |= section.format.underline != Stroke::NONE;
        format_summary.any_strikethrough |= section.format.strikethrough != Stroke::NONE;
    }
    format_summary
}

fn tessellate_row(
    point_scale: PointScale,
    job: &LayoutJob,
    format_summary: &FormatSummary,
    row: &mut Row,
) -> RowVisuals {
    if row.glyphs.is_empty() {
        return Default::default();
    }

    let mut mesh = Mesh::default();

    mesh.reserve_triangles(row.glyphs.len() * 2);
    mesh.reserve_vertices(row.glyphs.len() * 4);

    if format_summary.any_background {
        add_row_backgrounds(point_scale, job, row, &mut mesh);
    }

    let glyph_index_start = mesh.indices.len();
    let glyph_vertex_start = mesh.vertices.len();
    tessellate_glyphs(point_scale, job, row, &mut mesh);
    let glyph_vertex_end = mesh.vertices.len();

    if format_summary.any_underline {
        add_row_hline(point_scale, row, &mut mesh, |glyph| {
            let format = &job.sections[glyph.section_index as usize].format;
            let stroke = format.underline;
            let y = glyph.logical_rect().bottom();
            (stroke, y)
        });
    }

    if format_summary.any_strikethrough {
        add_row_hline(point_scale, row, &mut mesh, |glyph| {
            let format = &job.sections[glyph.section_index as usize].format;
            let stroke = format.strikethrough;
            let y = glyph.logical_rect().center().y;
            (stroke, y)
        });
    }

    let mesh_bounds = mesh.calc_bounds();

    RowVisuals {
        mesh,
        mesh_bounds,
        glyph_index_start,
        glyph_vertex_range: glyph_vertex_start..glyph_vertex_end,
    }
}

/// Create background for glyphs that have them.
/// Creates as few rectangular regions as possible.
fn add_row_backgrounds(point_scale: PointScale, job: &LayoutJob, row: &Row, mesh: &mut Mesh) {
    if row.glyphs.is_empty() {
        return;
    }

    let mut end_run = |start: Option<(Color32, Rect, f32)>, stop_x: f32| {
        if let Some((color, start_rect, expand)) = start {
            let rect = Rect::from_min_max(start_rect.left_top(), pos2(stop_x, start_rect.bottom()));
            let rect = rect.expand(expand);
            let rect = rect.round_to_pixels(point_scale.pixels_per_point());
            mesh.add_colored_rect(rect, color);
        }
    };

    let mut run_start = None;
    let mut last_rect = Rect::NAN;

    for glyph in &row.glyphs {
        let format = &job.sections[glyph.section_index as usize].format;
        let color = format.background;
        let rect = glyph.logical_rect();

        if color == Color32::TRANSPARENT {
            end_run(run_start.take(), last_rect.right());
        } else if let Some((existing_color, start, expand)) = run_start {
            if existing_color == color
                && start.top() == rect.top()
                && start.bottom() == rect.bottom()
                && format.expand_bg == expand
            {
                // continue the same background rectangle
            } else {
                end_run(run_start.take(), last_rect.right());
                run_start = Some((color, rect, format.expand_bg));
            }
        } else {
            run_start = Some((color, rect, format.expand_bg));
        }

        last_rect = rect;
    }

    end_run(run_start.take(), last_rect.right());
}

fn tessellate_glyphs(point_scale: PointScale, job: &LayoutJob, row: &mut Row, mesh: &mut Mesh) {
    for glyph in &mut row.glyphs {
        glyph.first_vertex = mesh.vertices.len() as u32;
        let uv_rect = glyph.uv_rect;
        if !uv_rect.is_nothing() {
            let mut left_top = glyph.pos + uv_rect.offset;
            left_top.x = point_scale.round_to_pixel(left_top.x);
            left_top.y = point_scale.round_to_pixel(left_top.y);

            let rect = Rect::from_min_max(left_top, left_top + uv_rect.size);
            let uv = Rect::from_min_max(
                pos2(uv_rect.min[0] as f32, uv_rect.min[1] as f32),
                pos2(uv_rect.max[0] as f32, uv_rect.max[1] as f32),
            );

            let format = &job.sections[glyph.section_index as usize].format;

            let color = format.color;

            if format.italics {
                let idx = mesh.vertices.len() as u32;
                mesh.add_triangle(idx, idx + 1, idx + 2);
                mesh.add_triangle(idx + 2, idx + 1, idx + 3);

                let top_offset = rect.height() * 0.25 * Vec2::X;

                mesh.vertices.push(Vertex {
                    pos: rect.left_top() + top_offset,
                    uv: uv.left_top(),
                    color,
                });
                mesh.vertices.push(Vertex {
                    pos: rect.right_top() + top_offset,
                    uv: uv.right_top(),
                    color,
                });
                mesh.vertices.push(Vertex {
                    pos: rect.left_bottom(),
                    uv: uv.left_bottom(),
                    color,
                });
                mesh.vertices.push(Vertex {
                    pos: rect.right_bottom(),
                    uv: uv.right_bottom(),
                    color,
                });
            } else {
                mesh.add_rect_with_uv(rect, uv, color);
            }
        }
    }
}

/// Add a horizontal line over a row of glyphs with a stroke and y decided by a callback.
fn add_row_hline(
    point_scale: PointScale,
    row: &Row,
    mesh: &mut Mesh,
    stroke_and_y: impl Fn(&Glyph) -> (Stroke, f32),
) {
    let mut path = crate::tessellator::Path::default(); // reusing path to avoid re-allocations.

    let mut end_line = |start: Option<(Stroke, Pos2)>, stop_x: f32| {
        if let Some((stroke, start)) = start {
            let stop = pos2(stop_x, start.y);
            path.clear();
            path.add_line_segment([start, stop]);
            let feathering = 1.0 / point_scale.pixels_per_point();
            path.stroke_open(feathering, &PathStroke::from(stroke), mesh);
        }
    };

    let mut line_start = None;
    let mut last_right_x = f32::NAN;

    for glyph in &row.glyphs {
        let (stroke, mut y) = stroke_and_y(glyph);
        stroke.round_center_to_pixel(point_scale.pixels_per_point, &mut y);

        if stroke.is_empty() {
            end_line(line_start.take(), last_right_x);
        } else if let Some((existing_stroke, start)) = line_start {
            if existing_stroke == stroke && start.y == y {
                // continue the same line
            } else {
                end_line(line_start.take(), last_right_x);
                line_start = Some((stroke, pos2(glyph.pos.x, y)));
            }
        } else {
            line_start = Some((stroke, pos2(glyph.pos.x, y)));
        }

        last_right_x = glyph.max_x();
    }

    end_line(line_start.take(), last_right_x);
}

// ----------------------------------------------------------------------------

/// Keeps track of good places to break a long row of text.
/// Will focus primarily on spaces, secondarily on things like `-`
#[derive(Clone, Copy, Default)]
struct RowBreakCandidates {
    /// Breaking at ` ` or other whitespace
    /// is always the primary candidate.
    space: Option<usize>,

    /// Logograms (single character representing a whole word) or kana (Japanese hiragana and katakana) are good candidates for line break.
    cjk: Option<usize>,

    /// Breaking anywhere before a CJK character is acceptable too.
    pre_cjk: Option<usize>,

    /// Breaking at a dash is a super-
    /// good idea.
    dash: Option<usize>,

    /// This is nicer for things like URLs, e.g. www.
    /// example.com.
    punctuation: Option<usize>,

    /// Breaking after just random character is some
    /// times necessary.
    any: Option<usize>,
}

impl RowBreakCandidates {
    fn add(&mut self, index: usize, glyphs: &[Glyph]) {
        let chr = glyphs[0].chr;
        const NON_BREAKING_SPACE: char = '\u{A0}';
        if chr.is_whitespace() && chr != NON_BREAKING_SPACE {
            self.space = Some(index);
        } else if is_cjk(chr) && (glyphs.len() == 1 || is_cjk_break_allowed(glyphs[1].chr)) {
            self.cjk = Some(index);
        } else if chr == '-' {
            self.dash = Some(index);
        } else if chr.is_ascii_punctuation() {
            self.punctuation = Some(index);
        } else if glyphs.len() > 1 && is_cjk(glyphs[1].chr) {
            self.pre_cjk = Some(index);
        }
        self.any = Some(index);
    }

    fn word_boundary(&self) -> Option<usize> {
        [self.space, self.cjk, self.pre_cjk]
            .into_iter()
            .max()
            .flatten()
    }

    fn has_good_candidate(&self, break_anywhere: bool) -> bool {
        if break_anywhere {
            self.any.is_some()
        } else {
            self.word_boundary().is_some()
        }
    }

    fn get(&self, break_anywhere: bool) -> Option<usize> {
        if break_anywhere {
            self.any
        } else {
            self.word_boundary()
                .or(self.dash)
                .or(self.punctuation)
                .or(self.any)
        }
    }

    fn forget_before_idx(&mut self, index: usize) {
        let Self {
            space,
            cjk,
            pre_cjk,
            dash,
            punctuation,
            any,
        } = self;
        if space.is_some_and(|s| s < index) {
            *space = None;
        }
        if cjk.is_some_and(|s| s < index) {
            *cjk = None;
        }
        if pre_cjk.is_some_and(|s| s < index) {
            *pre_cjk = None;
        }
        if dash.is_some_and(|s| s < index) {
            *dash = None;
        }
        if punctuation.is_some_and(|s| s < index) {
            *punctuation = None;
        }
        if any.is_some_and(|s| s < index) {
            *any = None;
        }
    }
}

// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {

    use super::{super::*, *};

    // Frozen reference copied from the pre-streaming epaint 0.34.2 shaping
    // loop. Keep this test-only path independent from `shape_chars` so the
    // continuation test can detect regressions shared by both paths.
    fn pristine_reference_glyphs(
        fonts: &mut FontsImpl,
        pixels_per_point: f32,
        format: &TextFormat,
        text: &str,
    ) -> Vec<Glyph> {
        pristine_reference_shape(fonts, pixels_per_point, format, text).0
    }

    fn pristine_reference_shape(
        fonts: &mut FontsImpl,
        pixels_per_point: f32,
        format: &TextFormat,
        text: &str,
    ) -> (Vec<Glyph>, f32) {
        let mut font = fonts.font(&format.font_id.family);
        let font_size = format.font_id.size;
        let font_metrics = font.styled_metrics(pixels_per_point, font_size, &format.coords);
        let line_height = format.line_height.unwrap_or(font_metrics.row_height);
        let mut cursor_x_px = 0.0;
        let mut last_glyph_id = None;
        let mut current_font = FontFaceKey::INVALID;
        let mut current_metrics = StyledMetrics::default();
        let mut glyphs = Vec::new();
        for chr in text.chars() {
            let (font_id, glyph_info) = font.glyph_info(chr);
            let mut font_face = font.fonts_by_id.get_mut(&font_id);
            if current_font != font_id {
                current_font = font_id;
                current_metrics = font_face
                    .as_ref()
                    .map(|face| face.styled_metrics(pixels_per_point, font_size, &format.coords))
                    .unwrap_or_default();
            }
            if let (Some(face), Some(previous), Some(glyph_id)) =
                (&font_face, last_glyph_id, glyph_info.id)
            {
                cursor_x_px += face.pair_kerning_pixels(&current_metrics, previous, glyph_id);
                cursor_x_px += format.extra_letter_spacing * pixels_per_point;
            }
            let (allocation, physical_x) = font_face
                .as_mut()
                .map(|face| {
                    face.allocate_glyph(font.atlas, &current_metrics, glyph_info, chr, cursor_x_px)
                })
                .unwrap_or_default();
            glyphs.push(Glyph {
                chr,
                pos: pos2(physical_x as f32 / pixels_per_point, f32::NAN),
                advance_width: allocation.advance_width_px / pixels_per_point,
                line_height,
                font_face_height: current_metrics.row_height,
                font_face_ascent: current_metrics.ascent,
                font_height: font_metrics.row_height,
                font_ascent: font_metrics.ascent,
                uv_rect: allocation.uv_rect,
                section_index: 0,
                first_vertex: 0,
            });
            cursor_x_px += allocation.advance_width_px;
            last_glyph_id = Some(allocation.id);
        }
        (glyphs, cursor_x_px)
    }

    fn assert_raw_glyphs_equal(actual: &[Glyph], expected: &[Glyph]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.chr, expected.chr);
            assert_eq!(actual.pos.x.to_bits(), expected.pos.x.to_bits());
            assert!(actual.pos.y.is_nan() && expected.pos.y.is_nan());
            assert_eq!(
                actual.advance_width.to_bits(),
                expected.advance_width.to_bits()
            );
            assert_eq!(actual.line_height.to_bits(), expected.line_height.to_bits());
            assert_eq!(
                actual.font_face_height.to_bits(),
                expected.font_face_height.to_bits()
            );
            assert_eq!(
                actual.font_face_ascent.to_bits(),
                expected.font_face_ascent.to_bits()
            );
            assert_eq!(actual.font_height.to_bits(), expected.font_height.to_bits());
            assert_eq!(actual.font_ascent.to_bits(), expected.font_ascent.to_bits());
            assert_eq!(actual.uv_rect, expected.uv_rect);
        }
    }

    #[test]
    fn test_zero_max_width() {
        let pixels_per_point = 1.0;
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let mut layout_job = LayoutJob::single_section("W".into(), TextFormat::default());
        layout_job.wrap.max_width = 0.0;
        let galley = layout(&mut fonts, pixels_per_point, layout_job.into());
        assert_eq!(galley.rows.len(), 1);
    }

    #[test]
    fn test_truncate_with_newline() {
        // No matter where we wrap, we should be appending the newline character.

        let pixels_per_point = 1.0;

        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let text_format = TextFormat {
            font_id: FontId::monospace(12.0),
            ..Default::default()
        };

        for text in ["Hello\nworld", "\nfoo"] {
            for break_anywhere in [false, true] {
                for max_width in [0.0, 5.0, 10.0, 20.0, f32::INFINITY] {
                    let mut layout_job =
                        LayoutJob::single_section(text.into(), text_format.clone());
                    layout_job.wrap.max_width = max_width;
                    layout_job.wrap.max_rows = 1;
                    layout_job.wrap.break_anywhere = break_anywhere;

                    let galley = layout(&mut fonts, pixels_per_point, layout_job.into());

                    assert!(galley.elided);
                    assert_eq!(galley.rows.len(), 1);
                    let row_text = galley.rows[0].text();
                    assert!(
                        row_text.ends_with('…'),
                        "Expected row to end with `…`, got {row_text:?} when line-breaking the text {text:?} with max_width {max_width} and break_anywhere {break_anywhere}.",
                    );
                }
            }
        }

        {
            let mut layout_job = LayoutJob::single_section("Hello\nworld".into(), text_format);
            layout_job.wrap.max_width = 50.0;
            layout_job.wrap.max_rows = 1;
            layout_job.wrap.break_anywhere = false;

            let galley = layout(&mut fonts, pixels_per_point, layout_job.into());

            assert!(galley.elided);
            assert_eq!(galley.rows.len(), 1);
            let row_text = galley.rows[0].text();
            assert_eq!(row_text, "Hello…");
        }
    }

    #[test]
    fn test_cjk() {
        let pixels_per_point = 1.0;
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let mut layout_job = LayoutJob::single_section(
            "日本語とEnglishの混在した文章".into(),
            TextFormat::default(),
        );
        layout_job.wrap.max_width = 90.0;
        let galley = layout(&mut fonts, pixels_per_point, layout_job.into());
        assert_eq!(
            galley.rows.iter().map(|row| row.text()).collect::<Vec<_>>(),
            vec!["日本語と", "Englishの混在", "した文章"]
        );
    }

    #[test]
    fn test_pre_cjk() {
        let pixels_per_point = 1.0;
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let mut layout_job = LayoutJob::single_section(
            "日本語とEnglishの混在した文章".into(),
            TextFormat::default(),
        );
        layout_job.wrap.max_width = 110.0;
        let galley = layout(&mut fonts, pixels_per_point, layout_job.into());
        assert_eq!(
            galley.rows.iter().map(|row| row.text()).collect::<Vec<_>>(),
            vec!["日本語とEnglish", "の混在した文章"]
        );
    }

    #[test]
    fn test_truncate_width() {
        let pixels_per_point = 1.0;
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let mut layout_job =
            LayoutJob::single_section("# DNA\nMore text".into(), TextFormat::default());
        layout_job.wrap.max_width = f32::INFINITY;
        layout_job.wrap.max_rows = 1;
        layout_job.round_output_to_gui = false;
        let galley = layout(&mut fonts, pixels_per_point, layout_job.into());
        assert!(galley.elided);
        assert_eq!(
            galley.rows.iter().map(|row| row.text()).collect::<Vec<_>>(),
            vec!["# DNA…"]
        );
        let row = &galley.rows[0];
        assert_eq!(row.pos, Pos2::ZERO);
        assert_eq!(row.rect().max.x, row.glyphs.last().unwrap().max_x());
    }

    #[test]
    fn test_truncate_with_pixels_per_point() {
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());

        for pixels_per_point in [
            0.33, 0.5, 0.67, 1.0, 1.25, 1.33, 1.5, 1.75, 2.0, 3.0, 4.0, 5.0,
        ] {
            for ch in ['W', 'A', 'n', 't', 'i'] {
                let target_width = 50.0;
                let text = (0..20).map(|_| ch).collect::<String>();

                let mut job = LayoutJob::single_section(text, TextFormat::default());
                job.wrap.max_width = target_width;
                job.wrap.max_rows = 1;
                let elided_galley = layout(&mut fonts, pixels_per_point, job.into());
                assert!(elided_galley.elided);

                let test_galley = layout(
                    &mut fonts,
                    pixels_per_point,
                    Arc::new(LayoutJob::single_section(
                        (0..elided_galley.rows[0].char_count_excluding_newline())
                            .map(|_| ch)
                            .chain(std::iter::once('…'))
                            .collect::<String>(),
                        TextFormat::default(),
                    )),
                );

                assert!(elided_galley.size().x >= 0.0);
                assert!(elided_galley.size().x <= target_width);
                assert!(test_galley.size().x > target_width);
            }
        }
    }

    #[test]
    fn test_empty_row() {
        let pixels_per_point = 1.0;
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());

        let font_id = FontId::default();
        let font_height = fonts
            .font(&font_id.family)
            .styled_metrics(pixels_per_point, font_id.size, &VariationCoords::default())
            .row_height;

        let job = LayoutJob::simple(String::new(), font_id, Color32::WHITE, f32::INFINITY);

        let galley = layout(&mut fonts, pixels_per_point, job.into());

        assert_eq!(galley.rows.len(), 1, "Expected one row");
        assert_eq!(
            galley.rows[0].row.glyphs.len(),
            0,
            "Expected no glyphs in the empty row"
        );
        assert_eq!(
            galley.size(),
            Vec2::new(0.0, font_height.round()),
            "Unexpected galley size"
        );
        assert_eq!(
            galley.intrinsic_size(),
            Vec2::new(0.0, font_height.round()),
            "Unexpected intrinsic size"
        );
    }

    #[test]
    fn test_end_with_newline() {
        let pixels_per_point = 1.0;
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());

        let font_id = FontId::default();
        let font_height = fonts
            .font(&font_id.family)
            .styled_metrics(pixels_per_point, font_id.size, &VariationCoords::default())
            .row_height;

        let job = LayoutJob::simple("Hi!\n".to_owned(), font_id, Color32::WHITE, f32::INFINITY);

        let galley = layout(&mut fonts, pixels_per_point, job.into());

        assert_eq!(galley.rows.len(), 2, "Expected two rows");
        assert_eq!(
            galley.rows[1].row.glyphs.len(),
            0,
            "Expected no glyphs in the empty row"
        );
        assert_eq!(
            galley.size().round(),
            Vec2::new(17.0, font_height.round() * 2.0),
            "Unexpected galley size"
        );
        assert_eq!(
            galley.intrinsic_size().round(),
            Vec2::new(17.0, font_height.round() * 2.0),
            "Unexpected intrinsic size"
        );
    }

    #[test]
    fn unwrapped_chunks_match_full_glyph_baseline_at_every_utf8_split() {
        for pixels_per_point in [1.0, 1.5, 2.0] {
            let text = "AVa\u{301}lue 前方";
            let format = TextFormat {
                font_id: FontId::monospace(14.0),
                ..Default::default()
            };
            let mut baseline_fonts =
                FontsImpl::new(TextOptions::default(), FontDefinitions::default());
            let expected =
                pristine_reference_glyphs(&mut baseline_fonts, pixels_per_point, &format, text);

            let mut stream_fonts =
                FontsImpl::new(TextOptions::default(), FontDefinitions::default());
            let identity = stream_fonts.layout_identity();
            for split in text
                .char_indices()
                .map(|(offset, _)| offset)
                .chain(std::iter::once(text.len()))
            {
                let mut continuation = None;
                let first = &text[..split];
                let second = &text[split..];
                let mut actual = Vec::new();
                let first_batch = layout_unwrapped_chunk(
                    &mut stream_fonts,
                    pixels_per_point,
                    Arc::clone(&identity),
                    format.clone(),
                    99,
                    0,
                    first,
                    second.is_empty(),
                    continuation,
                    4096,
                )
                .expect("valid unwrapped chunk");
                actual.extend(first_batch.glyphs);
                continuation = first_batch.continuation;
                if !second.is_empty() {
                    let second_batch = layout_unwrapped_chunk(
                        &mut stream_fonts,
                        pixels_per_point,
                        Arc::clone(&identity),
                        format.clone(),
                        99,
                        split as u64,
                        second,
                        true,
                        continuation,
                        4096,
                    )
                    .expect("valid resumed chunk");
                    actual.extend(second_batch.glyphs);
                    assert!(second_batch.continuation.is_none());
                }
                assert_raw_glyphs_equal(&actual, &expected);
            }
        }
    }

    #[test]
    fn unwrapped_chunk_budget_resumes_without_empty_progress() {
        let text = "AVAVAVAV";
        let format = TextFormat::default();
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let identity = fonts.layout_identity();
        let mut continuation = None;
        let mut offset = 0usize;
        let mut glyph_count = 0;
        let mut actual = Vec::new();
        while offset < text.len() {
            let batch = layout_unwrapped_chunk(
                &mut fonts,
                1.5,
                Arc::clone(&identity),
                format.clone(),
                123,
                offset as u64,
                &text[offset..],
                true,
                continuation,
                2,
            )
            .expect("budgeted chunk should progress");
            assert!(batch.consumed_bytes > 0);
            assert!(batch.glyphs.len() <= 2);
            glyph_count += batch.glyphs.len();
            actual.extend(batch.glyphs);
            offset += batch.consumed_bytes;
            continuation = batch.continuation;
        }
        assert_eq!(glyph_count, text.chars().count());
        let mut baseline_fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let expected = pristine_reference_glyphs(&mut baseline_fonts, 1.5, &format, text);
        assert_raw_glyphs_equal(&actual, &expected);
    }

    #[test]
    fn unwrapped_chunk_rejects_identity_offsets_formats_scales_and_limits() {
        let format = TextFormat::default();
        let seed = |fonts: &mut FontsImpl| {
            let identity = fonts.layout_identity();
            layout_unwrapped_chunk(
                fonts,
                1.0,
                Arc::clone(&identity),
                format.clone(),
                17,
                0,
                "A",
                false,
                None,
                1,
            )
            .expect("initial chunk")
            .continuation
            .expect("continuation")
        };

        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let continuation = seed(&mut fonts);
        let identity = fonts.layout_identity();
        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                1.0,
                identity,
                format.clone(),
                18,
                1,
                "V",
                true,
                Some(continuation),
                2,
            )
            .unwrap_err(),
            UnwrappedLayoutError::ChangedLayoutKey
        );

        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let continuation = seed(&mut fonts);
        let identity = fonts.layout_identity();
        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                1.0,
                identity,
                format.clone(),
                17,
                2,
                "V",
                true,
                Some(continuation),
                2,
            )
            .unwrap_err(),
            UnwrappedLayoutError::ChangedLayoutKey
        );

        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let continuation = seed(&mut fonts);
        let identity = fonts.layout_identity();
        let mut changed_format = format.clone();
        changed_format.font_id.size += 1.0;
        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                1.0,
                identity,
                changed_format,
                17,
                1,
                "V",
                true,
                Some(continuation),
                2,
            )
            .unwrap_err(),
            UnwrappedLayoutError::ChangedLayoutKey
        );

        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let continuation = seed(&mut fonts);
        let identity = fonts.layout_identity();
        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                1.5,
                Arc::clone(&identity),
                format.clone(),
                17,
                1,
                "V",
                true,
                Some(continuation),
                2,
            )
            .unwrap_err(),
            UnwrappedLayoutError::ChangedLayoutKey
        );

        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                0.0,
                Arc::clone(&identity),
                format.clone(),
                17,
                2,
                "V",
                true,
                None,
                2,
            )
            .unwrap_err(),
            UnwrappedLayoutError::InvalidFormat
        );
        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                1.0,
                Arc::clone(&identity),
                format.clone(),
                17,
                u64::MAX,
                "A",
                true,
                None,
                2,
            )
            .unwrap_err(),
            UnwrappedLayoutError::OffsetOverflow
        );
        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                1.0,
                Arc::clone(&identity),
                format.clone(),
                17,
                0,
                "A",
                true,
                None,
                0,
            )
            .unwrap_err(),
            UnwrappedLayoutError::OutputBudgetOutOfRange
        );
        let oversized = "A".repeat(96 * 1024 + 1);
        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                1.0,
                Arc::clone(&identity),
                format.clone(),
                17,
                0,
                &oversized,
                true,
                None,
                1,
            )
            .unwrap_err(),
            UnwrappedLayoutError::InputChunkTooLarge
        );
        let mut replacement = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let replacement_identity = replacement.layout_identity();
        let mut original = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let continuation = seed(&mut original);
        assert_eq!(
            layout_unwrapped_chunk(
                &mut replacement,
                1.0,
                replacement_identity,
                format.clone(),
                17,
                2,
                "V",
                true,
                Some(continuation),
                2,
            )
            .unwrap_err(),
            UnwrappedLayoutError::ChangedLayoutKey
        );
    }

    #[test]
    fn unwrapped_chunk_reports_empty_final_and_budgeted_final_states() {
        let format = TextFormat::default();
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let identity = fonts.layout_identity();
        let empty = layout_unwrapped_chunk(
            &mut fonts,
            1.0,
            identity,
            format.clone(),
            41,
            0,
            "",
            true,
            None,
            1,
        )
        .expect("empty final is a valid close");
        assert!(empty.glyphs.is_empty());
        assert!(empty.continuation.is_none());
        assert_eq!(empty.status, UnwrappedLayoutStatus::Complete);

        let identity = fonts.layout_identity();
        let first = layout_unwrapped_chunk(
            &mut fonts,
            1.0,
            identity.clone(),
            format.clone(),
            42,
            0,
            "AV",
            true,
            None,
            1,
        )
        .expect("budgeted final makes progress");
        assert_eq!(first.consumed_bytes, 1);
        assert_eq!(first.status, UnwrappedLayoutStatus::NeedMoreInput);
        assert!(first.summary.is_none());
        let second = layout_unwrapped_chunk(
            &mut fonts,
            1.0,
            identity,
            format.clone(),
            42,
            1,
            "V",
            true,
            first.continuation,
            1,
        )
        .expect("budgeted final resumes");
        assert_eq!(second.consumed_bytes, 1);
        assert_eq!(second.status, UnwrappedLayoutStatus::Complete);
        assert!(second.continuation.is_none());
        assert!(second.summary.is_some());

        let identity = fonts.layout_identity();
        let empty_nonfinal =
            layout_unwrapped_chunk(&mut fonts, 1.0, identity, format, 43, 0, "", false, None, 1)
                .expect("empty non-final remains resumable");
        assert_eq!(empty_nonfinal.status, UnwrappedLayoutStatus::NeedMoreInput);
        assert!(empty_nonfinal.summary.is_none());
    }

    #[test]
    fn unwrapped_checkpoint_clone_replays_utf8_seams_without_glyph_storage() {
        let text = "Á前V";
        let format = TextFormat::default();
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let identity = fonts.layout_identity();
        for split in text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(text.len()))
        {
            let first = layout_unwrapped_chunk(
                &mut fonts,
                1.25,
                Arc::clone(&identity),
                format.clone(),
                91,
                10,
                &text[..split],
                false,
                None,
                4096,
            )
            .expect("first seam chunk");
            let checkpoint = first.continuation.expect("checkpoint at seam");
            let cloned = checkpoint.clone();
            let left = layout_unwrapped_chunk(
                &mut fonts,
                1.25,
                Arc::clone(&identity),
                format.clone(),
                91,
                10 + split as u64,
                &text[split..],
                true,
                Some(checkpoint),
                4096,
            )
            .expect("original checkpoint replay");
            let right = layout_unwrapped_chunk(
                &mut fonts,
                1.25,
                Arc::clone(&identity),
                format.clone(),
                91,
                10 + split as u64,
                &text[split..],
                true,
                Some(cloned),
                4096,
            )
            .expect("cloned checkpoint replay");
            assert_raw_glyphs_equal(&left.glyphs, &right.glyphs);
            assert_eq!(left.consumed_bytes, right.consumed_bytes);
            assert_eq!(
                left.summary.as_ref().map(|s| s.byte_span()),
                right.summary.as_ref().map(|s| s.byte_span())
            );
            assert_eq!(
                left.summary.as_ref().map(|s| s.precise_width()),
                right.summary.as_ref().map(|s| s.precise_width())
            );
        }
    }

    #[test]
    fn unwrapped_summary_reports_exact_span_and_precise_pen() {
        for (pixels_per_point, spacing) in [(1.25, -50.0), (1.75, 0.2)] {
            let text = "AVálue";
            let format = TextFormat {
                font_id: FontId::monospace(14.0),
                extra_letter_spacing: spacing,
                ..Default::default()
            };
            let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
            let identity = fonts.layout_identity();
            let batch = layout_unwrapped_chunk(
                &mut fonts,
                pixels_per_point,
                identity,
                format.clone(),
                700,
                37,
                text,
                true,
                None,
                4096,
            )
            .expect("complete chunk");
            let summary = batch.summary.expect("summary only on final pass");
            assert_eq!(summary.byte_span(), 37..(37 + text.len() as u64));
            assert!(summary.precise_width().is_finite());

            let mut reference_fonts =
                FontsImpl::new(TextOptions::default(), FontDefinitions::default());
            let (_reference_glyphs, reference_pen_px) =
                pristine_reference_shape(&mut reference_fonts, pixels_per_point, &format, text);
            assert_eq!(
                summary.precise_width().to_bits(),
                (reference_pen_px / pixels_per_point).to_bits()
            );
            assert_eq!(summary.start_byte(), 37);
            assert_eq!(summary.end_byte(), 37 + text.len() as u64);
            assert!(
                summary
                    .validate_for_layout(
                        700,
                        37..(37 + text.len() as u64),
                        &format,
                        pixels_per_point,
                        &fonts.layout_identity(),
                    )
                    .is_ok()
            );
            assert!(
                summary
                    .validate_for_layout(
                        701,
                        37..(37 + text.len() as u64),
                        &format,
                        pixels_per_point,
                        &fonts.layout_identity(),
                    )
                    .is_err()
            );
            assert!(
                summary
                    .validate_for_layout(
                        700,
                        38..(38 + text.len() as u64),
                        &format,
                        pixels_per_point,
                        &fonts.layout_identity(),
                    )
                    .is_err()
            );
            let mut changed_format = format.clone();
            changed_format.extra_letter_spacing += 1.0;
            assert!(
                summary
                    .validate_for_layout(
                        700,
                        37..(37 + text.len() as u64),
                        &changed_format,
                        pixels_per_point,
                        &fonts.layout_identity(),
                    )
                    .is_err()
            );
            assert!(
                summary
                    .validate_for_layout(
                        700,
                        37..(37 + text.len() as u64),
                        &format,
                        pixels_per_point + 0.5,
                        &fonts.layout_identity(),
                    )
                    .is_err()
            );
            let replacement_fonts =
                FontsImpl::new(TextOptions::default(), FontDefinitions::default());
            assert!(
                summary
                    .validate_for_layout(
                        700,
                        37..(37 + text.len() as u64),
                        &format,
                        pixels_per_point,
                        &replacement_fonts.layout_identity(),
                    )
                    .is_err()
            );
            if spacing < -10.0 {
                assert!(summary.precise_width() < 0.0);
            } else {
                assert!(summary.precise_width() >= 0.0);
            }

            let mut budget_fonts =
                FontsImpl::new(TextOptions::default(), FontDefinitions::default());
            let budget_identity = budget_fonts.layout_identity();
            let mut continuation = None;
            let mut offset = 0;
            let budget_summary = loop {
                let batch = layout_unwrapped_chunk(
                    &mut budget_fonts,
                    pixels_per_point,
                    Arc::clone(&budget_identity),
                    format.clone(),
                    700,
                    37 + offset as u64,
                    &text[offset..],
                    true,
                    continuation,
                    1,
                )
                .expect("budgeted summary chunk");
                offset += batch.consumed_bytes;
                if batch.status == UnwrappedLayoutStatus::Complete {
                    break batch.summary.expect("budgeted final summary");
                }
                continuation = batch.continuation;
            };
            assert_eq!(
                summary.precise_width().to_bits(),
                budget_summary.precise_width().to_bits()
            );
        }

        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let identity = fonts.layout_identity();
        let empty = layout_unwrapped_chunk(
            &mut fonts,
            1.0,
            identity,
            TextFormat::default(),
            701,
            42,
            "",
            true,
            None,
            1,
        )
        .expect("empty final");
        assert_eq!(
            empty.summary.expect("empty final summary").byte_span(),
            42..42
        );
    }
}
