//! Text on a form (DESIGN.md §1.26): a paragraph wrapped onto a sphere, a mesh or any surface given by
//! its distance from the centre in each direction, seen by a `mesh::Camera`. `Form::pose` gives every
//! glyph its place, turn, scale, depth (`Pose::z`) and facing (`Pose::facing`); `El::form(FormFx)` then
//! draws the glyphs far to near and keeps the form legible: what faces away is left out or faint, what
//! is edge on fades out, a glyph mostly hidden by nearer ones is not drawn, the far side is dimmer.
//! `Form::hit` says where on the surface a point of the element is, so an opening or a drag follows the
//! form instead of the flat page.
#![warn(missing_docs)]
use crate::el::{GlyphAt, Pose};
use crate::mesh::{Camera, Mesh, Projection, add, cross, dot, mulf, norm, sub};
use crate::scene::Instance;
use std::rc::Rc;

/// How posed glyphs on a form are drawn (`El::form`).
///
/// | name | what | range | step | default |
/// |---|---|---|---|---|
/// | `back` | opacity kept by glyphs facing away | 0–1 | 0.05 | 0 |
/// | `edge` | glyphs facing less than this fade out, gone at 60 % of it (edge-on glyphs are not drawn) | 0–0.8 | 0.05 | 0.6 |
/// | `overlap` | a glyph whose ink is covered by nearer glyphs more than this is not drawn | 0.1–1 | 0.05 | 0.35 |
/// | `fog` | tone of the farthest glyphs (the nearest keep theirs) | 0–1 | 0.05 | 0.6 |
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FormFx {
    /// The opacity kept by glyphs facing away.
    pub back: f32,
    /// Glyphs facing less than this fade out.
    pub edge: f32,
    /// Covered more than this by nearer glyphs, a glyph is not drawn.
    pub overlap: f32,
    /// The tone of the farthest glyphs.
    pub fog: f32,
}
impl Default for FormFx { fn default() -> Self { Self { back: 0.0, edge: 0.6, overlap: 0.35, fog: 0.6 } } }
impl FormFx {
    /// The numbers' ranges and steps, for controls: (name, min, max, step).
    pub const PARAMS: [(&'static str, f32, f32, f32); 4] = [("back", 0.0, 1.0, 0.05), ("edge", 0.0, 0.8, 0.05), ("overlap", 0.1, 1.0, 0.05), ("fog", 0.0, 1.0, 0.05)];
    /// The same with every number inside its range.
    pub fn clamped(self) -> Self {
        Self { back: self.back.clamp(0.0, 1.0), edge: self.edge.clamp(0.0, 0.8), overlap: self.overlap.clamp(0.1, 1.0), fog: self.fog.clamp(0.0, 1.0) }
    }
    /// How much of a glyph's opacity is kept for how it faces: `back` when turned away, fading in
    /// smoothly from 60 % of `edge` (0) to `edge`, whole beyond it.
    pub fn facing_alpha(&self, facing: f32) -> f32 {
        if facing < 0.0 { return self.back; }
        if self.edge <= 0.0 || facing >= self.edge { return 1.0; }
        // Gone well before edge on: a glyph squeezed to 60 % of `edge` is not drawn.
        let k = ((facing - self.edge * 0.6) / (self.edge * 0.4)).clamp(0.0, 1.0);
        let k = k * k * (3.0 - 2.0 * k);
        // Turned away glyphs keep `back`; an edge-on one is never fainter than that.
        self.back.max(k)
    }
}

/// One posed glyph while the runtime arranges a text element on its form: its instance (device px,
/// posed), its glyph id and number in the text, its depth and facing, and where layout put it (its
/// centre, logical px in the content box).
#[derive(Clone, Copy, Debug)]
pub struct Posed {
    /// Its instance (device px, posed).
    pub inst: Instance,
    /// Its glyph id.
    pub glyph: u32,
    /// Its number in the text.
    pub seq: usize,
    /// Its depth (larger is nearer).
    pub z: f32,
    /// How it faces the viewer (1 head on, 0 edge on, below 0 away).
    pub facing: f32,
    /// Its centre as laid out, logical px in the content box.
    pub at: (f32, f32),
    /// The piece it belongs to, when the text has pieces.
    pub group: Option<u32>,
}

/// Fade each posed glyph by how it faces the viewer (`FormFx::facing_alpha`).
pub fn face(glyphs: &mut [Posed], fx: &FormFx) {
    for g in glyphs { g.inst.paint[2] *= fx.facing_alpha(g.facing); }
}

/// Arrange the posed glyphs of one text element (DESIGN.md §1.26): what is transparent left out, the
/// rest sorted far to near (stable, so equal depths keep their order and a flat text is unchanged); with
/// a `FormFx`, the ones mostly covered by nearer ink left out (the true outlines, measured with
/// `morph::cover_at`; each close pair measured costs one mark of the frame budget, and when the budget
/// is spent the boxes' overlap stands in), the far ones dimmed. Glyphs in a piece (`Posed::group`, from
/// `reveal_whole`) are left out together, when any of them is covered that much.
pub fn arrange(glyphs: &mut Vec<Posed>, fx: Option<&FormFx>, atlas: &[u8]) {
    glyphs.retain(|g| g.inst.paint[2] > 0.004);
    glyphs.sort_by(|a, b| a.z.total_cmp(&b.z));
    let Some(fx) = fx else { return };
    if glyphs.is_empty() { return; }
    // Thinning: nearest first, each glyph against the ones kept in front of it.
    let open = [-1e9, -1e9, 1e9, 1e9];
    let mut kept: Vec<(Instance, [f32; 4])> = Vec::with_capacity(glyphs.len());
    let mut cover = vec![0.0f32; glyphs.len()];
    let grouped = glyphs.iter().any(|g| g.group.is_some());
    for k in (0..glyphs.len()).rev() {
        let mut g = glyphs[k].inst;
        g.clip = open;
        let b = ink_box(&g);
        let front: Vec<&(Instance, [f32; 4])> = kept.iter().filter(|(_, o)| o[0] < b[2] && o[2] > b[0] && o[1] < b[3] && o[3] > b[1]).collect();
        if !front.is_empty() {
            cover[k] = if crate::budget::grant(front.len()) == front.len() { covered_ink(&g, &b, &front, atlas) } else { covered_box(&b, &front) };
            if !grouped && cover[k] > fx.overlap { continue; }
        }
        kept.push((g, b));
    }
    let drop: Vec<bool> = if grouped {
        let mut most: std::collections::HashMap<u32, f32> = Default::default();
        for (g, c) in glyphs.iter().zip(&cover) { if let Some(w) = g.group { let e = most.entry(w).or_default(); *e = e.max(*c); } }
        glyphs.iter().zip(&cover).map(|(g, c)| match g.group { Some(w) => most[&w] > fx.overlap, None => *c > fx.overlap }).collect()
    } else { cover.iter().map(|c| *c > fx.overlap).collect() };
    let mut k = 0;
    glyphs.retain(|_| { k += 1; !drop[k - 1] });
    // Fog: the far side dimmer, by depth across what is drawn.
    let (z0, z1) = glyphs.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), g| (a.min(g.z), b.max(g.z)));
    if z1 - z0 > 1e-3 && fx.fog < 1.0 {
        for g in glyphs.iter_mut() {
            let t = (g.z - z0) / (z1 - z0);
            g.inst.paint[0] *= fx.fog + (1.0 - fx.fog) * t;
        }
    }
}

