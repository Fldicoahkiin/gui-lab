//! Draw only what changed (DESIGN.md §1.19). Before a frame the renderer compares what it is about to
//! draw with what it drew last time, and finds boxes (device px, at most `MAX_BOXES` for stage 1 and
//! `MAX_SHADE_BOXES` for stage 2): where
//! stage 1's images (ink, near) can differ, and where the final frame can differ. Everything outside them is the
//! previous frame's, kept in place. Pure CPU; the renderer scissors its passes to the boxes.
//!
//! Stage 1 changes only where an instance changed (its record, or what it shows because time passes: it
//! is condensing, a front reaches it, the lens over a picture or 3D element that lets it, a burst over a
//! 3D element), and an instance's pixels
//! lie within its rect grown by the nearness, and the push. Stage 2 changes where
//! stage 1 changed, grown by how far the material reads its neighbours (`reach`), and where the field
//! acts on the ground (a front, the lens, a burst, a wipe), and where the annotation layer changed.
//! Whatever acts on every pixel (a new look, a commit's front, flying tiles, the reduced material pass,
//! the material clock under a material that reads it) redraws everything.
//!
//! A scroll node whose content moved by whole device px is a `Move`: the renderer moves the last frame's
//! images inside its viewport and draws only the band scrolled into view, a margin along the viewport's
//! edges as wide as the material reads, and what changed besides. Instances are matched against the last
//! frame's as the move carried them. With its trace showing (DESIGN.md §1.8.3), the trace changes from
//! frame to frame near the ink it trails: the renderer also shades again, within the viewport, every
//! pixel the trace can reach in either frame, and keeps the moved one elsewhere. Dottr and charr hold
//! the content's rows to their lattice while the trace shows: their frame moves by the lattice's steps,
//! not the content's.
use crate::scene::{Field, Instance, MESH, PICTURE};
use crate::style::{Look, Material};
use crate::text::SPREAD;

/// x0, y0, x1, y1, device px; empty when x1 <= x0 or y1 <= y0.
pub type Bx = [f32; 4];
pub const EMPTY: Bx = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
pub fn union(a: Bx, b: Bx) -> Bx { [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])] }
pub fn grow(a: Bx, d: f32) -> Bx { if is_empty(a) { a } else { [a[0] - d, a[1] - d, a[2] + d, a[3] + d] } }
pub fn is_empty(a: Bx) -> bool { a[2] <= a[0] || a[3] <= a[1] }
/// Where two boxes meet (empty when they do not).
pub fn meet(a: Bx, b: Bx) -> Bx { [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])] }
fn rect(r: [f32; 4]) -> Bx { [r[0], r[1], r[0] + r[2], r[1] + r[3]] }

/// How many lens radii from the pointer the lens can still change a pixel: `exp(-r²)` there is under
/// 0.01, which moves no picture cell off pitch 1 (at pitch 64 the pitch's ramp is 0.07 of a step) and
/// lights no dot of the dottr ground (its threshold is 0.05).
pub const LENS_REACH: f32 = 2.2;
/// The same for the dottr ground, where the lens adds to the drift: any trace of it moves the edge of a
/// dot the drift has lit, so it reaches as far as the bell is above 1e-5.
pub const LENS_REACH_DOT: f32 = 3.5;

/// What a frame is drawn from, as far as which pixels it touches goes.
#[derive(Clone)]
pub struct Inputs {
    pub instances: Vec<Instance>,
    pub annot: Vec<Instance>,
    /// Each 3D element's fingerprint (`Scene3d::fingerprint`), by its index in the scene.
    pub meshes: Vec<u64>,
    pub field: Field,
    pub look: Look,
    pub now: f32,
    pub mclock: f32,
    /// The ground clock (the dottr ground's drift), which only global motion moves.
    pub gclock: f32,
    pub size: (u32, u32),
    /// Something acts on every pixel this frame: a commit's front, flying tiles, the reduced pass.
    pub global: bool,
    /// The field map's bytes (DESIGN.md §1.29).
    pub map: crate::fieldmap::Snapshot,
    /// Where the instance list is cut into passes (`Scene::cuts`): instances are matched only within the
    /// same part.
    pub cuts: Vec<(u32, u32)>,
}

/// The two boxes, or everything.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Region { All, Boxes { stage1: Bx, stage2: Bx } }

/// The most boxes stage 1 redraws: what changed in more places is merged into this many (each box draws
/// every instance again, scissored, so they are few; ink drawn again where nothing changed is the same).
pub const MAX_BOXES: usize = 8;
/// The most boxes stage 2 shades: each is one full-window triangle, scissored, so there can be more,
/// and changes scattered over the window (drifting motes, a turning body's glyphs) are shaded where
/// they are, not in the few boxes that hold them all.
pub const MAX_SHADE_BOXES: usize = 32;
/// What a frame redraws, as several boxes: where stage 1's images can differ, and where the final frame
/// can; at most `MAX_BOXES` and `MAX_SHADE_BOXES`, and they may overlap.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parts {
    pub stage1: Vec<Bx>,
    pub stage2: Vec<Bx>,
    pub moved: Option<Move>,
    /// Why a scroll node whose content moved was not moved as a frame (`scroll_move`), for the readout;
    /// empty when one was or none moved.
    pub refused: &'static str,
}

