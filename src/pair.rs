//! Distances between glyph outlines (DESIGN.md §1.6.6): what spacing by the true gap and by the white area
//! between two letters needs, outside any layout. `Ui::faces` hands the state a `Faces` (the page's faces,
//! nothing else of the text system); a `Glyph` is one character's ink at a size, row by row, and
//! `gap`, `area`, `spaced` and `spaced_area` measure or place a pair.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::Format;

use crate::el::Font;

/// The page's faces, for glyph outlines (`Ui::faces`): cheap to clone, kept in the state.
#[derive(Clone)]
pub struct Faces(Rc<Inner>);
struct Inner { faces: Vec<(Arc<Vec<u8>>, bool)>, roles: Rc<Cell<[u8; 3]>>, scaler: RefCell<ScaleContext>, made: RefCell<Made> }
/// The glyphs made so far, by face, character and size bits.
type Made = HashMap<(u8, char, u32), Option<Rc<Glyph>>>;
impl std::fmt::Debug for Faces {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Faces({})", self.0.faces.len()) }
}

impl Faces {
    /// From each face's bytes, and whether it is a pixel face (cut at half coverage, as on the page).
    /// `roles`: what the page's root resolves the roles to (its `El::fonts`), shared with the `Ui`.
    pub(crate) fn new(faces: Vec<(Arc<Vec<u8>>, bool)>, roles: Rc<Cell<[u8; 3]>>) -> Self { Self(Rc::new(Inner { faces, roles, scaler: RefCell::new(ScaleContext::new()), made: RefCell::new(HashMap::new()) })) }
    /// The face `font` is drawn in here: a role as the page's root resolves it.
    pub fn face_of(&self, font: Font) -> u8 { font.resolve(self.0.roles.get()) }
    /// `ch` in `font` at `px` px (logical px for a page's text; a role resolves as the page's root
    /// resolves it, by its `El::fonts`; inside a subtree with fonts of its own, name the face with
    /// `Font::Face`). Made once for each face, character and size. `None` for a face it does not have.
    pub fn glyph(&self, font: Font, ch: char, px: f32) -> Option<Rc<Glyph>> {
        let face = self.face_of(font);
        let key = (face, ch, px.to_bits());
        if let Some(g) = self.0.made.borrow().get(&key) { return g.clone(); }
        let g = self.make(face, ch, px).map(Rc::new);
        self.0.made.borrow_mut().insert(key, g.clone());
        g
    }
    fn make(&self, face: u8, ch: char, px: f32) -> Option<Glyph> {
        let (data, pixel) = self.0.faces.get(face as usize)?;
        let fref = swash::FontRef::from_index(data, 0)?;
        let gid = fref.charmap().map(ch);
        let advance = fref.glyph_metrics(&[]).scale(px).advance_width(gid);
        let mut sc = self.0.scaler.borrow_mut();
        let mut scaler = sc.builder(fref).size(px).hint(false).build();
        let img = Render::new(&[Source::Outline]).format(Format::Alpha).render(&mut scaler, gid)?;
        let (w, h) = (img.placement.width as usize, img.placement.height as usize);
        let cov = |x: usize, y: usize| { let c = img.data[y * w + x] as f32 / 255.0; if *pixel { if c >= 0.5 { 1.0 } else { 0.0 } } else { c } };
        // Each row's ink from its first to its last covered pixel, the edges placed within their pixels
        // by their coverage.
        let rows = (0..h).map(|y| {
            let first = (0..w).find(|&x| cov(x, y) > 0.0);
            let last = (0..w).rev().find(|&x| cov(x, y) > 0.0);
            match (first, last) {
                (Some(a), Some(b)) => [img.placement.left as f32 + a as f32 + 1.0 - cov(a, y), img.placement.left as f32 + b as f32 + cov(b, y)],
                _ => [f32::NAN; 2],
            }
        }).collect();
        Some(Glyph { advance, top: img.placement.top as f32, rows })
    }
}

/// One character's ink at a size: its advance, and for each pixel row from `top` (px above the baseline)
/// down, where its ink starts and ends (px from the pen; NaN for a row without ink).
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph { pub advance: f32, pub top: f32, pub rows: Vec<[f32; 2]> }
impl Glyph {
    /// Row `k`'s ink, if any.
    fn at(&self, k: usize) -> Option<[f32; 2]> { self.rows.get(k).copied().filter(|r| !r[0].is_nan()) }
    /// The centre of row `k`, px above the baseline.
    fn y(&self, k: usize) -> f32 { self.top - k as f32 - 0.5 }
    /// The ink's extent across, from the pen: (leftmost, rightmost).
    pub fn extent(&self) -> Option<(f32, f32)> {
        let v: Vec<[f32; 2]> = (0..self.rows.len()).filter_map(|k| self.at(k)).collect();
        (!v.is_empty()).then(|| (v.iter().map(|r| r[0]).fold(f32::INFINITY, f32::min), v.iter().map(|r| r[1]).fold(f32::NEG_INFINITY, f32::max)))
    }
}