/// Show or take away text by whole pieces (`El::reveal_by`): `groups` are the pieces' boxes (logical px,
/// from `text::Text::reveal_groups`), `origin` the content box's top left, `clip` the visible box
/// (device px). Every glyph of a piece gets the opacity of its faintest glyph, and a piece any of whose
/// glyphs' ink the clip cuts is not drawn at all.
pub fn reveal_whole(glyphs: &mut [Posed], groups: &[Vec<crate::ui::Rect>], origin: (f32, f32), clip: [f32; 4]) {
    if groups.is_empty() { return; }
    let holds = |w: usize, x: f32, y: f32| groups[w].iter().any(|r| r.inset(-0.5, -0.5, -0.5, -0.5).contains(x, y));
    // Every box by its top (a line's boxes share their top and height): a glyph is looked for among the
    // boxes of the line it is on. Glyphs come in reading order, as the pieces do, so the last glyph's
    // piece and the one after it are looked at first.
    let mut boxes: Vec<(crate::ui::Rect, usize)> = groups.iter().enumerate().flat_map(|(w, bs)| bs.iter().map(move |r| (r.inset(-0.5, -0.5, -0.5, -0.5), w))).collect();
    boxes.sort_by(|a, b| a.0.y.total_cmp(&b.0.y).then(a.1.cmp(&b.1)));
    let on_line = |x: f32, y: f32| {
        let end = boxes.partition_point(|(r, _)| r.y <= y);
        boxes[..end].iter().rev().take_while(|(r, _)| r.y + r.h >= y).filter(|(r, _)| r.contains(x, y)).map(|&(_, w)| w).min()
    };
    let mut last = 0;
    let which: Vec<Option<usize>> = glyphs.iter().map(|g| {
        let (x, y) = (g.at.0 + origin.0, g.at.1 + origin.1);
        let w = [last, last + 1].into_iter().filter(|&w| w < groups.len()).find(|&w| holds(w, x, y)).or_else(|| on_line(x, y));
        if let Some(w) = w { last = w; }
        w
    }).collect();
    // Each piece's opacity is its faintest glyph's (relative to the text's own), so a word fades as one and
    // is gone as soon as any of it would be.
    let mut least = vec![(f32::INFINITY, false); groups.len()];
    for (g, w) in glyphs.iter().zip(&which) {
        let Some(w) = *w else { continue };
        let b = ink_box(&g.inst);
        let cut = b[2] > b[0] && (b[0] < clip[0] - 0.5 || b[1] < clip[1] - 0.5 || b[2] > clip[2] + 0.5 || b[3] > clip[3] + 0.5);
        let e = &mut least[w];
        e.0 = e.0.min(g.inst.paint[2]);
        e.1 |= cut;
    }
    for (g, w) in glyphs.iter_mut().zip(&which) {
        let Some(w) = *w else { continue };
        let (a, cut) = least[w];
        g.inst.paint[2] = if cut { 0.0 } else { a };
        g.group = Some(w as u32);
    }
}