/// A scroll node's viewport (x0, y0, x1, y1, whole device px) whose content moved by `d` device px:
/// inside it, this frame's images (ink, near) are the last frame's moved by `d`, and the final frame the
/// last one moved by `frame`, but for the band scrolled into view and the boxes of what changed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub rect: [u32; 4],
    pub d: [i32; 2],
    /// How far the final frame moves: `d`, or under a lattice that holds the rows while the trace shows,
    /// how far the lattice stepped (it may be 0).
    pub frame: [i32; 2],
    /// The stronger trace of the two frames (0: none); the renderer shades the pixels it can reach.
    pub trace: f32,
    /// The way the trace points (the shader's `Sc::d`, a unit vector).
    pub way: [f32; 2],
    /// The lattice's pitch that holds the rows (device px; 0: none): how far a pixel reads off its place.
    pub latch: f32,
}
/// The trace strength from which dottr and charr hold the rows to their lattice (the shader's `latch`).
const LATCH: f32 = 0.02;
impl Move {
    /// The viewport, as a box.
    pub fn bx(&self) -> Bx { self.rect.map(|v| v as f32) }
    /// `b` moved with the content, where it stays inside the viewport.
    pub fn carry(&self, b: Bx) -> Bx { self.carry_by(b, self.d) }
    /// `b` of the last final frame, where the move took it.
    pub fn carry_frame(&self, b: Bx) -> Bx { self.carry_by(b, self.frame) }
    fn carry_by(&self, b: Bx, d: [i32; 2]) -> Bx {
        let v = self.bx();
        meet([b[0] + d[0] as f32, b[1] + d[1] as f32, b[2] + d[0] as f32, b[3] + d[1] as f32], v)
    }
    /// Whether `i` is the content's: clipped by the viewport.
    fn holds(&self, i: &Instance) -> bool {
        let (v, c) = (self.bx(), i.clip);
        !is_empty(c) && c[0] >= v[0] - 0.5 && c[1] >= v[1] - 0.5 && c[2] <= v[2] + 0.5 && c[3] <= v[3] + 0.5
    }
    /// `i` as the move carries it (unchanged when it is not the content's).
    fn moved(&self, i: &Instance) -> Instance {
        let mut i = *i;
        if !self.holds(&i) { return i; }
        let (dx, dy) = (self.d[0] as f32, self.d[1] as f32);
        i.rect[0] += dx;
        i.rect[1] += dy;
        // The viewport's own edges stay where they are; a clip of the content's moves with it.
        let v = self.bx();
        let c = std::array::from_fn(|k| if (i.clip[k] - v[k]).abs() < 0.5 { i.clip[k] } else { i.clip[k] + self.d[k % 2] as f32 });
        i.clip = meet(c, v);
        if i.resp[1] > 1.5 { i.resp[2] += dx; i.resp[3] += dy; }
        if i.blade_head() { i.xf[0] += dx; i.xf[1] += dy; }
        i
    }
    /// The strips along the viewport's edges drawn again: the band scrolled into view grown by `r`, and
    /// `r` along every other edge whose other side the material reads differently after the move (an edge
    /// on the window's border along the motion reads the viewport's own pixels, clamped).
    pub fn edges(&self, r: f32, size: (u32, u32)) -> [Bx; 4] {
        let [x0, y0, x1, y1] = self.bx();
        let (dx, dy) = (self.d[0] as f32, self.d[1] as f32);
        let side = |border: bool, along: f32, toward: bool| if toward { along.abs() + r } else if border && along == 0.0 { 0.0 } else { r };
        let (l, rt) = (side(x0 <= 0.0, dx, dx > 0.0), side(x1 >= size.0 as f32, dx, dx < 0.0));
        let (t, b) = (side(y0 <= 0.0, dy, dy > 0.0), side(y1 >= size.1 as f32, dy, dy < 0.0));
        [[x0, y0, x0 + l, y1], [x1 - rt, y0, x1, y1], [x0, y0, x1, y0 + t], [x0, y1 - b, x1, y1]]
    }
}