/// The smallest distance between the ink of `a` and of `b` set `dx` px after it (from pen to pen), px;
/// less than 0 when they overlap (by how much, along a row). Infinity when either has no ink.
pub fn gap(a: &Glyph, b: &Glyph, dx: f32) -> f32 {
    // Rows of `b` in `a`'s numbering: b's row k is a's row k + shift.
    let shift = a.top - b.top;
    let mut over = 0.0f32;
    for k in 0..a.rows.len() {
        let Some(ra) = a.at(k) else { continue };
        let kb = k as f32 - shift;
        if kb.fract().abs() < 1e-3 && kb >= 0.0 && let Some(rb) = b.at(kb.round() as usize) {
            over = over.max(ra[1].min(rb[1] + dx) - ra[0].max(rb[0] + dx));
        }
    }
    if over > 0.0 { return -over; }
    // Each row of ink a segment: the nearest pair of segments, rows near each other first.
    let mut best = f32::INFINITY;
    for ka in 0..a.rows.len() {
        let Some(ra) = a.at(ka) else { continue };
        let ya = a.y(ka);
        for kb in 0..b.rows.len() {
            // Rows are a pixel tall: next to each other they touch.
            let dy = ((ya - b.y(kb)).abs() - 1.0).max(0.0);
            if dy * dy >= best { continue; }
            let Some(rb) = b.at(kb) else { continue };
            let h = (rb[0] + dx - ra[1]).max(ra[0] - rb[1] - dx).max(0.0);
            best = best.min(h * h + dy * dy);
        }
    }
    best.sqrt()
}

/// The white between `a` and `b` set `dx` px after it, px²: over the rows where both have ink, from the
/// end of `a`'s to the start of `b`'s.
pub fn area(a: &Glyph, b: &Glyph, dx: f32) -> f32 {
    let shift = a.top - b.top;
    (0..a.rows.len()).filter_map(|k| {
        let ra = a.at(k)?;
        let kb = k as f32 - shift;
        if kb < 0.0 { return None; }
        let rb = b.at(kb.round() as usize)?;
        Some((rb[0] + dx - ra[1]).max(0.0))
    }).sum()
}

/// Where `b` goes after `a` (pen to pen, px) for the ink to be `target` px apart at its closest.
pub fn spaced(a: &Glyph, b: &Glyph, target: f32) -> f32 { solve(a, b, target, gap) }
/// Where `b` goes after `a` (pen to pen, px) for `target` px² of white between them.
pub fn spaced_area(a: &Glyph, b: &Glyph, target: f32) -> f32 { solve(a, b, target, area) }

/// The `dx` at which `f` (growing with `dx`) reaches `target`, to 1/64 px.
fn solve(a: &Glyph, b: &Glyph, target: f32, f: fn(&Glyph, &Glyph, f32) -> f32) -> f32 {
    let (Some((al, ar)), Some((bl, br))) = (a.extent(), b.extent()) else { return a.advance };
    // From fully over each other to apart by more than the target.
    let (mut lo, mut hi) = (al - br, ar - bl + 1.0);
    while f(a, b, hi) < target { hi += (hi - lo).max(1.0); }
    while hi - lo > 1.0 / 64.0 {
        let mid = (lo + hi) * 0.5;
        if f(a, b, mid) < target { lo = mid; } else { hi = mid; }
    }
    hi
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bar(x0: f32, x1: f32, top: f32, h: usize) -> Glyph { Glyph { advance: x1 + 2.0, top, rows: vec![[x0, x1]; h] } }
    #[test]
    fn two_bars_are_as_far_apart_as_their_edges() {
        let (a, b) = (bar(0.0, 4.0, 10.0, 10), bar(1.0, 3.0, 10.0, 10));
        assert!((gap(&a, &b, 10.0) - 7.0).abs() < 1e-4);
        assert!((gap(&a, &b, 2.0) + 1.0).abs() < 1e-4, "overlapping by 1: {}", gap(&a, &b, 2.0));
        assert!((area(&a, &b, 10.0) - 70.0).abs() < 1e-3);
        assert!((spaced(&a, &b, 5.0) - 8.0).abs() < 0.02);
        assert!((spaced_area(&a, &b, 50.0) - 8.0).abs() < 0.02);
        // One above the other: the gap is the corner to corner distance.
        let c = bar(0.0, 2.0, 30.0, 10);
        let d = gap(&a, &c, 7.0);
        assert!((d - (3.0f32 * 3.0 + 10.0 * 10.0).sqrt()).abs() < 0.01, "{d}");
    }
}