/// A glyph's ink box as drawn (device px): its transformed box without the spread padding.
pub fn ink_box(g: &Instance) -> [f32; 4] {
    let pad = crate::text::SPREAD as f32;
    let mut i = *g;
    i.rect = [g.rect[0] + pad, g.rect[1] + pad, (g.rect[2] - 2.0 * pad).max(0.0), (g.rect[3] - 2.0 * pad).max(0.0)];
    i.bounds(0.0)
}

/// The fraction of `g`'s ink that nearer glyphs' ink covers, sampled on a grid over its box.
fn covered_ink(g: &Instance, b: &[f32; 4], front: &[&(Instance, [f32; 4])], atlas: &[u8]) -> f32 {
    const N: usize = 7;
    let (mut ink, mut hid) = (0.0f32, 0.0f32);
    let (w, h) = ((b[2] - b[0]) / N as f32, (b[3] - b[1]) / N as f32);
    for iy in 0..N {
        for ix in 0..N {
            let (x, y) = (b[0] + (ix as f32 + 0.5) * w, b[1] + (iy as f32 + 0.5) * h);
            if crate::morph::cover_at(g, x, y, atlas) < 0.5 { continue; }
            ink += 1.0;
            if front.iter().any(|(o, ob)| x >= ob[0] && x < ob[2] && y >= ob[1] && y < ob[3] && crate::morph::cover_at(o, x, y, atlas) >= 0.5) { hid += 1.0; }
        }
    }
    if ink <= 0.0 { 0.0 } else { hid / ink }
}
/// The same measured on boxes alone (when the frame's budget is spent).
fn covered_box(b: &[f32; 4], front: &[&(Instance, [f32; 4])]) -> f32 {
    let area = ((b[2] - b[0]) * (b[3] - b[1])).max(1e-3);
    let most = front.iter().map(|(_, o)| (b[2].min(o[2]) - b[0].max(o[0])).max(0.0) * (b[3].min(o[3]) - b[1].max(o[1])).max(0.0)).fold(0.0, f32::max);
    most / area
}

// ── Wrapping text onto a form ────────────────────────────────────────────────────────────────────

/// What the text lies on, in its own space around the origin.
#[derive(Clone)]
pub enum Shape {
    /// A sphere of this radius.
    Sphere(f32),
    /// Any star-shaped surface: its distance from the centre in each (unit) direction.
    Surface(Rc<dyn Fn([f32; 3]) -> f32>),
    /// A closed mesh around the origin (the text lies where a ray from the centre leaves it).
    Mesh(Rc<Mesh>),
}
impl std::fmt::Debug for Shape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Shape::Sphere(r) => write!(f, "Sphere({r})"), Shape::Surface(_) => write!(f, "Surface"), Shape::Mesh(m) => write!(f, "Mesh({} triangles)", m.tris.len()) }
    }
}