/// The first scroll node whose content moved by whole device px (or, with its trace showing, stayed),
/// and less than its viewport, under a ground that is the same at every height. With the trace showing:
/// the same way in both frames, not under glitchr (its rows slip on their own clock) or an added
/// material, and under dottr or charr holding the rows the same way in both frames, alone in its
/// viewport, with the lattice stepping by whole device px.
fn scroll_move(prev: &Inputs, cur: &Inputs, mats: &[Material], s: f32) -> (Option<(usize, Move)>, &'static str) {
    let why = std::cell::Cell::new("");
    let no = |w: &'static str| { if why.get().is_empty() { why.set(w); } None };
    let moved = (0..8).any(|j| prev.field.scrollo[j] != cur.field.scrollo[j] || cur.field.scrollo[j][2] > 0.0);
    if !moved { return (None, ""); }
    if cur.look.theme.ground2 != cur.look.theme.ground { return (None, "ground gradient"); }
    let (w, h) = (cur.size.0 as f32, cur.size.1 as f32);
    let found = (0..8).find_map(|j| {
        let (r, a, b) = (cur.field.scrolls[j], prev.field.scrollo[j], cur.field.scrollo[j]);
        // The content moves against the offset.
        let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
        let v = [r[0].max(0.0).ceil(), r[1].max(0.0).ceil(), r[2].min(w).floor(), r[3].min(h).floor()];
        let trace = a[2].max(b[2]);
        if (dx, dy) == (0.0, 0.0) && trace <= 0.0 { return None; }
        if prev.field.scrolls[j] != r { return no("viewport changed"); }
        if dx.abs() >= v[2] - v[0] || dy.abs() >= v[3] - v[1] { return no("jump past viewport"); }
        let (rect, d) = (v.map(|x| x as u32), [dx as i32, dy as i32]);
        if trace <= 0.0 { return Some((j, Move { rect, d, frame: d, trace: 0.0, way: [0.0, 1.0], latch: 0.0 })); }
        if a[2] > 0.0 && b[2] > 0.0 && a[3] != b[3] { return no("trace turned"); }
        if mats.iter().any(|m| matches!(m, Material::Glitchr | Material::Custom(_))) { return no("trace: glitchr or custom"); }
        let w3 = if b[2] > 0.0 { b[3] } else { a[3] };
        let across = w3.abs() > 1.5;
        let way = if across { [w3.signum(), 0.0] } else { [0.0, w3.signum()] };
        if (a[2] > LATCH) != (b[2] > LATCH) { return no("latch on/off"); }
        let pitches: Vec<f32> = mats.iter().filter_map(|m| latch_pitch(*m, &cur.look, s)).collect();
        if b[2] <= LATCH || pitches.is_empty() { return Some((j, Move { rect, d, frame: d, trace, way, latch: 0.0 })); }
        // The rows held to the lattice: the frame shows the content at its offset rounded to the pitch.
        let p = pitches[0];
        let alone = (0..8).all(|i| i == j || is_empty(meet(cur.field.scrolls[i], r)));
        if pitches.len() != mats.len() || pitches.iter().any(|q| *q != p) { return no("latch: mixed lattices"); }
        if !alone { return no("latch: not alone"); }
        let (oa, ob) = if across { (a[0], b[0]) } else { (a[1], b[1]) };
        // An offset half way between two rows is rounded by the GPU's own division: not relied on.
        let held = |o: f32| { let x = o / p; ((x.fract().abs() - 0.5).abs() > 1e-3).then(|| x.round_ties_even() * p) };
        let (Some(ha), Some(hb)) = (held(oa), held(ob)) else { return no("latch: half a row") };
        let step = ha - hb;
        if (step - step.round()).abs() > 1e-3 || step.abs() >= if across { v[2] - v[0] } else { v[3] - v[1] } { return no("latch: step"); }
        let step = step.round() as i32;
        let frame = if across { [step, 0] } else { [0, step] };
        Some((j, Move { rect, d, frame, trace, way, latch: p }))
    });
    let why = if found.is_some() { "" } else { why.get() };
    (found, why)
}
/// The lattice pitch a material holds the rows to while the trace shows (device px), if it does.
fn latch_pitch(m: Material, look: &Look, s: f32) -> Option<f32> {
    match m {
        Material::Dottr => Some((look.fx.pitch * s).max(1.5)),
        Material::Charr => Some((look.numbers(Material::Charr).0[0] * s).round().clamp(4.0, 40.0)),
        _ => None,
    }
}
impl Parts {
    /// Both lists as one box each.
    pub fn region(&self) -> Region { Region::Boxes { stage1: self.stage1.iter().fold(EMPTY, |a, b| union(a, *b)), stage2: self.stage2.iter().fold(EMPTY, |a, b| union(a, *b)) } }
}

