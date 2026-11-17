//! Draw only what changed (DESIGN.md §1.19). Before a frame the renderer compares what it is about to
//! draw with what it drew last time, and finds two boxes (device px): where stage 1's images (ink, near,
//! ) can differ, and where the final frame can differ. Everything outside them is the
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
use crate::scene::{Field, Instance, MESH, PICTURE};
use crate::style::{Look, Material};
use crate::text::SPREAD;

/// x0, y0, x1, y1, device px; empty when x1 <= x0 or y1 <= y0.
pub type Bx = [f32; 4];
pub const EMPTY: Bx = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
pub fn union(a: Bx, b: Bx) -> Bx { [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])] }
pub fn grow(a: Bx, d: f32) -> Bx { if is_empty(a) { a } else { [a[0] - d, a[1] - d, a[2] + d, a[3] + d] } }
pub fn is_empty(a: Bx) -> bool { a[2] <= a[0] || a[3] <= a[1] }
fn meet(a: Bx, b: Bx) -> Bx { [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])] }
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
}

/// The two boxes, or everything.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Region { All, Boxes { stage1: Bx, stage2: Bx } }

/// How far stage 2 reads around a pixel under material `m` (device px).
pub fn reach(m: Material, look: &Look, s: f32) -> f32 {
    let p = |i: usize| look.numbers(m).0[i];
    let base = 2.0 + 10.0 * s; // the dottr ground's cells read the nearness at their centre
    base + match m {
        Material::Plain | Material::Near | Material::Scribblr => 4.0 * s,
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

pub fn region(prev: &Inputs, cur: &Inputs, scale: f32) -> Region {
    if prev.global || cur.global || prev.size != cur.size || prev.look != cur.look || prev.field.calm != cur.field.calm || prev.meshes.len() != cur.meshes.len() { return Region::All; }
    let mats = materials(cur);
    // The dot ground drifts everywhere on the ground clock.
    if prev.gclock != cur.gclock && mats.contains(&Material::Dottr) { return Region::All; }
    if mats != materials(prev) { return Region::All; }
    let clock_moved = prev.mclock != cur.mclock && mats.iter().any(|m| clocked(*m));
    let s = scale;
    let look = &cur.look;
    let pad = SPREAD as f32 + look.fx.push * s * 1.5 + 2.0;
    let mut st1 = EMPTY;
    let mut st2 = EMPTY;
    // Instances whose records differ (in place; a longer or shorter list from where they part).
    let (a, b) = (&prev.instances, &cur.instances);
    for k in 0..a.len().max(b.len()) {
        match (a.get(k), b.get(k)) {
            (Some(x), Some(y)) if x == y => {}
            (x, y) => { for i in [x, y].into_iter().flatten() { st1 = union(st1, i.bounds(pad)); } }
        }
    }
    // 3D elements whose meshes changed (the instance is the same box).
    for (k, (pa, pb)) in prev.meshes.iter().zip(&cur.meshes).enumerate() {
        if pa != pb { for i in b.iter().filter(|i| i.paint[3] == MESH && i.extra[1] as usize == k) { st1 = union(st1, i.bounds(pad)); } }
    }
    // What changes because time passes: condensing, the fronts, the lens and the burst on 3D elements.
    let enter = look.fx.enter.max(1e-3);
    let mut acting: Vec<Bx> = fronts(&prev.field, &prev.look, prev.now, s);
    acting.extend(fronts(&cur.field, look, cur.now, s));
    let lens_moved = prev.field.pointer != cur.field.pointer;
    // The lens is a bell: past `LENS_REACH` radii it is too weak to change a cell's pitch or a dot.
    let reach_r = if mats.contains(&Material::Dottr) { LENS_REACH_DOT } else { LENS_REACH };
    let lens = |f: &Field| { let r = reach_r * look.fx.lens.max(0.0) * s; if f.pointer[2] > 0.0 && look.fx.lens >= 0.5 { [f.pointer[0] - r, f.pointer[1] - r, f.pointer[0] + r, f.pointer[1] + r] } else { EMPTY } };
    let lenses = if lens_moved { union(lens(&prev.field), lens(&cur.field)) } else { EMPTY };
    let burst = |f: &Field, t: f32| if f.burst_at(t) > 0.0 { f.burst } else { EMPTY };
    let bursts = if prev.field.burst != cur.field.burst || prev.field.burst_t != cur.field.burst_t || burst(&cur.field, cur.now) != EMPTY || burst(&prev.field, prev.now) != EMPTY {
        union(burst(&prev.field, prev.now), burst(&cur.field, cur.now))
    } else { EMPTY };
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
        if condensing || fronted || meshy || held { st1 = union(st1, if lensed { meet(lenses, r) } else { r }); }
    }
    // The field map: where its bytes changed, every instance that reads it there (its looseness, and the
    // whole of it where its flow carries it), and the ground the materials draw from it.
    let map_box = cur.map.changed(&prev.map).map(|b| meet([b[0] * s, b[1] * s, b[2] * s, b[3] * s], [0.0, 0.0, cur.size.0 as f32, cur.size.1 as f32]));
    if let Some(mb) = map_box {
        let drift = look.map.drift * s + 1.0;
        for i in a.iter().chain(b) {
            if i.look[2] > 0.5 && i.resp[0] + i.resp[1] > 0.0 && !is_empty(meet(mb, i.bounds(pad + drift))) { st1 = union(st1, i.bounds(pad + drift)); }
        }
    }
    // Stage 2: what stage 1 changed, as far as the materials read around it, and the field on the ground.
    // Pictures' marks read their own cells (and the parallax its depth): only where a picture changed.
    let pic_changed = b.iter().chain(a.iter()).any(|i| i.paint[3] == PICTURE && !is_empty(meet(i.bounds(pad), st1)));
    let rch = mats.iter().map(|m| reach(*m, look, s)).fold(0.0, f32::max)
        + if pic_changed { look.pics.iter().map(|p| 2.0 * p.pitch * s + p.depth * s).fold(0.0, f32::max) } else { 0.0 };
    st2 = union(st2, grow(st1, rch));
    for f in &acting { st2 = union(st2, grow(*f, rch)); }
    // The lens acts on the pictures and 3D elements that let it (above) and on the dot ground.
    if mats.contains(&Material::Dottr) { st2 = union(st2, grow(lenses, rch)); }
    st2 = union(st2, grow(bursts, rch));
    // The ground under the field map (dot's units lean up to half their pitch, glitch's rows slip).
    if let Some(mb) = map_box { st2 = union(st2, grow(mb, rch + 9.0 * s)); }
    // Scroll nodes whose content moved (the lattices follow it) or whose trace shows (DESIGN.md §1.8.3).
    for j in 0..8 {
        for f in [&prev.field, &cur.field] {
            if prev.field.scrolls[j] != cur.field.scrolls[j] || prev.field.scrollo[j] != cur.field.scrollo[j] || f.scrollo[j][2] > 0.0 {
                let r = f.scrolls[j];
                if r[2] > r[0] { st2 = union(st2, grow(r, rch + look.scroll.trail * s + look.scroll.slip * s)); }
            }
        }
    }
    if prev.field.wipe != cur.field.wipe || prev.field.wipe_rect != cur.field.wipe_rect {
        for f in [&prev.field, &cur.field] { if f.wipe[1] > 0.0 { st2 = union(st2, grow(f.wipe_rect, rch)); } }
    }
    // The annotation layer is drawn over the material in the same pass: the marks that are new or gone
    // (as a set, so a label that grows by a glyph does not move every mark after it in the list).
    if prev.annot != cur.annot {
        let key = |i: &Instance| -> Vec<u32> { i.rect.iter().chain(&i.uv).chain(&i.paint).chain(&i.clip).chain(&i.extra).chain(&i.look).map(|v| v.to_bits()).collect() };
        let had: std::collections::HashSet<Vec<u32>> = prev.annot.iter().map(key).collect();
        let has: std::collections::HashSet<Vec<u32>> = cur.annot.iter().map(key).collect();
        for i in prev.annot.iter().filter(|i| !has.contains(&key(i))).chain(cur.annot.iter().filter(|i| !had.contains(&key(i)))) { st2 = union(st2, grow(rect(i.rect), 2.0)); }
    }
    let screen = [0.0, 0.0, cur.size.0 as f32, cur.size.1 as f32];
    Region::Boxes { stage1: meet(st1, screen), stage2: meet(st2, screen) }
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
        Inputs { instances: insts, annot: vec![], meshes: vec![], field: Field::default(), look: Look { material: Material::Plain, ..Look::default() }, now: 10.0, mclock: 0.0, gclock: 0.0, size: (800, 600), global: false, map: Default::default() }
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