/// A form text is wrapped onto: the shape, how it is turned (`spin`: degrees about the vertical axis,
/// then about the horizontal one), the camera that sees it (the element's box is the camera's view),
/// and how much of the shape the text covers (`cover`: the fraction of the way round, and of pole to
/// pole). The text's block (the span its glyphs' centres cover) maps onto that much of the shape.
#[derive(Clone, Debug)]
pub struct Form {
    /// What the text lies on.
    pub shape: Shape,
    /// How it is turned: degrees about the vertical axis, then about the horizontal one.
    pub spin: [f32; 2],
    /// The camera that sees it.
    pub camera: Camera,
    /// How much of the shape the text covers: of the way round, and of pole to pole.
    pub cover: [f32; 2],
}
impl Default for Form {
    fn default() -> Self { Self { shape: Shape::Sphere(1.0), spin: [0.0, 0.0], camera: Camera { yaw: 0.0, pitch: 0.0, ..Camera::default() }, cover: [1.0, 0.72] } }
}

/// Where a point on the form is seen: in the element's box (logical px), its depth (logical px toward
/// the viewer from the target's plane), and how it faces the viewer (1 straight on, 0 edge on).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    /// Across the element's box, logical px.
    pub x: f32,
    /// Down it, logical px.
    pub y: f32,
    /// The depth, logical px toward the viewer.
    pub z: f32,
    /// How it faces the viewer (1 straight on, 0 edge on).
    pub facing: f32,
    /// How much nearer things are drawn larger (1 for an orthographic camera).
    pub perspective: f32,
}

/// Where a point of the element's box meets the form: the point and its normal in the form's own space
/// (so it turns with the form), the distance along the ray (world units), and where it is in the text's
/// block (`uv`, fractions of its span; outside 0..1 where the text does not reach).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormHit {
    /// The point, in the form's own space.
    pub point: [f32; 3],
    /// The normal there, in the form's own space.
    pub normal: [f32; 3],
    /// The distance along the ray, world units.
    pub t: f32,
    /// Where it is in the text's block, fractions of its span.
    pub uv: [f32; 2],
}

impl Form {
    /// A sphere of radius 1 seen straight on, turned by `spin`.
    pub fn sphere(spin: [f32; 2]) -> Self { Self { spin, ..Self::default() } }
    /// The form's turn as a 3 × 3 rotation (rows), own space → world.
    fn rot(&self) -> [[f32; 3]; 3] {
        let (y, p) = (self.spin[0].to_radians(), self.spin[1].to_radians());
        let (sy, cy, sp, cp) = (y.sin(), y.cos(), p.sin(), p.cos());
        // About y by yaw, then about x by pitch.
        [[cy, 0.0, sy], [sp * sy, cp, -sp * cy], [-cp * sy, sp, cp * cy]]
    }
    fn to_world(&self, v: [f32; 3]) -> [f32; 3] { let r = self.rot(); [dot(r[0], v), dot(r[1], v), dot(r[2], v)] }
    fn to_own(&self, v: [f32; 3]) -> [f32; 3] { let r = self.rot(); [r[0][0] * v[0] + r[1][0] * v[1] + r[2][0] * v[2], r[0][1] * v[0] + r[1][1] * v[1] + r[2][1] * v[2], r[0][2] * v[0] + r[1][2] * v[1] + r[2][2] * v[2]] }