/// The boxes merged into at most `max`: those that overlap where their union adds nothing beyond them
/// first (a long thin box crossing another is kept apart: drawing their overlap twice costs less), then
/// the two whose union adds least.
pub fn merge(mut v: Vec<Bx>, max: usize) -> Vec<Bx> {
    v.retain(|b| !is_empty(*b));
    let area = |b: Bx| (b[2] - b[0]).max(0.0) * (b[3] - b[1]).max(0.0);
    // Many small boxes (changed marks each its own): those whose centres fall in one cell of an 8 × 8 grid
    // over them all, together.
    if v.len() > 64 {
        let all = v.iter().fold(EMPTY, |a, b| union(a, *b));
        let (cw, ch) = (((all[2] - all[0]) / 8.0).max(1.0), ((all[3] - all[1]) / 8.0).max(1.0));
        let mut cells = [EMPTY; 64];
        for b in &v {
            let (cx, cy) = (((b[0] + b[2]) * 0.5 - all[0]) / cw, ((b[1] + b[3]) * 0.5 - all[1]) / ch);
            let k = (cy.clamp(0.0, 7.0) as usize) * 8 + cx.clamp(0.0, 7.0) as usize;
            cells[k] = union(cells[k], *b);
        }
        v = cells.into_iter().filter(|b| !is_empty(*b)).collect();
    }
    loop {
        let mut merged = false;
        let mut k = 0;
        while k < v.len() {
            let mut j = k + 1;
            while j < v.len() {
                if !is_empty(meet(v[k], v[j])) && area(union(v[k], v[j])) <= area(v[k]) + area(v[j]) { let b = v.swap_remove(j); v[k] = union(v[k], b); merged = true; } else { j += 1; }
            }
            k += 1;
        }
        if !merged { break; }
    }
    while v.len() > max {
        let (mut best, mut at) = (f32::INFINITY, (0, 1));
        for a in 0..v.len() { for b in a + 1..v.len() {
            let add = area(union(v[a], v[b])) - area(v[a]) - area(v[b]);
            if add < best { best = add; at = (a, b); }
        } }
        let b = v.swap_remove(at.1);
        v[at.0] = union(v[at.0], b);
    }
    v
}

/// The instances that changed between `a` and `b`, as runs: those of one list matched in order to equal
/// ones of the other are the same, so a run of changed marks (a drawing whose strokes changed in number)
/// leaves what comes after it matched. Each unmatched run, in either list, is one box; a run changed in
/// place (as many in both) is a box for each of its instances, before and after, so marks that move all
/// over the window (drifting motes) are each where they are.
fn changed(a: &[Instance], b: &[Instance], pad: f32, out: &mut Vec<Bx>) {
    const K: usize = 4;
    let run = |x: &[Instance], y: &[Instance], out: &mut Vec<Bx>| {
        if x.len() == y.len() { out.extend(x.iter().zip(y).filter(|(p, q)| p != q).map(|(p, q)| union(p.bounds(pad), q.bounds(pad)))); }
        else { out.push(x.iter().chain(y).fold(EMPTY, |r, i| union(r, i.bounds(pad)))); }
    };
    let (mut lo, mut hi) = (0, 0);
    while lo < a.len() && lo < b.len() && a[lo] == b[lo] { lo += 1; }
    while hi < a.len() - lo && hi < b.len() - lo && a[a.len() - 1 - hi] == b[b.len() - 1 - hi] { hi += 1; }
    let (a, b) = (&a[lo..a.len() - hi], &b[lo..b.len() - hi]);
    if a.is_empty() || b.is_empty() { if !a.is_empty() || !b.is_empty() { run(a, b, out); } return; }
    let key = |w: &[Instance]| { use std::hash::{Hash, Hasher}; let mut h = std::collections::hash_map::DefaultHasher::new(); bytemuck::cast_slice::<Instance, u8>(w).hash(&mut h); h.finish() };
    // Where each run of `K` starts in `b`, by its hash: the first place, then each next one (`more`), in order.
    let n = b.len().saturating_sub(K - 1);
    let (mut at, mut more): (std::collections::HashMap<u64, usize>, Vec<usize>) = (std::collections::HashMap::new(), vec![usize::MAX; n]);
    for j in (0..n).rev() { if let Some(p) = at.insert(key(&b[j..j + K]), j) { more[j] = p; } }
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] { i += 1; j += 1; continue; }
        // Where the two lists meet again: the first run of `K` in `a` from here that `b` has at or after `j`.
        let mut next = None;
        for i2 in i..a.len().saturating_sub(K - 1) {
            let Some(&first) = at.get(&key(&a[i2..i2 + K])) else { continue };
            let mut p = first;
            while p != usize::MAX && (p < j || a[i2..i2 + K] != b[p..p + K]) { p = more[p]; }
            if p != usize::MAX { next = Some((i2, p)); break; }
        }
        let Some((i2, j2)) = next else { break };
        run(&a[i..i2], &b[j..j2], out);
        (i, j) = (i2, j2);
    }
    if i < a.len() || j < b.len() { run(&a[i..], &b[j..], out); }
}

