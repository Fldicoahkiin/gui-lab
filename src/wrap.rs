//! Text around the true outline of other things (DESIGN.md §1.2, "around"): a paragraph is broken
//! line by line, and each line is given only the parts of its band that no obstacle (widened by a
//! gap) covers, so the text follows a picture's or a shape's real silhouette rather than its box. A
//! band split in two by an obstacle takes a line on each side. No GPU here.
use std::hash::{Hash, Hasher};

/// The ink of an obstacle, row by row: for each logical-px row from `top`, the span `(x0, x1)` its ink
/// covers (`x0 > x1` for an empty row). Coordinates are those of the paragraph the text is set in.
#[derive(Clone, Debug, PartialEq)]
pub struct Silhouette { pub top: f32, pub rows: Vec<(f32, f32)> }

const EMPTY: (f32, f32) = (f32::INFINITY, f32::NEG_INFINITY);

impl Silhouette {
    /// A disc.
    pub fn circle(cx: f32, cy: f32, r: f32) -> Self {
        let top = (cy - r).floor();
        let n = (cy + r).ceil() - top;
        let rows = (0..n as usize).map(|i| {
            // The widest point of the row (the row's band nearest the centre).
            let (y0, y1) = (top + i as f32, top + i as f32 + 1.0);
            let dy = if cy < y0 { y0 - cy } else if cy > y1 { cy - y1 } else { 0.0 };
            if dy >= r { EMPTY } else { let w = (r * r - dy * dy).sqrt(); (cx - w, cx + w) }
        }).collect();
        Self { top, rows }
    }
    /// A rectangle with rounded corners.
    pub fn rounded(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Self {
        let r = radius.min(w * 0.5).min(h * 0.5).max(0.0);
        let top = y.floor();
        let n = (y + h).ceil() - top;
        let rows = (0..n as usize).map(|i| {
            let (y0, y1) = (top + i as f32, top + i as f32 + 1.0);
            if y1 <= y || y0 >= y + h { return EMPTY; }
            // Distance into a corner's band, from the row's nearest edge to the middle.
            let dy = if y1 < y + r { y + r - y1 } else if y0 > y + h - r { y0 - (y + h - r) } else { 0.0 };
            let inset = if dy > 0.0 { r - (r * r - dy.min(r) * dy.min(r)).sqrt() } else { 0.0 };
            (x + inset, x + w - inset)
        }).collect();
        Self { top, rows }
    }
    /// From a picture's alpha (`alpha(x, y)` for `iw` × `ih` texels), drawn at (x, y) with width `w`:
    /// each row's span is from the first to the last texel above half coverage.
    pub fn from_alpha(iw: u32, ih: u32, alpha: impl Fn(u32, u32) -> u8, x: f32, y: f32, w: f32) -> Self {
        let k = w / iw as f32;
        let h = ih as f32 * k;
        let top = y.floor();
        let n = (y + h).ceil() - top;
        let rows = (0..n as usize).map(|i| {
            let (y0, y1) = (top + i as f32, top + i as f32 + 1.0);
            let (t0, t1) = ((((y0 - y) / k).floor().max(0.0)) as u32, ((((y1 - y) / k).ceil()) as u32).min(ih));
            let mut span = EMPTY;
            for ty in t0..t1 {
                if let Some(a) = (0..iw).find(|&tx| alpha(tx, ty) >= 128) { span.0 = span.0.min(x + a as f32 * k); }
                if let Some(b) = (0..iw).rev().find(|&tx| alpha(tx, ty) >= 128) { span.1 = span.1.max(x + (b + 1) as f32 * k); }
            }
            span
        }).collect();
        Self { top, rows }
    }
    /// The same outline moved by (dx, dy).
    pub fn moved(&self, dx: f32, dy: f32) -> Self {
        // Rows are whole px; a fractional move rounds outward by keeping both neighbouring rows.
        let shift = dy.floor();
        let frac = dy - shift;
        let mut rows: Vec<(f32, f32)> = self.rows.iter().map(|&(a, b)| (a + dx, b + dx)).collect();
        if frac > 0.0 {
            rows.push(EMPTY);
            for i in (1..rows.len()).rev() { rows[i] = (rows[i].0.min(rows[i - 1].0), rows[i].1.max(rows[i - 1].1)); }
        }
        Self { top: self.top + shift, rows }
    }
    /// The span the ink covers anywhere in the band `y0..y1`, if any.
    pub fn span(&self, y0: f32, y1: f32) -> Option<(f32, f32)> {
        let a = ((y0 - self.top).floor().max(0.0)) as usize;
        let b = ((y1 - self.top).ceil().max(0.0) as usize).min(self.rows.len());
        let s = self.rows.get(a..b)?.iter().fold(EMPTY, |s, r| (s.0.min(r.0), s.1.max(r.1)));
        (s.0 <= s.1).then_some(s)
    }
    /// The span the ink grown by `g` all round (a disc of radius `g` swept along the outline) covers in
    /// the band `y0..y1`: rows inside the band grow by `g` sideways, rows above or below it by what is
    /// left of `g` at their distance, so the gap is the same all round the true outline.
    pub fn span_grown(&self, y0: f32, y1: f32, g: f32) -> Option<(f32, f32)> {
        let a = ((y0 - g - self.top).floor().max(0.0)) as usize;
        let b = ((y1 + g - self.top).ceil().max(0.0) as usize).min(self.rows.len());
        let mut s = EMPTY;
        for i in a..b {
            let r = self.rows[i];
            if r.0 > r.1 { continue; }
            let (ra, rb) = (self.top + i as f32, self.top + i as f32 + 1.0);
            let dy = (y0 - rb).max(ra - y1).max(0.0);
            if dy > g { continue; }
            let e = (g * g - dy * dy).max(0.0).sqrt();
            s = (s.0.min(r.0 - e), s.1.max(r.1 + e));
        }
        (s.0 <= s.1).then_some(s)
    }
    /// Whether the ink covers the point (x, y).
    pub fn covers(&self, x: f32, y: f32) -> bool {
        let i = (y - self.top).floor();
        i >= 0.0 && self.rows.get(i as usize).is_some_and(|r| x >= r.0 && x < r.1)
    }
}

/// What a paragraph flows around: obstacles, the gap kept from their ink, and the narrowest piece of
/// a line worth setting text in (logical px; 0 = three times the type size, `MIN_EM`).
/// The narrowest piece of a line worth setting text in, in ems, when `Around::min` is 0: a sliver
/// narrower than this gets no text.
pub const MIN_EM: f32 = 3.0;
/// Obstacles are placed on a grid this fine (logical px) before lines are broken around them, so a
/// sub-pixel move never moves a break: the lines change only when the outline has moved a whole step.
pub const STEP: f32 = 2.0;
#[derive(Clone, Debug, PartialEq)]
pub struct Around { pub obstacles: Vec<Silhouette>, pub gap: f32, pub min: f32 }

impl Around {
    pub fn new(obstacles: Vec<Silhouette>, gap: f32) -> Self { Self { obstacles, gap, min: 0.0 } }
    /// A key for caches: the same outlines and gap give the same lines.
    pub fn id(&self) -> u64 {
        let mut h = std::hash::DefaultHasher::new();
        self.gap.to_bits().hash(&mut h);
        self.min.to_bits().hash(&mut h);
        for o in &self.obstacles {
            o.top.to_bits().hash(&mut h);
            for r in &o.rows { r.0.to_bits().hash(&mut h); r.1.to_bits().hash(&mut h); }
        }
        h.finish()
    }
    /// The free pieces `(x0, x1)` of the band `y0..y1` across `0..width`, left to right.
    /// `min` is the narrowest piece kept (logical px).
    pub fn free(&self, width: f32, y0: f32, y1: f32, min: f32) -> Vec<(f32, f32)> {
        let g = self.gap;
        let mut blocked: Vec<(f32, f32)> = self.obstacles.iter().filter_map(|o| o.span_grown(y0, y1, g)).collect();
        blocked.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut out = vec![];
        let mut x = 0.0f32;
        for (a, b) in blocked {
            if a > x { out.push((x, a.min(width))); }
            x = x.max(b);
            if x >= width { break; }
        }
        if x < width { out.push((x, width)); }
        out.retain(|&(a, b)| b - a >= min);
        out
    }
}
