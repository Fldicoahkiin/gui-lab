//! Hit geometry: where things are drawn, so the pointer lands on what it is over. An element turned
//! after layout (`El::turn`) is hit where it is drawn, not in its layout box; a posed glyph (on a path,
//! on a form) is hit where its outline is now, the nearest one first (`Ui::glyph_at`).
#![warn(missing_docs)]
use crate::scene::{GLYPH, Instance};
use crate::ui::Rect;

/// A 2D affine map in logical px: `(x, y) → m × (x, y) + t`, `m` row-major.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    /// The linear part, row-major.
    pub m: [[f32; 2]; 2],
    /// The translation.
    pub t: [f32; 2],
}
impl Default for Affine { fn default() -> Self { Self::IDENTITY } }
impl Affine {
    /// The map that leaves every point where it is.
    pub const IDENTITY: Affine = Affine { m: [[1.0, 0.0], [0.0, 1.0]], t: [0.0, 0.0] };
    /// A turn by `angle` radians (clockwise on screen, y down) about (cx, cy).
    pub fn turn_about(angle: f32, cx: f32, cy: f32) -> Self {
        let (s, c) = angle.sin_cos();
        let m = [[c, -s], [s, c]];
        Affine { m, t: [cx - (m[0][0] * cx + m[0][1] * cy), cy - (m[1][0] * cx + m[1][1] * cy)] }
    }
    /// This map after `first` (`self ∘ first`).
    pub fn after(&self, first: &Affine) -> Affine {
        let (a, b) = (self.m, first.m);
        let m = [[a[0][0] * b[0][0] + a[0][1] * b[1][0], a[0][0] * b[0][1] + a[0][1] * b[1][1]],
                 [a[1][0] * b[0][0] + a[1][1] * b[1][0], a[1][0] * b[0][1] + a[1][1] * b[1][1]]];
        let (x, y) = self.apply(first.t[0], first.t[1]);
        Affine { m, t: [x, y] }
    }
    /// Where it maps (x, y).
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (self.m[0][0] * x + self.m[0][1] * y + self.t[0], self.m[1][0] * x + self.m[1][1] * y + self.t[1])
    }
    /// The point this map sends to (x, y); `None` when it is degenerate.
    pub fn unapply(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        let [[a, b], [c, d]] = self.m;
        let det = a * d - b * c;
        if det.abs() < 1e-9 { return None; }
        let (qx, qy) = (x - self.t[0], y - self.t[1]);
        Some(((d * qx - b * qy) / det, (a * qy - c * qx) / det))
    }
    /// Draw an instance (device px, `scale` device px per logical px) through this map: its centre moves,
    /// its own transform is turned with it.
    pub fn instance(&self, inst: &mut Instance, scale: f32) {
        let r = inst.rect;
        let (cx, cy) = ((r[0] + r[2] * 0.5) / scale, (r[1] + r[3] * 0.5) / scale);
        let (nx, ny) = self.apply(cx, cy);
        inst.rect[0] += (nx - cx) * scale;
        inst.rect[1] += (ny - cy) * scale;
        let x = inst.xf;
        let own = [[1.0 + x[0], x[1]], [x[2], 1.0 + x[3]]];
        let m = Affine { m: own, t: [0.0; 2] };
        let k = Affine { m: self.m, t: [0.0; 2] }.after(&m).m;
        inst.xf = [k[0][0] - 1.0, k[0][1], k[1][0], k[1][1] - 1.0];
    }
}

/// The transform an element is drawn through: its parent's (`up`), then its own turn about its centre.
pub fn turned(up: Option<Affine>, turn: f32, rect: &Rect) -> Option<Affine> {
    if turn == 0.0 { return up; }
    let (cx, cy) = rect.center();
    let own = Affine::turn_about(turn, cx, cy);
    Some(match up { Some(a) => a.after(&own), None => own })
}

/// Whether (x, y) is inside `rect` as drawn through `xf` (none: as laid out).
pub fn contains(rect: &Rect, xf: Option<&Affine>, x: f32, y: f32) -> bool {
    match xf {
        None => rect.contains(x, y),
        Some(a) => a.unapply(x, y).is_some_and(|(lx, ly)| rect.contains(lx, ly)),
    }
}

/// The glyph drawn under a point: which element (its key), its number in the text, its depth
/// (`Pose::z`), and the point in the text's own unposed layout (logical px from the element's content box).
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphHit {
    /// The element's key.
    pub key: String,
    /// The glyph's number in the text.
    pub i: usize,
    /// Its depth (`Pose::z`).
    pub z: f32,
    /// The point in the text's unposed layout, logical px from the content box.
    pub local: (f32, f32),
}

/// What the runtime remembers of each drawn glyph for hits: its instance's index in the scene, its
/// node, its number in the text, its depth, and where it was laid out before its pose (its centre,
/// logical px in the element's content box).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawnGlyph {
    /// Its instance's index in the scene.
    pub inst: usize,
    /// Its node.
    pub node: usize,
    /// Its number in the text.
    pub i: usize,
    /// Its depth.
    pub z: f32,
    /// Its centre before its pose, logical px in the content box.
    pub at: (f32, f32),
}