/// How far stage 2 reads around a pixel under material `m` (device px).
pub fn reach(m: Material, look: &Look, s: f32) -> f32 {
    let p = |i: usize| look.numbers(m).0[i];
    // The dottr ground's cells read the nearness at their centre.
    let base = 2.0 + if m == Material::Dottr { 10.0 * s } else { 0.0 };
    base + match m {
        Material::Plain | Material::Scribblr => 4.0 * s,
        // Its ink bands read as far as the outermost one ends (`2 × count × width`).
        Material::Near => 4.0 * s + 2.0 * p(2) * if p(3) > 0.0 { p(3) } else { 0.5 * p(0) } * s,
        Material::Dottr => 2.0 * look.fx.pitch * s + 40.0 * look.fx.glitch * s,
        Material::Halftonr => 2.0 * p(0) * s + p(2) * s,
        Material::Charr => 2.0 * p(0) * s,
        Material::Smudgr => (p(1) + p(3)) * s + 6.0 * s,
        Material::Carvr => (p(1) + p(2)) * s,
        Material::Glitchr => p(1) * s + 3.0 * p(4) * s + p(2) * s,
        // What an added material reads around a pixel is not known: everything is redrawn.
        Material::Custom(_) => 1e6,
    }
}

/// Whether a material reads the material clock where things are loose (glitch's rows).
fn clocked(m: Material) -> bool { matches!(m, Material::Glitchr) }

/// The materials in use: the global one and every override.
fn materials(i: &Inputs) -> Vec<Material> {
    let mut v = vec![i.look.material];
    for inst in &i.instances { if inst.look[0] >= 127.5 { let m = Material::from_id((inst.look[0].round() as u32 % 16) as u8); if !v.contains(&m) { v.push(m); } } }
    v
}

/// Where the fronts of `f` act at `now` (alive, within their scope): the disc they have swept, grown by
/// their width; the tail behind them lies inside it.
fn fronts(f: &Field, look: &Look, now: f32, s: f32) -> Vec<Bx> {
    let fx = &look.fx;
    f.fronts.iter().enumerate().filter(|(_, r)| r[3] > 0.0 && now - r[2] >= 0.0 && now - r[2] < fx.wave_life.max(1e-3)).map(|(j, r)| {
        let rad = (now - r[2]) * fx.wave_speed * s + 3.5 * fx.wave_width * s + fx.push * s + 4.0;
        meet([r[0] - rad, r[1] - rad, r[0] + rad, r[1] + rad], f.scopes[j])
    }).collect()
}

/// What a frame redraws (one box per stage, the union of `parts`).
pub fn region(prev: &Inputs, cur: &Inputs, scale: f32) -> Region { parts(prev, cur, scale).map_or(Region::All, |p| p.region()) }

