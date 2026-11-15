//! Digit layout for beatoraja `floatvalue` (decimal numbers drawn with a digit image).
//!
//! Only the layout is decided here: which cell of the digit image goes into
//! which slot. Drawing is done by `float_value_render_items`.

/// What one drawn slot shows before it is mapped to a cell of the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::skin) enum FloatGlyph {
    Digit(u8),
    /// Leading-zero filler used by `zeropadding` 2.
    BackZero,
    Point,
    Sign,
}

/// The cell groups of a `floatvalue` image, chosen from the number of cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::skin) struct FloatCellSet {
    /// Cells in one group.
    pub(in crate::skin) stride: i32,
    pub(in crate::skin) groups: i32,
    kind: FloatSetKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FloatSetKind {
    Sign26,
    Pair24,
    Pair22,
    Plain12,
    Plain11,
}

impl FloatCellSet {
    pub(in crate::skin) fn from_cell_count(cells: i32) -> Option<Self> {
        let (stride, kind) = if cells > 0 && cells % 26 == 0 {
            (26, FloatSetKind::Sign26)
        } else if cells > 0 && cells % 24 == 0 {
            (24, FloatSetKind::Pair24)
        } else if cells > 0 && cells % 22 == 0 {
            (22, FloatSetKind::Pair22)
        } else if cells > 0 && cells % 12 == 0 {
            (12, FloatSetKind::Plain12)
        } else if cells > 0 && cells % 11 == 0 {
            (11, FloatSetKind::Plain11)
        } else if cells >= 12 {
            // Cells beyond a multiple of 12 do not form a group.
            (12, FloatSetKind::Plain12)
        } else {
            return None;
        };
        Some(Self { stride, groups: (cells / stride).max(1), kind })
    }

    /// Only 26-cell images have a sign cell, so `isSignvisible` matters only there.
    pub(in crate::skin) fn has_sign_cell(self) -> bool {
        self.kind == FloatSetKind::Sign26
    }

    /// Cell index inside one group. `negative` selects the minus half of 26/24/22 groups.
    pub(in crate::skin) fn cell(self, glyph: FloatGlyph, negative: bool) -> i32 {
        let (digit_base, back_zero, point, sign, minus_offset) = match self.kind {
            FloatSetKind::Sign26 => (0, Some(10), 11, Some(12), 13),
            FloatSetKind::Pair24 => (0, Some(10), 11, None, 12),
            FloatSetKind::Pair22 => (0, None, 10, None, 11),
            FloatSetKind::Plain12 => (0, Some(10), 11, None, 0),
            FloatSetKind::Plain11 => (0, None, 10, None, 0),
        };
        let offset = if negative { minus_offset } else { 0 };
        offset
            + match glyph {
                FloatGlyph::Digit(digit) => digit_base + i32::from(digit),
                // Sets without a back-zero cell use the group's own "0".
                FloatGlyph::BackZero => back_zero.unwrap_or(digit_base),
                FloatGlyph::Point => point,
                FloatGlyph::Sign => sign.unwrap_or(point),
            }
    }