/// Every glyph of the last paint: the drawn ones (`DrawnGlyph`), and per text element (its node) every
/// laid-out glyph's centre with whether it was drawn (a pose or a clip can leave some out).
#[derive(Clone, Debug, Default)]
pub struct Glyphs {
    /// The drawn glyphs.
    pub log: Vec<DrawnGlyph>,
    /// Per text element, every laid-out glyph and whether it was drawn.
    pub laid: Vec<Laid>,
}
/// A text element's laid-out glyphs: its node, each glyph's centre, and whether it was drawn.
pub type Laid = (usize, Vec<(f32, f32)>, Vec<bool>);
/// An element's ink box (device px) and the point of its outline nearest a corner (`ink_of`).
pub type Ink = Option<([f32; 4], Option<(f32, f32)>)>;
impl Glyphs {
    /// Forget every glyph (a new paint).
    pub fn clear(&mut self) { self.log.clear(); self.laid.clear(); }
}

/// The nearest drawn glyph whose outline box (as drawn: moved, turned, scaled) holds device point
/// (x, y). Glyphs are drawn far to near, so the last one that holds it is in front. `clip` is honoured.
pub fn glyph_under(instances: &[Instance], drawn: &[DrawnGlyph], x: f32, y: f32) -> Option<DrawnGlyph> {
    drawn.iter().rev().copied().find(|g| {
        let Some(inst) = instances.get(g.inst) else { return false };
        if inst.paint[3] != GLYPH || inst.paint[2] <= 0.02 { return false; }
        let c = inst.clip;
        if x < c[0] || y < c[1] || x >= c[2] || y >= c[3] { return false; }
        let (bx, by) = crate::morph::to_box(inst, x, y);
        let pad = crate::text::SPREAD as f32;
        let r = inst.rect;
        bx >= r[0] + pad && by >= r[1] + pad && bx < r[0] + r[2] - pad && by < r[1] + r[3] - pad
    })
}

/// A posed glyph's point back in its unposed layout: `(x, y)` (device px) through the inverse of its
/// drawn transform, moved by where it was laid out (logical px).
pub fn local_of(inst: &Instance, at: (f32, f32), x: f32, y: f32, scale: f32) -> (f32, f32) {
    let (bx, by) = crate::morph::to_box(inst, x, y);
    let r = inst.rect;
    let (cx, cy) = (r[0] + r[2] * 0.5, r[1] + r[3] * 0.5);
    (at.0 + (bx - cx) / scale, at.1 + (by - cy) / scale)
}


/// What an element really draws (device px): the box round the ink of every instance it owns (`owned`
/// says which nodes; glyphs without their spread, shapes and pictures as drawn), and the point of a
/// glyph's outline nearest `corner` (sampled every device px), when it has glyphs. The annotation
/// layer's `INK` and the audit measure the same boxes.
pub fn ink_of(instances: &[Instance], owners: &[(usize, u64)], owned: impl Fn(usize) -> bool, corner: (f32, f32), atlas: &[u8]) -> Ink {
    let mut b: Option<[f32; 4]> = None;
    let mut near: Option<(f32, usize)> = None;
    for (k, (node, _)) in owners.iter().enumerate() {
        if !owned(*node) { continue; }
        let Some(inst) = instances.get(k) else { continue };
        if inst.paint[2] < 0.05 { continue; }
        let r = if inst.paint[3] == GLYPH { crate::form::ink_box(inst) } else { inst.bounds(0.0) };
        let c = inst.clip;
        let r = [r[0].max(c[0]), r[1].max(c[1]), r[2].min(c[2]), r[3].min(c[3])];
        if r[2] <= r[0] || r[3] <= r[1] { continue; }
        b = Some(match b { None => r, Some(o) => [o[0].min(r[0]), o[1].min(r[1]), o[2].max(r[2]), o[3].max(r[3])] });
        // Glyphs and shapes have an outline to end on (a picture's or a 3D view's is its box).
        if inst.paint[3] == GLYPH || inst.paint[3] == crate::scene::SHAPE {
            let d = (corner.0 - corner.0.clamp(r[0], r[2])).hypot(corner.1 - corner.1.clamp(r[1], r[3]));
            if near.is_none_or(|(nd, _)| d < nd) { near = Some((d, k)); }
        }
    }
    let end = near.and_then(|(_, k)| {
        let inst = &instances[k];
        let r = if inst.paint[3] == GLYPH { crate::form::ink_box(inst) } else { inst.bounds(0.0) };
        let mut best: Option<(f32, (f32, f32))> = None;
        let mut y = r[1].floor();
        while y < r[3] {
            let mut x = r[0].floor();
            while x < r[2] {
                if crate::morph::cover_at(inst, x + 0.5, y + 0.5, atlas) >= 0.5 {
                    let d = (x - corner.0).hypot(y - corner.1);
                    if best.is_none_or(|(bd, _)| d < bd) { best = Some((d, (x + 0.5, y + 0.5))); }
                }
                x += 1.0;
            }
            y += 1.0;
        }
        best.map(|b| b.1)
    });
    b.map(|b| (b, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn turns_compose_and_invert() {
        let a = Affine::turn_about(std::f32::consts::FRAC_PI_2, 10.0, 10.0);
        let (x, y) = a.apply(20.0, 10.0);
        assert!((x - 10.0).abs() < 1e-4 && (y - 20.0).abs() < 1e-4, "{x} {y}");
        let (u, v) = a.unapply(x, y).expect("invertible");
        assert!((u - 20.0).abs() < 1e-4 && (v - 10.0).abs() < 1e-4);
        let b = a.after(&a);
        let (x, y) = b.apply(20.0, 10.0);
        assert!((x - 0.0).abs() < 1e-3 && (y - 10.0).abs() < 1e-3, "{x} {y}");
    }
}