/// What a frame redraws, as boxes, or `None` for everything.
pub fn parts(prev: &Inputs, cur: &Inputs, scale: f32) -> Option<Parts> { parts_why(prev, cur, scale).ok() }
/// `parts`, or why the frame is drawn whole (for the readout, `perf::FramePath`).
pub fn parts_why(prev: &Inputs, cur: &Inputs, scale: f32) -> Result<Parts, &'static str> {
    if prev.global || cur.global { return Err("global"); }
    if prev.size != cur.size { return Err("size"); }
    if prev.look != cur.look { return Err("look"); }
    if prev.field.calm != cur.field.calm { return Err("calm"); }
    if prev.meshes.len() != cur.meshes.len() { return Err("meshes"); }
    let mats = materials(cur);
    // The dot ground drifts everywhere on the ground clock.
    if prev.gclock != cur.gclock && mats.contains(&Material::Dottr) { return Err("dottr ground clock"); }
    if mats != materials(prev) { return Err("materials"); }
    let clock_moved = prev.mclock != cur.mclock && mats.iter().any(|m| clocked(*m));
    let s = scale;
    let look = &cur.look;
    let pad = SPREAD as f32 + look.fx.push * s * 1.5 + 2.0;
    let mut st1: Vec<Bx> = vec![];
    let mut st2: Vec<Bx> = vec![];
    let (mv, refused) = scroll_move(prev, cur, &mats, s);
    // A box of the last final frame, and where the move took what was in it.
    let old = |b: Bx| [b, mv.map_or(EMPTY, |(_, m)| m.carry_frame(b))];
    // Instances whose records differ, matched in order within each part of the list (between the cuts
    // where passes begin); with the cuts in another order, in place.
    // The last frame's instances as the move carried them: what is matched there the move draws.
    let carried: Vec<Instance>;
    let (a, b) = match mv {
        Some((_, m)) => {
            carried = prev.instances.iter().map(|i| m.moved(i)).collect();
            // What is not the content's but lies over the viewport was moved with it: drawn again where
            // it is and where it went.
            for i in prev.instances.iter().filter(|i| !m.holds(i)) { let r = meet(i.bounds(pad), m.bx()); if !is_empty(r) { st1.extend([r, m.carry(r)]); } }
            (&carried, &cur.instances)
        }
        None => (&prev.instances, &cur.instances),
    };
    let what = |c: &[(u32, u32)]| c.iter().map(|x| x.1).collect::<Vec<_>>();
    if what(&prev.cuts) == what(&cur.cuts) {
        let bounds = |c: &[(u32, u32)], n: usize| { let mut v = vec![0]; v.extend(c.iter().map(|x| (x.0 as usize).min(n))); v.push(n); v };
        let (pa, pb) = (bounds(&prev.cuts, a.len()), bounds(&cur.cuts, b.len()));
        for k in 0..pa.len() - 1 { changed(&a[pa[k]..pa[k + 1].max(pa[k])], &b[pb[k]..pb[k + 1].max(pb[k])], pad, &mut st1); }
    } else {
        for k in 0..a.len().max(b.len()) {
            match (a.get(k), b.get(k)) {
                (Some(x), Some(y)) if x == y => {}
                (x, y) => { for i in [x, y].into_iter().flatten() { st1.push(i.bounds(pad)); } }
            }
        }
    }
    // 3D elements whose meshes changed (the instance is the same box).
    for (k, (pa, pb)) in prev.meshes.iter().zip(&cur.meshes).enumerate() {
        if pa != pb { for i in b.iter().filter(|i| i.paint[3] == MESH && i.extra[1] as usize == k) { st1.push(i.bounds(pad)); } }
    }
    // What changes because time passes: condensing, the fronts, the lens and the burst on 3D elements.
    let enter = look.fx.enter.max(1e-3);
    let mut acting: Vec<Bx> = fronts(&prev.field, &prev.look, prev.now, s).into_iter().flat_map(old).collect();
    acting.extend(fronts(&cur.field, look, cur.now, s));
    // Content moving under the lens is changed by it as the lens moving over the content.
    let lens_moved = prev.field.pointer != cur.field.pointer || mv.is_some();
    // The lens is a bell: past `LENS_REACH` radii it is too weak to change a cell's pitch or a dot.
    let reach_r = if mats.contains(&Material::Dottr) { LENS_REACH_DOT } else { LENS_REACH };
    let lens = |f: &Field| { let r = reach_r * look.fx.lens.max(0.0) * s; if f.pointer[2] > 0.0 && look.fx.lens >= 0.5 { [f.pointer[0] - r, f.pointer[1] - r, f.pointer[0] + r, f.pointer[1] + r] } else { EMPTY } };
    let lenses = if lens_moved { old(lens(&prev.field)).into_iter().fold(lens(&cur.field), union) } else { EMPTY };
    let burst = |f: &Field, t: f32| if f.burst_at(t) > 0.0 { f.burst } else { EMPTY };
    let bursts = if prev.field.burst != cur.field.burst || prev.field.burst_t != cur.field.burst_t || burst(&cur.field, cur.now) != EMPTY || burst(&prev.field, prev.now) != EMPTY {
        old(burst(&prev.field, prev.now)).into_iter().fold(burst(&cur.field, cur.now), union)
    } else { EMPTY };
    // Consecutive instances that change are one box.
    let mut acc = EMPTY;
    for i in b {
        let r = i.bounds(pad);
        let born = i.extra[3];
        let condensing = cur.now - born < enter || prev.now - born < enter;
        let fronted = acting.iter().any(|f| !is_empty(meet(*f, r)));
        // The lens reaches, in stage 1, the pictures and 3D elements that let it (`resp[3]`); a burst, 3D.
        let lensed = i.resp[3] > 0.5 && i.resp[1] < 1.5 && !is_empty(meet(lenses, r));
        let meshy = lensed || (i.paint[3] == MESH && !is_empty(meet(bursts, r)));
        // Under a material whose loose parts read the material clock, what is held loose changes when
        // it moves (only there: the rest of the window reads no clock).
        let held = clock_moved && i.extra[2] > 0.0;
        if condensing || fronted || meshy || held { acc = union(acc, if lensed { meet(lenses, r) } else { r }); } else if !is_empty(acc) { st1.push(std::mem::replace(&mut acc, EMPTY)); }
    }
    st1.push(acc);
    // The field map: where its bytes changed, every instance that reads it there (its looseness, and the
    // whole of it where its flow carries it), and the ground the materials draw from it.
    // With a move, everywhere the map is not at rest: the content moved through it.
    let map_box = cur.map.changed(&prev.map).into_iter().chain(mv.and(prev.map.lit())).chain(mv.and(cur.map.lit())).map(|b| [b[0] * s, b[1] * s, b[2] * s, b[3] * s]).reduce(union)
        .map(|b| { let b = old(b).into_iter().fold(b, union); meet(b, [0.0, 0.0, cur.size.0 as f32, cur.size.1 as f32]) });
    if let Some(mb) = map_box {
        let drift = look.map.drift * s + 1.0;
        for i in a.iter().chain(b) {
            // A blade bends wherever the map changed under its reach.
            let reads = i.blade() || (i.look[2] > 0.5 && i.resp[0] + i.resp[1] > 0.0);
            if reads && !is_empty(meet(mb, i.bounds(pad + drift))) { st1.push(i.bounds(pad + drift)); }
        }
    }
    let screen = [0.0, 0.0, cur.size.0 as f32, cur.size.1 as f32];
    if let Some((_, m)) = mv {
        // A loose picture with depth leans toward the pointer: those moved under it change.
        let deep = |i: &Instance| i.paint[3] == PICTURE && look.pics.get(i.paint[1].max(0.0) as usize).is_some_and(|p| p.depth > 0.0) && m.holds(i);
        if prev.field.pointer[2] > 0.0 || cur.field.pointer[2] > 0.0 { st1.extend(a.iter().chain(b).filter(|i| deep(i)).map(|i| i.bounds(pad))); }
        // The band scrolled into view.
        st1.extend(m.edges(0.0, cur.size));
    }
    // Stage 2: what stage 1 changed, as far as the materials read around it, and the field on the ground.
    // Pictures' marks read their own cells (and the parallax its depth): only where a picture changed.
    // Stage 2 grows what changed as it is; stage 1 draws it in a few boxes.
    let changed1: Vec<Bx> = merge(st1.into_iter().map(|x| meet(x, screen)).collect(), MAX_SHADE_BOXES);
    let st1: Vec<Bx> = merge(changed1.clone(), MAX_BOXES);
    let pic_changed = b.iter().chain(a.iter()).any(|i| i.paint[3] == PICTURE && changed1.iter().any(|x| !is_empty(meet(i.bounds(pad), *x))));
    // Rows held to a lattice read up to half its pitch off their place.
    let rch = mats.iter().map(|m| reach(*m, look, s)).fold(0.0, f32::max)
        + if pic_changed { look.pics.iter().map(|p| 2.0 * p.pitch * s + p.depth * s).fold(0.0, f32::max) } else { 0.0 }
        + mv.map_or(0.0, |(_, m)| m.latch);
    st2.extend(changed1.iter().map(|x| grow(*x, rch)));
    for f in &acting { st2.push(grow(*f, rch)); }
    // The lens acts on the pictures and 3D elements that let it (above) and on the dot ground.
    if mats.contains(&Material::Dottr) { st2.push(grow(lenses, rch)); }
    st2.push(grow(bursts, rch));
    // The ground under the field map (dot's units lean up to half their pitch, glitch's rows slip).
    if let Some(mb) = map_box { st2.push(grow(mb, rch + 9.0 * s)); }
    // Scroll nodes whose content moved (the lattices follow it) or whose trace shows (DESIGN.md §1.8.3).
    // A move draws its band and edges instead (above, and here as far as the material reads).
    for j in 0..8 {
        if mv.is_some_and(|(k, _)| k == j) { continue; }
        for (n, f) in [&prev.field, &cur.field].into_iter().enumerate() {
            if prev.field.scrolls[j] != cur.field.scrolls[j] || prev.field.scrollo[j] != cur.field.scrollo[j] || f.scrollo[j][2] > 0.0 {
                let r = grow(f.scrolls[j], rch + look.scroll.trail * s + look.scroll.slip * s);
                if f.scrolls[j][2] > f.scrolls[j][0] { st2.extend(if n == 0 { old(r) } else { [r, EMPTY] }); }
            }
        }
    }
    if let Some((_, m)) = mv {
        // The trace reads its ink up to its length off, across the viewport's edges too: there what it
        // read has moved out of sight, or in.
        let r = if m.trace > 0.0 { 2.0 * rch + look.scroll.trail.max(look.scroll.misreg) * s * m.trace } else { rch };
        st2.extend(m.edges(r, cur.size));
        if m.frame != m.d { st2.extend(Move { d: m.frame, ..m }.edges(r, cur.size)); }
    }
    if prev.field.wipe != cur.field.wipe || prev.field.wipe_rect != cur.field.wipe_rect || (mv.is_some() && cur.field.wipe[1] > 0.0) {
        if prev.field.wipe[1] > 0.0 { st2.extend(old(grow(prev.field.wipe_rect, rch))); }
        if cur.field.wipe[1] > 0.0 { st2.push(grow(cur.field.wipe_rect, rch)); }
    }
    // The annotation layer is drawn over the material in the same pass: the marks that are new or gone
    // (as a set, so a label that grows by a glyph does not move every mark after it in the list).
    // With a move, the last frame's marks as it carried them; those it moved that are not the content's
    // are drawn again where they were and where they went.
    let pa: Vec<Instance> = match mv { Some((_, m)) => prev.annot.iter().map(|i| m.moved(i)).collect(), None => prev.annot.clone() };
    if let Some((_, m)) = mv { for i in prev.annot.iter().filter(|i| !m.holds(i)) { let r = meet(grow(rect(i.rect), 2.0), m.bx()); if !is_empty(r) { st2.extend([r, m.carry_frame(r)]); } } }
    // With the trace showing, the renderer shades again where it can reach, marks or not: the marks over
    // the viewport are drawn again (where the frame took the last ones, and where they are).
    if let Some((_, m)) = mv.filter(|(_, m)| m.trace > 0.0 || m.frame != m.d) {
        for i in &prev.annot { let r = meet(grow(rect(i.rect), 2.0), m.bx()); if !is_empty(r) { st2.push(m.carry_frame(r)); } }
        for i in &cur.annot { let r = meet(grow(rect(i.rect), 2.0), m.bx()); if !is_empty(r) { st2.push(r); } }
    }
    if pa != cur.annot {
        let key = |i: &Instance| -> Vec<u32> { i.rect.iter().chain(&i.uv).chain(&i.paint).chain(&i.clip).chain(&i.extra).chain(&i.look).map(|v| v.to_bits()).collect() };
        let had: std::collections::HashSet<Vec<u32>> = pa.iter().map(key).collect();
        let has: std::collections::HashSet<Vec<u32>> = cur.annot.iter().map(key).collect();
        for i in pa.iter().filter(|i| !has.contains(&key(i))).chain(cur.annot.iter().filter(|i| !had.contains(&key(i)))) { st2.push(grow(rect(i.rect), 2.0)); }
    }
    Ok(Parts { stage1: st1, stage2: merge(st2.into_iter().map(|x| meet(x, screen)).collect(), MAX_SHADE_BOXES), moved: mv.map(|(_, m)| m), refused })
}