    /// 12- and 11-cell groups have no minus half, so negative values are drawn as absolute values.
    pub(in crate::skin) fn has_minus_half(self) -> bool {
        matches!(self.kind, FloatSetKind::Sign26 | FloatSetKind::Pair24 | FloatSetKind::Pair22)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::skin) struct FloatNumberLayout {
    /// Glyphs from left to right.
    pub(in crate::skin) glyphs: Vec<FloatGlyph>,
    /// Slots left unused at the right end (only with `zeropadding` 0).
    pub(in crate::skin) empty_slots: usize,
    /// The value (after gain) is negative: use the minus half of the cell group.
    pub(in crate::skin) negative: bool,
}

/// Normalizes `iketa` / `fketa`: negatives become 0, and a total of 8 or more is cut to 8.
pub(in crate::skin) fn normalize_float_digits(iketa: i32, fketa: i32) -> (usize, usize) {
    let mut iketa = iketa.max(0);
    let mut fketa = fketa.max(0);
    if iketa >= 8 || fketa >= 8 || iketa + fketa >= 8 {
        fketa = fketa.min(8);
        iketa = 8 - fketa;
    }
    (iketa as usize, fketa as usize)
}

/// Lays out `value` (already multiplied by gain). `None` means nothing is drawn.
pub(in crate::skin) fn layout_float_number(
    value: f32,
    iketa: i32,
    fketa: i32,
    sign_visible: bool,
    zeropadding: i32,
    sign_cell_available: bool,
) -> Option<FloatNumberLayout> {
    if !value.is_finite() || value == f32::MIN || value == f32::MAX {
        return None;
    }
    let (iketa, fketa) = normalize_float_digits(iketa, fketa);
    let sign_visible = sign_visible && sign_cell_available;
    let point_slots = usize::from(fketa >= 1);
    let total_slots = usize::from(sign_visible) + iketa + fketa + point_slots;
    if total_slots == 0 {
        return None;
    }
    let negative = value < 0.0;
    let magnitude = f64::from(value.abs());
    let int_part = magnitude.floor();
    // Truncate, never round; f32 noise may lower the last digit.
    let scale = 10_f64.powi(fketa as i32);
    let frac = ((magnitude - int_part) * scale).floor().min(scale - 1.0).max(0.0) as u64;
    let int_part = int_part as u64;

    let iketa_limit = 10_u64.pow(iketa as u32);
    let show_sign = sign_visible && int_part < iketa_limit;
    // A visible sign slot takes the overflowing top digit when no sign is drawn.
    let int_slots = iketa + usize::from(sign_visible && !show_sign);
    let int_digits = if int_slots == 0 { 0 } else { int_part % 10_u64.pow(int_slots as u32) };

    let mut glyphs = Vec::with_capacity(total_slots);
    if show_sign {
        glyphs.push(FloatGlyph::Sign);
    }
    if int_slots > 0 {
        let text = int_digits.to_string();
        let zeropadding = zeropadding.clamp(0, 2);
        if zeropadding > 0 {
            for _ in text.len()..int_slots {
                glyphs.push(if zeropadding == 1 {
                    FloatGlyph::Digit(0)
                } else {
                    FloatGlyph::BackZero
                });
            }
        }
        glyphs.extend(text.bytes().map(|byte| FloatGlyph::Digit(byte - b'0')));
    }
    if fketa >= 1 {
        glyphs.push(FloatGlyph::Point);
        let text = format!("{frac:0width$}", width = fketa);
        glyphs.extend(text.bytes().map(|byte| FloatGlyph::Digit(byte - b'0')));
    }
    let empty_slots = total_slots.saturating_sub(glyphs.len());
    Some(FloatNumberLayout { glyphs, empty_slots, negative })
}

#[cfg(test)]
mod tests {
    use super::FloatGlyph::*;
    use super::*;

    fn d(text: &str) -> Vec<FloatGlyph> {
        text.chars()
            .map(|c| match c {
                '.' => Point,
                '+' => Sign,
                '_' => BackZero,
                c => Digit(c.to_digit(10).unwrap() as u8),
            })
            .collect()
    }

    fn lay(value: f32, i: i32, f: i32, sign: bool, zp: i32) -> FloatNumberLayout {
        layout_float_number(value, i, f, sign, zp, true).unwrap()
    }

    #[test]
    fn digit_counts_are_normalized() {
        assert_eq!(normalize_float_digits(-1, -2), (0, 0));
        assert_eq!(normalize_float_digits(6, 5), (3, 5));
        assert_eq!(normalize_float_digits(3, 8), (0, 8));
        assert_eq!(normalize_float_digits(3, 2), (3, 2));
        assert_eq!(normalize_float_digits(9, 0), (8, 0));
    }

    #[test]
    fn spec_examples_without_sign() {
        let l = lay(0.0, 3, 2, false, 0);
        assert_eq!((l.glyphs, l.empty_slots), (d("0.00"), 2));
        assert_eq!(lay(0.0, 3, 2, false, 1).glyphs, d("000.00"));
        assert_eq!(lay(0.0, 3, 2, false, 2).glyphs, d("__0.00"));
        let l = lay(1.5, 3, 2, false, 0);
        assert_eq!((l.glyphs, l.empty_slots), (d("1.50"), 2));
        assert_eq!(lay(1.5, 3, 2, false, 1).glyphs, d("001.50"));
        assert_eq!(lay(1.5, 3, 2, false, 2).glyphs, d("__1.50"));
        let l = lay(12.345, 3, 2, false, 0);
        assert_eq!((l.glyphs, l.empty_slots), (d("12.34"), 1));
        assert_eq!(lay(12.345, 3, 2, false, 1).glyphs, d("012.34"));
        assert_eq!(lay(12.345, 3, 2, false, 2).glyphs, d("_12.34"));
        let l = lay(123.456, 3, 2, false, 0);
        assert_eq!((l.glyphs, l.empty_slots), (d("123.45"), 0));
    }