    /// The direction (own space) that a place in the text's block (fractions of its span) lies in:
    /// across is the way round (from the front, left to right), down is from the upper pole downwards.
    pub fn direction(&self, u: f32, v: f32) -> [f32; 3] { let (lon, lat) = self.angles(u, v); dir(lon, lat) }
    /// Distance of the surface from the centre along a unit direction (own space), and its normal there.
    pub fn surface(&self, d: [f32; 3]) -> ([f32; 3], [f32; 3]) {
        match &self.shape {
            Shape::Sphere(r) => (mulf(d, *r), d),
            Shape::Surface(f) => {
                let r = f(d);
                // The normal from the surface's slope: two tangents, nudged.
                let up = if d[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
                let (t1, t2) = (norm(cross(up, d)), norm(cross(d, norm(cross(up, d)))));
                let e = 1e-3;
                let at = |v: [f32; 3]| { let v = norm(v); mulf(v, f(v)) };
                let p = mulf(d, r);
                let (a, b) = (sub(at(add(d, mulf(t1, e))), p), sub(at(add(d, mulf(t2, e))), p));
                let mut n = norm(cross(a, b));
                if dot(n, d) < 0.0 { n = mulf(n, -1.0); }
                (p, n)
            }
            Shape::Mesh(m) => {
                // Out from far away toward the centre: the outermost surface in this direction.
                let far = m.radius() * 2.0 + 1.0;
                match m.ray(mulf(d, far), mulf(d, -1.0)) {
                    Some((_, p, n)) => (p, n),
                    None => (mulf(d, 1e-3), d),
                }
            }
        }
    }
    /// Logical px per world unit at the camera's target, for a box `h` logical px tall.
    fn px_per_unit(&self, h: f32) -> f32 {
        match self.camera.projection {
            Projection::Perspective { fov } => h / (2.0 * self.camera.distance.max(1e-3) * (fov.clamp(1.0, 170.0).to_radians() * 0.5).tan()),
            Projection::Orthographic { height } => h / height.max(1e-3),
        }
    }
    /// Where a point of the form (own space), with its normal, is seen in a box `w` × `h` logical px.
    pub fn see(&self, p: [f32; 3], n: [f32; 3], w: f32, h: f32) -> Option<Seen> {
        let (pw, nw) = (self.to_world(p), self.to_world(n));
        let cam = &self.camera;
        let aspect = w.max(1.0) / h.max(1.0);
        let c = cam.proj(aspect).mul(&cam.view());
        let q = &c.0;
        let clip = [q[0] * pw[0] + q[4] * pw[1] + q[8] * pw[2] + q[12], q[1] * pw[0] + q[5] * pw[1] + q[9] * pw[2] + q[13], q[3] * pw[0] + q[7] * pw[1] + q[11] * pw[2] + q[15]];
        if clip[2] <= 1e-6 { return None; }
        let (fx, fy) = ((clip[0] / clip[2] + 1.0) * 0.5, (1.0 - clip[1] / clip[2]) * 0.5);
        let eye = cam.eye();
        let to_eye = match cam.projection { Projection::Perspective { .. } => norm(sub(eye, pw)), Projection::Orthographic { .. } => cam.basis()[2] };
        let back = cam.basis()[2];
        // Depth toward the viewer from the target's plane, and how much nearer things are enlarged.
        let toward = dot(sub(pw, cam.target), back);
        let perspective = match cam.projection { Projection::Perspective { .. } => cam.distance / (cam.distance - toward).max(1e-3), Projection::Orthographic { .. } => 1.0 };
        let k = self.px_per_unit(h);
        Some(Seen { x: fx * w, y: fy * h, z: toward * k, facing: dot(nw, to_eye), perspective })
    }

    /// The glyph poses that lay a text on this form (`El::pose`): each glyph's centre goes to its place
    /// on the surface, turned along the way round, scaled by how the surface is seen there (narrow
    /// where it turns away), with its depth and facing for `El::form`.
    pub fn pose(&self) -> impl Fn(GlyphAt) -> Pose + 'static {
        let form = self.clone();
        move |g: GlyphAt| form.pose_of(g)
    }
    /// One glyph's pose (see `pose`).
    pub fn pose_of(&self, g: GlyphAt) -> Pose {
        let [x0, y0, x1, y1] = g.span;
        let sw = (x1 - x0).max(1.0);
        let u = (g.x - x0) / sw;
        let v = if y1 - y0 < 1.0 { 0.5 } else { (g.y - y0) / (y1 - y0) };
        let (lon, lat) = self.angles(u, v);
        let (p, n) = self.surface(dir(lon, lat));
        let Some(c) = self.see(p, n, g.w, g.h) else { return Pose { alpha: 0.0, ..Pose::default() } };
        // One text px across is this much longitude; one text px down the same much latitude, so a
        // glyph keeps its proportions where the form faces the viewer.
        let step = std::f32::consts::TAU * self.cover[0].clamp(1e-3, 1.0) / sw;
        let seen = |lon: f32, lat: f32| self.see(self.surface(dir(lon, lat)).0, n, g.w, g.h).map_or((0.0, 0.0), |s| (s.x - c.x, s.y - c.y));
        let ax = seen(lon + step, lat);
        let ay = seen(lon, lat - step);
        let angle = ax.1.atan2(ax.0);
        let (ca, sa) = (angle.cos(), angle.sin());
        // Down, the part across the turned baseline is what a glyph's height becomes.
        let down = (-sa * ay.0 + ca * ay.1).abs();
        Pose { dx: c.x - g.x, dy: c.y - g.y, angle, sx: ax.0.hypot(ax.1), sy: down, z: c.z, facing: c.facing, ..Pose::default() }
    }
    /// Longitude and latitude (radians) of a place in the text's block (fractions of its span).
    fn angles(&self, u: f32, v: f32) -> (f32, f32) {
        ((u - 0.5) * std::f32::consts::TAU * self.cover[0].clamp(0.0, 1.0), (0.5 - v) * std::f32::consts::PI * self.cover[1].clamp(0.0, 0.98))
    }