/// A box as whole device pixels inside the frame (for a scissor), or `None` when it is empty.
pub fn scissor(b: Bx, w: u32, h: u32) -> Option<[u32; 4]> {
    let (x0, y0) = (b[0].floor().max(0.0) as u32, b[1].floor().max(0.0) as u32);
    let (x1, y1) = (b[2].ceil().min(w as f32).max(0.0) as u32, b[3].ceil().min(h as f32).max(0.0) as u32);
    (x1 > x0 && y1 > y0).then(|| [x0, y0, x1 - x0, y1 - y0])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inputs(insts: Vec<Instance>) -> Inputs {
        Inputs { instances: insts, annot: vec![], meshes: vec![], field: Field::default(), look: Look { material: Material::Plain, ..Look::default() }, now: 10.0, mclock: 0.0, gclock: 0.0, size: (800, 600), global: false, map: Default::default(), cuts: vec![] }
    }
    fn inst(x: f32) -> Instance { Instance { rect: [x, 100.0, 40.0, 20.0], paint: [1.0, 0.0, 1.0, 1.0], extra: [0.0, 0.0, 0.0, -100.0], ..Default::default() } }

    #[test]
    fn nothing_changed_is_nothing_to_draw() {
        let a = inputs(vec![inst(10.0), inst(300.0)]);
        let mut b = a.clone();
        b.now = 11.0;
        match region(&a, &b, 1.0) { Region::Boxes { stage1, stage2 } => assert!(is_empty(stage1) && is_empty(stage2)), r => panic!("{r:?}") }
    }

    #[test]
    fn one_changed_instance_is_its_box_grown() {
        let a = inputs(vec![inst(10.0), inst(300.0)]);
        let mut b = a.clone();
        b.instances[1].paint[2] = 0.5;
        let Region::Boxes { stage1, stage2 } = region(&a, &b, 1.0) else { panic!() };
        assert!(stage1[0] < 300.0 && stage1[2] > 340.0 && stage1[0] > 200.0, "{stage1:?}");
        assert!(stage2[0] <= stage1[0] && stage2[2] >= stage1[2]);
        // A new look is everything.
        let mut c = b.clone();
        c.look.material = Material::Dottr;
        assert_eq!(region(&b, &c, 1.0), Region::All);
    }

    #[test]
    fn a_scoped_front_stays_in_its_scope() {
        let a = inputs(vec![inst(10.0), inst(500.0)]);
        let mut b = a.clone();
        b.field.front_in([30.0, 110.0], 10.0, 1.0, [10.0, 100.0, 50.0, 120.0], 7, 2.0);
        b.now = 10.2;
        let Region::Boxes { stage1, stage2 } = region(&a, &b, 1.0) else { panic!() };
        assert!(stage1[2] < 200.0 && stage2[2] < 200.0, "{stage1:?} {stage2:?}");
    }
}