    #[test]
    fn overflow_drops_upper_digits_and_truncates() {
        assert_eq!(lay(9999.9, 3, 2, false, 0).glyphs, d("999.90"));
        assert_eq!(lay(1.35, 3, 1, false, 0).glyphs, d("1.3"));
        assert_eq!(lay(0.75, 0, 2, false, 0).glyphs, d(".75"));
        assert_eq!(lay(-2.5, 1, 1, false, 0).glyphs, d("2.5"));
        assert!(lay(-2.5, 1, 1, false, 0).negative);
    }

    #[test]
    fn sign_slot_holds_sign_or_overflow_digit() {
        let l = lay(1.5, 3, 2, true, 0);
        assert_eq!((l.glyphs, l.empty_slots), (d("+1.50"), 2));
        let l = lay(9999.9, 3, 2, true, 0);
        assert_eq!((l.glyphs, l.empty_slots), (d("9999.90"), 0));
        assert_eq!(lay(1.5, 3, 2, true, 1).glyphs, d("+001.50"));
        assert_eq!(lay(0.0, 0, 0, true, 0).glyphs, d("+"));
    }

    #[test]
    fn sign_is_ignored_without_a_sign_cell() {
        let l = layout_float_number(1.5, 3, 2, true, 0, false).unwrap();
        assert_eq!(l.glyphs, d("1.50"));
        assert_eq!(l.empty_slots, 2);
    }

    #[test]
    fn hidden_values_and_empty_layouts_draw_nothing() {
        assert!(layout_float_number(f32::NAN, 3, 2, false, 0, true).is_none());
        assert!(layout_float_number(f32::INFINITY, 3, 2, false, 0, true).is_none());
        assert!(layout_float_number(f32::MAX, 3, 2, false, 0, true).is_none());
        assert!(layout_float_number(f32::MIN, 3, 2, false, 0, true).is_none());
        assert!(layout_float_number(1.0, 0, 0, false, 0, true).is_none());
    }

    #[test]
    fn cell_sets_follow_the_cell_count() {
        assert!(FloatCellSet::from_cell_count(11 - 1).is_none());
        let s26 = FloatCellSet::from_cell_count(52).unwrap();
        assert_eq!((s26.stride, s26.groups), (26, 2));
        assert_eq!(s26.cell(Digit(3), false), 3);
        assert_eq!(s26.cell(Digit(3), true), 16);
        assert_eq!(s26.cell(BackZero, false), 10);
        assert_eq!(s26.cell(Point, false), 11);
        assert_eq!(s26.cell(Sign, false), 12);
        assert_eq!(s26.cell(BackZero, true), 23);
        assert_eq!(s26.cell(Point, true), 24);
        assert_eq!(s26.cell(Sign, true), 25);
        let s24 = FloatCellSet::from_cell_count(24).unwrap();
        assert_eq!(s24.cell(Point, true), 23);
        assert_eq!(s24.cell(Digit(0), true), 12);
        let s22 = FloatCellSet::from_cell_count(22).unwrap();
        assert_eq!(s22.cell(Point, false), 10);
        assert_eq!(s22.cell(Digit(1), true), 12);
        assert_eq!(s22.cell(Point, true), 21);
        assert_eq!(s22.cell(BackZero, true), 11);
        let s12 = FloatCellSet::from_cell_count(12).unwrap();
        assert!(!s12.has_minus_half());
        assert_eq!(s12.cell(Point, false), 11);
        let s11 = FloatCellSet::from_cell_count(11).unwrap();
        assert_eq!(s11.cell(BackZero, false), 0);
        assert_eq!(s11.cell(Point, false), 10);
        // 13 cells: read as one group of 12.
        let odd = FloatCellSet::from_cell_count(13).unwrap();
        assert_eq!((odd.stride, odd.groups), (12, 1));
    }
}
