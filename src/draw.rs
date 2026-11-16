//! Drawings (DESIGN.md §1.26): an element whose content is marks in its own coordinate space — dots
//! and strokes — each made into a shape instance with a true outline (a stroke is a rounded box turned
//! along its direction, `Instance::xf`), so every material draws them and fronts loosen them like any
//! shape. A field becomes a drawing by `dots` (a dot per node, sized by the value) or `contours`
//! (marching squares). `el::canvas` puts one on the page; its input reports positions in its space.
use crate::el::Paint;
use crate::field::{Grid, Scalar};

/// One mark, in the drawing's space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mark {
    /// A round dot of radius `r` about (x, y).
    Dot { x: f32, y: f32, r: f32, paint: Paint },
    /// A stroke `w` wide from `a` to `b` with round ends.
    Line { a: (f32, f32), b: (f32, f32), w: f32, paint: Paint },
}

/// Marks in a `w × h` space; the element shows the space scaled to its width (its height follows).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawing { pub w: f32, pub h: f32, pub marks: Vec<Mark> }
impl Drawing {
    pub fn new(w: f32, h: f32) -> Self { Self { w, h, marks: Vec::new() } }
    pub fn dot(&mut self, x: f32, y: f32, r: f32, paint: Paint) { if r > 0.0 && paint.alpha > 0.0 { self.marks.push(Mark::Dot { x, y, r, paint }); } }
    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), w: f32, paint: Paint) { if w > 0.0 && paint.alpha > 0.0 { self.marks.push(Mark::Line { a, b, w, paint }); } }
    /// A polyline as strokes; `paint` is asked for each piece by its midpoint.
    pub fn polyline(&mut self, pts: &[(f32, f32)], w: f32, paint: impl Fn(f32, f32) -> Paint) {
        for p in pts.windows(2) { self.line(p[0], p[1], w, paint((p[0].0 + p[1].0) * 0.5, (p[0].1 + p[1].1) * 0.5)); }
    }
    /// A circle of radius `r` about (x, y) as `n` strokes.
    pub fn circle(&mut self, x: f32, y: f32, r: f32, n: usize, w: f32, paint: Paint) {
        let pts: Vec<(f32, f32)> = (0..=n).map(|i| { let a = i as f32 / n as f32 * std::f32::consts::TAU; (x + r * a.cos(), y + r * a.sin()) }).collect();
        self.polyline(&pts, w, |_, _| paint);
    }
    /// The iso-contours of `g` at each level as strokes.
    pub fn contours(&mut self, g: &Grid, levels: &[f32], w: f32, paint: impl Fn(f32, f32, f32) -> Paint) {
        for &l in levels { for c in g.contours(l) { self.polyline(&c, w, |x, y| paint(x, y, l)); } }
    }
    /// A dot at every node of a `pitch` lattice, its radius `size(value)` (none at 0).
    pub fn dots(&mut self, f: &impl Scalar, pitch: f32, size: impl Fn(f32) -> f32, paint: impl Fn(f32, f32, f32) -> Paint) {
        let (nx, ny) = ((self.w / pitch).floor() as usize, (self.h / pitch).floor() as usize);
        let (ox, oy) = ((self.w - (nx as f32 - 1.0) * pitch) * 0.5, (self.h - (ny as f32 - 1.0) * pitch) * 0.5);
        for j in 0..ny { for i in 0..nx {
            let (x, y) = (ox + i as f32 * pitch, oy + j as f32 * pitch);
            let v = f.at(x, y);
            let r = size(v);
            if r > 0.0 { let p = paint(x, y, v); self.dot(x, y, r, p); }
        } }
    }
}