    /// Where a point of the element's box (logical px, the box `w` × `h`) meets the form, nearest first.
    pub fn hit(&self, x: f32, y: f32, w: f32, h: f32) -> Option<FormHit> {
        let aspect = w.max(1.0) / h.max(1.0);
        let (o, d) = self.camera.ray(x / w.max(1.0), y / h.max(1.0), aspect);
        let (o, d) = (self.to_own(o), self.to_own(d));
        let (t, p, n) = match &self.shape {
            Shape::Sphere(r) => {
                let b = dot(o, d);
                let disc = b * b - (dot(o, o) - r * r);
                if disc < 0.0 { return None; }
                let t = -b - disc.sqrt();
                if t <= 0.0 { return None; }
                let p = add(o, mulf(d, t));
                (t, p, norm(p))
            }
            Shape::Mesh(m) => m.ray(o, d)?,
            Shape::Surface(_) => {
                // March to where the ray goes inside the surface, then halve the step.
                let reach = dot(o, o).sqrt() * 2.0 + 2.0;
                let inside = |t: f32| { let p = add(o, mulf(d, t)); let l = dot(p, p).sqrt(); l < 1e-6 || l <= self.surface(mulf(p, 1.0 / l)).0.iter().map(|v| v * v).sum::<f32>().sqrt() };
                let steps = 256;
                let mut prev = 0.0;
                let mut found = None;
                for k in 1..=steps {
                    let t = reach * k as f32 / steps as f32;
                    if inside(t) { found = Some((prev, t)); break; }
                    prev = t;
                }
                let (mut a, mut b) = found?;
                for _ in 0..24 { let m = (a + b) * 0.5; if inside(m) { b = m; } else { a = m; } }
                let p = add(o, mulf(d, b));
                let (_, n) = self.surface(norm(p));
                (b, p, n)
            }
        };
        let dir = norm(p);
        let lat = dir[1].clamp(-1.0, 1.0).asin();
        let lon = dir[0].atan2(dir[2]);
        let u = 0.5 + lon / (std::f32::consts::TAU * self.cover[0].clamp(1e-3, 1.0));
        let v = 0.5 - lat / (std::f32::consts::PI * self.cover[1].clamp(1e-3, 0.98));
        Some(FormHit { point: p, normal: n, t, uv: [u, v] })
    }
}

/// The unit direction at a longitude and latitude (radians): longitude 0 is the front (+z), growing to the right.
fn dir(lon: f32, lat: f32) -> [f32; 3] { [lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos()] }

/// A glyph pose for any element from a `Form` (the same as `El::pose(form.pose())`), as an `Rc`.
pub fn glyph_pose(form: &Form) -> crate::el::GlyphPose { Rc::new(form.pose()) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_sphere_hit_lies_on_it() {
        for spin in [[0.0, 0.0], [40.0, 10.0], [170.0, -20.0]] {
            let f = Form::sphere(spin);
            let h = f.hit(100.0, 100.0, 200.0, 200.0).expect("the centre is on the sphere");
            assert!((dot(h.point, h.point).sqrt() - 1.0).abs() < 1e-4);
            assert!(dot(h.normal, norm(h.point)) > 1.0 - 1e-4);
            assert!(f.hit(2.0, 2.0, 200.0, 200.0).is_none());
        }
        // The front of an unturned sphere is the middle of the text.
        let h = Form::sphere([0.0, 0.0]).hit(100.0, 100.0, 200.0, 200.0).expect("hit");
        assert!((h.uv[0] - 0.5).abs() < 1e-3 && (h.uv[1] - 0.5).abs() < 1e-3, "{:?}", h.uv);
    }
    #[test]
    fn facing_fades_smoothly() {
        let fx = FormFx::default();
        assert_eq!(fx.facing_alpha(-0.3), 0.0);
        assert_eq!(fx.facing_alpha(0.7), 1.0);
        // Gone at 60 % of `edge`, whole at `edge`.
        assert_eq!(fx.facing_alpha(0.6 * fx.edge - 0.01), 0.0);
        let (a, b) = (fx.facing_alpha(0.4), fx.facing_alpha(0.5));
        assert!(0.0 < a && a < b && b < 1.0);
    }
}
