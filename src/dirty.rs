//! Draw only what changed (DESIGN.md §1.19). Before a frame the renderer compares what it is about to
//! draw with what it drew last time, and finds boxes (device px, at most `MAX_BOXES` a stage): where
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
    /// Where the instance list is cut into passes (`Scene::cuts`): instances are matched only within the
    /// same part.
    pub cuts: Vec<(u32, u32)>,
}

/// The two boxes, or everything.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Region { All, Boxes { stage1: Bx, stage2: Bx } }

/// The most boxes a frame redraws in each stage: what changed in more places is merged into this many.
pub const MAX_BOXES: usize = 8;
/// What a frame redraws, as several boxes: where stage 1's images can differ, and where the final frame
/// can; each list is at most `MAX_BOXES`, and may overlap.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parts { pub stage1: Vec<Bx>, pub stage2: Vec<Bx> }
impl Parts {
    /// Both lists as one box each.
    pub fn region(&self) -> Region { Region::Boxes { stage1: self.stage1.iter().fold(EMPTY, |a, b| union(a, *b)), stage2: self.stage2.iter().fold(EMPTY, |a, b| union(a, *b)) } }
}

/// The boxes merged into at most `max`: those that overlap first, then the two whose union adds least.
pub fn merge(mut v: Vec<Bx>, max: usize) -> Vec<Bx> {
    v.retain(|b| !is_empty(*b));
    let area = |b: Bx| (b[2] - b[0]).max(0.0) * (b[3] - b[1]).max(0.0);
    // Many small boxes (a run of changed marks each its own): neighbours in the list, which are mostly
    // neighbours on the screen, together.
    if v.len() > 64 {
        let per = v.len().div_ceil(64);
        v = v.chunks(per).map(|c| c.iter().fold(EMPTY, |a, b| union(a, *b))).collect();
    }
    loop {
        let mut merged = false;
        let mut k = 0;
        while k < v.len() {
            let mut j = k + 1;
            while j < v.len() {
                if !is_empty(meet(v[k], v[j])) { let b = v.swap_remove(j); v[k] = union(v[k], b); merged = true; } else { j += 1; }
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
/// leaves what comes after it matched. Each unmatched run, in either list, is one box.
fn changed(a: &[Instance], b: &[Instance], pad: f32, out: &mut Vec<Bx>) {
    const K: usize = 4;
    let run = |x: &[Instance], y: &[Instance]| x.iter().chain(y).fold(EMPTY, |r, i| union(r, i.bounds(pad)));
    let (mut lo, mut hi) = (0, 0);
    while lo < a.len() && lo < b.len() && a[lo] == b[lo] { lo += 1; }
    while hi < a.len() - lo && hi < b.len() - lo && a[a.len() - 1 - hi] == b[b.len() - 1 - hi] { hi += 1; }
    let (a, b) = (&a[lo..a.len() - hi], &b[lo..b.len() - hi]);
    if a.is_empty() || b.is_empty() { if !a.is_empty() || !b.is_empty() { out.push(run(a, b)); } return; }
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
        out.push(run(&a[i..i2], &b[j..j2]));
        (i, j) = (i2, j2);
    }
    if i < a.len() || j < b.len() { out.push(run(&a[i..], &b[j..])); }
}

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

/// What a frame redraws (one box per stage, the union of `parts`).
pub fn region(prev: &Inputs, cur: &Inputs, scale: f32) -> Region { parts(prev, cur, scale).map_or(Region::All, |p| p.region()) }

/// What a frame redraws, as boxes, or `None` for everything.
pub fn parts(prev: &Inputs, cur: &Inputs, scale: f32) -> Option<Parts> {
    let all = None;
    if prev.global || cur.global || prev.size != cur.size || prev.look != cur.look || prev.field.calm != cur.field.calm || prev.meshes.len() != cur.meshes.len() { return all; }
    let mats = materials(cur);
    // The dot ground drifts everywhere on the ground clock.
    if prev.gclock != cur.gclock && mats.contains(&Material::Dottr) { return all; }
    if mats != materials(prev) { return all; }
    let clock_moved = prev.mclock != cur.mclock && mats.iter().any(|m| clocked(*m));
    let s = scale;
    let look = &cur.look;
    let pad = SPREAD as f32 + look.fx.push * s * 1.5 + 2.0;
    let mut st1: Vec<Bx> = vec![];
    let mut st2: Vec<Bx> = vec![];
    // Instances whose records differ, matched in order within each part of the list (between the cuts
    // where passes begin); with the cuts in another order, in place.
    let (a, b) = (&prev.instances, &cur.instances);
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
    let map_box = cur.map.changed(&prev.map).map(|b| meet([b[0] * s, b[1] * s, b[2] * s, b[3] * s], [0.0, 0.0, cur.size.0 as f32, cur.size.1 as f32]));
    if let Some(mb) = map_box {
        let drift = look.map.drift * s + 1.0;
        for i in a.iter().chain(b) {
            // A blade bends wherever the map changed under its reach.
            let reads = i.blade() || (i.look[2] > 0.5 && i.resp[0] + i.resp[1] > 0.0);
            if reads && !is_empty(meet(mb, i.bounds(pad + drift))) { st1.push(i.bounds(pad + drift)); }
        }
    }
    // Stage 2: what stage 1 changed, as far as the materials read around it, and the field on the ground.
    // Pictures' marks read their own cells (and the parallax its depth): only where a picture changed.
    let screen = [0.0, 0.0, cur.size.0 as f32, cur.size.1 as f32];
    let st1: Vec<Bx> = merge(st1.into_iter().map(|x| meet(x, screen)).collect(), MAX_BOXES);
    let pic_changed = b.iter().chain(a.iter()).any(|i| i.paint[3] == PICTURE && st1.iter().any(|x| !is_empty(meet(i.bounds(pad), *x))));
    let rch = mats.iter().map(|m| reach(*m, look, s)).fold(0.0, f32::max)
        + if pic_changed { look.pics.iter().map(|p| 2.0 * p.pitch * s + p.depth * s).fold(0.0, f32::max) } else { 0.0 };
    st2.extend(st1.iter().map(|x| grow(*x, rch)));
    for f in &acting { st2.push(grow(*f, rch)); }
    // The lens acts on the pictures and 3D elements that let it (above) and on the dot ground.
    if mats.contains(&Material::Dottr) { st2.push(grow(lenses, rch)); }
    st2.push(grow(bursts, rch));
    // The ground under the field map (dot's units lean up to half their pitch, glitch's rows slip).
    if let Some(mb) = map_box { st2.push(grow(mb, rch + 9.0 * s)); }
    // Scroll nodes whose content moved (the lattices follow it) or whose trace shows (DESIGN.md §1.8.3).
    for j in 0..8 {
        for f in [&prev.field, &cur.field] {
            if prev.field.scrolls[j] != cur.field.scrolls[j] || prev.field.scrollo[j] != cur.field.scrollo[j] || f.scrollo[j][2] > 0.0 {
                let r = f.scrolls[j];
                if r[2] > r[0] { st2.push(grow(r, rch + look.scroll.trail * s + look.scroll.slip * s)); }
            }
        }
    }
    if prev.field.wipe != cur.field.wipe || prev.field.wipe_rect != cur.field.wipe_rect {
        for f in [&prev.field, &cur.field] { if f.wipe[1] > 0.0 { st2.push(grow(f.wipe_rect, rch)); } }
    }
    // The annotation layer is drawn over the material in the same pass: the marks that are new or gone
    // (as a set, so a label that grows by a glyph does not move every mark after it in the list).
    if prev.annot != cur.annot {
        let key = |i: &Instance| -> Vec<u32> { i.rect.iter().chain(&i.uv).chain(&i.paint).chain(&i.clip).chain(&i.extra).chain(&i.look).map(|v| v.to_bits()).collect() };
        let had: std::collections::HashSet<Vec<u32>> = prev.annot.iter().map(key).collect();
        let has: std::collections::HashSet<Vec<u32>> = cur.annot.iter().map(key).collect();
        for i in prev.annot.iter().filter(|i| !has.contains(&key(i))).chain(cur.annot.iter().filter(|i| !had.contains(&key(i)))) { st2.push(grow(rect(i.rect), 2.0)); }
    }
    Some(Parts { stage1: st1, stage2: merge(st2.into_iter().map(|x| meet(x, screen)).collect(), MAX_BOXES) })
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
