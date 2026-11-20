// Morph tiles (DESIGN.md §1.10). A tile is one lattice cell. It shows what the source drew in its
// source cell and, as it lands, what the target draws in its target cell: both are read texel for
// texel from pictures of the two elements' stage-1 output (ink and near), made once per morph. Tiles
// are drawn into ink (blended over) and near (max) like any instance, so the material draws them.
// At progress 0 a tile is its source cell exactly, at 1 its target cell exactly.

struct PU {
  res: vec4f, // width, height, scale, seconds into the flight
  m: vec4f,   // curl (device px), ease (0 linear, 1 in-out cubic, 2 out-expo), picture flag (near.g) of the morph's picture or 0, flight length (s)
  bg: vec4f, fg: vec4f, accent: vec4f, // theme colours
  // The GPU path (DESIGN.md §1.10, GPU particles): tiles are a grid over each element, paired by place.
  ga: vec4f, // source grid: x0, y0, tile w, tile h (device px)
  gb: vec4f, // target grid
  gd: vec4f, // grid columns, rows, state texture width, stagger
  ge: vec4f, // dt of one step (s), the run's absolute start time, turned around at this flight clock (or -1), over this long
  gp: vec4f, // pointer x, y (device px), strength (+ pulls, − pushes), radius (device px)
  gf: vec4f, // curl noise strength (px/s²), its scale (px), damping (1/s), spring back to the path (1/s²)
  gw: vec4f, // fronts: speed (device px/s), width (device px), push (px/s²), now (absolute s)
  gt: vec4f, // turned around (any number of times, `morph::Turn`): alpha, beta, the leg's end (0 or 1)
  fronts: array<vec4f, 8>, // x, y, start time (absolute), strength
  greys: array<vec4f, 5>,  // the theme's greys, ground to ink, w the tone each is at (shader.wgsl `grey`)
};
@group(0) @binding(0) var<uniform> pu: PU;
/// A tone's colour: the ground through the theme's greys to the ink, as the page draws it.
fn grey_p(t: f32) -> vec3f {
  let tc = clamp(t, 0.0, 1.0);
  var a = pu.bg.rgb; var ta = 0.0;
  for (var k = 0; k < 4; k++) {
    let b = pu.greys[k];
    if (tc <= b.w) { return mix(a, b.rgb, clamp((tc - ta) / max(b.w - ta, 1e-6), 0.0, 1.0)); }
    a = b.rgb; ta = b.w;
  }
  return mix(a, pu.fg.rgb, clamp((tc - ta) / max(1.0 - ta, 1e-6), 0.0, 1.0));
}
@group(0) @binding(1) var src_ink: texture_2d<f32>;
@group(0) @binding(2) var dst_ink: texture_2d<f32>;
@group(0) @binding(3) var src_near: texture_2d<f32>;
@group(0) @binding(4) var dst_near: texture_2d<f32>;
// The GPU path's state: per tile, its offset from the eased path (xy, device px) and velocity (zw).
@group(0) @binding(5) var state: texture_2d<f32>;

// The same hash, noise, curves and opacities as morph.rs.
fn hash(x0: u32) -> u32 {
  var x = x0;
  x ^= x >> 16u; x *= 0x7feb352du;
  x ^= x >> 15u; x *= 0x846ca68bu;
  return x ^ (x >> 16u);
}
fn unit(h: u32) -> f32 { return f32(h >> 8u) / 16777216.0; }
fn corner(i: vec2i, salt: u32) -> f32 { return unit(hash(bitcast<u32>(i.x) ^ hash(bitcast<u32>(i.y) ^ salt))) * 2.0 - 1.0; }
fn vnoise(p: vec2f, salt: u32) -> f32 {
  let fl = floor(p);
  let i = vec2i(fl);
  let f = p - fl;
  let s = f * f * (3.0 - 2.0 * f);
  let a = mix(corner(i, salt), corner(i + vec2i(1, 0), salt), s.x);
  let b = mix(corner(i + vec2i(0, 1), salt), corner(i + vec2i(1, 1), salt), s.x);
  return mix(a, b, s.y);
}
fn ease(x0: f32) -> f32 {
  let x = clamp(x0, 0.0, 1.0);
  let k = pu.m.y;
  if (k < 0.5) { return x; }
  if (k < 1.5) { return select(1.0 - pow(-2.0 * x + 2.0, 3.0) * 0.5, 4.0 * x * x * x, x < 0.5); }
  return (1.0 - exp2(-10.0 * x)) / (1.0 - exp2(-10.0));
}
// The kind without the two mesh flags (8: starts behind a mesh, 16: lands behind one).
fn base_kind(k: f32) -> f32 { return f32(u32(k) & 7u); }
fn alpha(k0: f32, u: f32) -> f32 {
  let code = u32(k0);
  let k = base_kind(k0);
  var a = 1.0;
  if (k > 0.5 && k < 1.5) { a = smoothstep(0.0, 0.2, u); }
  if (k > 1.5 && k < 2.5) { a = 1.0 - smoothstep(0.8, 1.0, u); }
  if (k > 2.5 && k < 3.5) { a = 1.0 - smoothstep(0.0, 0.2, u); }
  if (k > 3.5) { a = smoothstep(0.8, 1.0, u); }
  if ((code & 8u) != 0u) { a *= smoothstep(0.06, 0.3, u); }
  if ((code & 16u) != 0u) { a *= 1.0 - smoothstep(0.7, 0.94, u); }
  return a;
}
fn progress(run: vec4f) -> f32 {
  let l = clamp((pu.res.w - run.z) / max(run.w, 1e-4), 0.0, 1.0);
  return mix(run.x, run.y, ease(l));
}
// Top-left at progress u: along the line, plus a swing to one side, largest mid-flight. The swing is
// mostly perpendicular and the same way for neighbours (smooth noise of the source position), so the
// swarm arcs together; the seed keeps tiles from moving in lockstep.
fn place(ab: vec4f, u: f32, misc: vec4f) -> vec2f {
  var q = mix(ab.xy, ab.zw, u);
  if (base_kind(misc.x) < 2.5 && u > 0.0 && u < 1.0) {
    let d = ab.zw - ab.xy;
    let len = length(d);
    let dir = select(vec2f(1.0, 0.0), d / max(len, 1e-6), len > 0.5);
    let n = vec2f(vnoise(ab.xy / 120.0, 0x68e31da4u), vnoise(ab.xy / 120.0, 0xb5297a4du));
    let s = vec2f(n.x + (misc.z - 0.5) * 0.6, n.y + (unit(hash(u32(misc.z * 16777216.0))) - 0.5) * 0.6);
    let k = pu.m.x * sin(3.14159265 * u);
    let side = 0.8 + 0.5 * s.x;
    let along = 0.35 * s.y;
    q += k * (vec2f(-dir.y, dir.x) * side + dir * along);
  }
  return q;
}

struct PIn { @builtin(vertex_index) vi: u32, @location(0) ab: vec4f, @location(1) run: vec4f, @location(2) misc: vec4f };
struct POut {
  @builtin(position) pos: vec4f,
  @location(0) @interpolate(flat) org: vec2f,
  @location(1) @interpolate(flat) ab: vec4f,
  @location(2) @interpolate(flat) w: vec4f, // content mix, opacity, where encodings switch, progress
  @location(3) @interpolate(flat) k: vec4f, // texels per px of the tile on the source side (xy) and the target side (zw)
};

@vertex fn vs_part(i: PIn) -> POut {
  let c = vec2f(f32(i.vi & 1u), f32((i.vi >> 1u) & 1u));
  let u = progress(i.run);
  let org = place(i.ab, u, i.misc);
  let p = org + c * i.misc.y;
  var o: POut;
  o.pos = vec4f(p.x / pu.res.x * 2.0 - 1.0, 1.0 - p.y / pu.res.y * 2.0, 0.0, 1.0);
  let a = alpha(i.misc.x, u);
  // A tile that has faded out entirely is not drawn.
  if (a <= 0.0) { o.pos = vec4f(-2.0, -2.0, 0.0, 1.0); }
  // Cells that stay show only their own side.
  var w = u;
  let kb = base_kind(i.misc.x);
  if (kb > 2.5 && kb < 3.5) { w = 0.0; }
  if (kb > 3.5) { w = 1.0; }
  o.org = org; o.ab = i.ab; o.w = vec4f(w, a, i.misc.w, u); o.k = vec4f(1.0);
  return o;
}

// ── The GPU path ───────────────────────────────────────────────────────────────────────────────
// Tile `i` is cell (i mod columns, i div columns) of a grid over each element's ink bounds (the two grids
// have the same columns and rows, so the pairing keeps places in order, as the CPU's Morton ranks do).
// Its run is the CPU's: the side facing the target leaves first, the seed spreads departures. Where it
// is: the eased path (the same `place`) plus an offset the simulation integrates, scaled by sin(πu), so
// at progress 0 and 1 it is exactly its cell.
struct Cell { a: vec2f, b: vec2f, seed: f32, delay: f32, dur: f32, swap: f32 };
fn grid_cell(i: u32) -> Cell {
  let cols = u32(pu.gd.x);
  let g = vec2f(f32(i % cols), f32(i / cols));
  var c: Cell;
  c.a = pu.ga.xy + g * pu.ga.zw;
  c.b = pu.gb.xy + g * pu.gb.zw;
  c.seed = unit(hash(i ^ 0x9e3779b9u));
  c.swap = 0.3 + 0.4 * unit(hash((i + 7777777u) ^ 0x9e3779b9u));
  let dims = pu.gd.xy;
  let ca = pu.ga.xy + pu.ga.zw * dims * 0.5;
  let cb = pu.gb.xy + pu.gb.zw * dims * 0.5;
  let d = cb - ca;
  let len = length(d);
  let reach = max((pu.ga.z * dims.x + pu.ga.w * dims.y) * 0.5, 1.0);
  let st = clamp(pu.gd.w, 0.0, 0.9);
  var lead = vnoise(c.a / 90.0, 0x68e31da4u) * 0.5 + 0.5;
  if (len > 20.0) { lead = clamp(0.5 - dot(c.a - ca, d) / len / reach, 0.0, 1.0); }
  c.delay = st * pu.m.w * 0.95 * clamp(0.6 * lead + 0.4 * c.seed, 0.0, 1.0);
  c.dur = pu.m.w * (1.0 - st);
  return c;
}
fn grid_u(c: Cell, clock: f32) -> f32 {
  if (pu.ge.z >= 0.0) {
    // Turned around: where it was at the first turn (ge.z), carried through the later turns as
    // alpha × that + beta, then all at once to this leg's end.
    let at = ease(clamp((pu.ge.z - c.delay) / max(c.dur, 1e-4), 0.0, 1.0));
    let start = pu.gt.x * at + pu.gt.y;
    return mix(start, pu.gt.z, ease(clamp(clock / max(pu.ge.w, 1e-4), 0.0, 1.0)));
  }
  return ease(clamp((clock - c.delay) / max(c.dur, 1e-4), 0.0, 1.0));
}
fn grid_place(c: Cell, u: f32, off: vec2f) -> vec2f {
  return place(vec4f(c.a, c.b), u, vec4f(0.0, 0.0, c.seed, c.swap)) + off * sin(3.14159265 * u);
}
fn state_at(i: u32) -> vec4f {
  let w = u32(pu.gd.z);
  return textureLoad(state, vec2i(i32(i % w), i32(i / w)), 0);
}

@vertex fn vs_grid(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
  let corner = vec2f(f32(vi & 1u), f32((vi >> 1u) & 1u));
  let c = grid_cell(ii);
  let u = grid_u(c, pu.res.w);
  let org = grid_place(c, u, state_at(ii).xy);
  let size = mix(pu.ga.zw, pu.gb.zw, u);
  let p = org + corner * size;
  var o: POut;
  o.pos = vec4f(p.x / pu.res.x * 2.0 - 1.0, 1.0 - p.y / pu.res.y * 2.0, 0.0, 1.0);
  o.org = org; o.ab = vec4f(c.a, c.b); o.w = vec4f(u, 1.0, c.swap, u);
  o.k = vec4f(pu.ga.zw / max(size, vec2f(1e-4)), pu.gb.zw / max(size, vec2f(1e-4)));
  return o;
}

/// One step of the simulation for tile `i`: forces act on its offset from the eased path. The same
/// function runs as a compute shader (WebGPU, native) and as a fragment shader into a float target
/// (WebGL2), so both tiers integrate alike.
fn integrate(i: u32, s: vec4f) -> vec4f {
  if (i >= u32(pu.gd.x * pu.gd.y)) { return vec4f(0.0); }
  let c = grid_cell(i);
  let dt = pu.ge.x;
  let clock = pu.res.w;
  let u = grid_u(c, clock);
  // At the ends of the flight the tile is its cell: no offset, no speed.
  if (u <= 0.0 || u >= 1.0) { return vec4f(0.0); }
  var off = s.xy;
  var v = s.zw;
  let at = grid_place(c, u, off);
  // Curl noise: the gradient of smooth noise turned a quarter, so the swarm swirls without bunching.
  let q = at / max(pu.gf.y, 1.0);
  let e = 0.5;
  let nx = vnoise(q + vec2f(e, 0.0), 0x2c1b3c6du) - vnoise(q - vec2f(e, 0.0), 0x2c1b3c6du);
  let ny = vnoise(q + vec2f(0.0, e), 0x2c1b3c6du) - vnoise(q - vec2f(0.0, e), 0x2c1b3c6du);
  var acc = vec2f(ny, -nx) / (2.0 * e) * pu.gf.x;
  // The pointer pulls (strength > 0) or pushes (< 0) within its radius.
  let to = pu.gp.xy - at;
  let dp = length(to);
  if (pu.gp.w > 0.0 && dp < pu.gp.w && dp > 1e-3) { acc += to / dp * pu.gp.z * (1.0 - dp / pu.gp.w); }
  // A front passing: a kick outward from where it started.
  for (var j = 0; j < 8; j++) {
    let f = pu.fronts[j];
    let age = pu.gw.w - f.z;
    if (f.w <= 0.0 || age < 0.0) { continue; }
    let r = age * pu.gw.x;
    let d = distance(at, f.xy);
    let band = exp(-((d - r) * (d - r)) / max(pu.gw.y * pu.gw.y, 1.0));
    acc += normalize(at - f.xy + vec2f(1e-3)) * band * pu.gw.z * f.w;
  }
  acc -= v * pu.gf.z + off * pu.gf.w;
  v += acc * dt;
  off += v * dt;
  return vec4f(off, v);
}

@vertex fn vs_sim(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
  return vec4f(f32((vi << 1u) & 2u) * 2.0 - 1.0, f32(vi & 2u) * 2.0 - 1.0, 0.0, 1.0);
}
/// The WebGL2 tier: one texel per tile, drawn into the next state texture.
@fragment fn fs_sim(@builtin(position) f: vec4f) -> @location(0) vec4f {
  let p = vec2u(floor(f.xy));
  let i = p.y * u32(pu.gd.z) + p.x;
  return integrate(i, textureLoad(state, vec2i(p), 0));
}

fn texel(t: texture_2d<f32>, p: vec2f) -> vec4f { return textureLoad(t, vec2i(clamp(floor(p), vec2f(0.0), pu.res.xy - 1.0)), 0); }

struct Px { ink: vec4f, near: vec4f };
fn tile_px(f: POut) -> Px {
  let o = f.pos.xy - f.org;
  let sp = f.ab.xy + o * f.k.xy;
  let dp = f.ab.zw + o * f.k.zw;
  let si = texel(src_ink, sp); let di = texel(dst_ink, dp);
  let sn = texel(src_near, sp); let dn = texel(dst_near, dp);
  let w = f.w.x;
  var ink: vec4f;
  var nr: vec4f;
  if ((sn.g > 0.05) == (dn.g > 0.05)) {
    // Drawn the same way on both sides (both text and shapes, or both pictures): a cross-fade.
    ink = mix(si, di, w);
    nr = vec4f(mix(sn.r, dn.r, w), select(sn.g, dn.g, w >= 0.5), 0.0, select(sn.a, dn.a, w >= 0.5));
  } else {
    // A picture on one side and text on the other are not in the same colours: each tile changes over
    // at its own moment.
    let take = w >= f.w.z;
    ink = select(si, di, take);
    nr = select(sn, dn, take);
  }
  // In the air, ink in the page's own colour (a button's ground fill) would cut holes in whatever it
  // passes over; there, tone and accent become the coverage. At both ends the tile is untouched.
  if (nr.g <= 0.05) {
    let key = smoothstep(0.0, 0.15, f.w.w) * (1.0 - smoothstep(0.85, 1.0, f.w.w));
    ink = mix(ink, vec4f(ink.r, ink.g, 0.0, max(ink.r, ink.g)), key);
    // When the morph involves a picture, text tiles would overlap picture tiles, and a pixel can only be
    // drawn one way. So during the flight text tiles turn into the theme colours they show and are
    // drawn as part of the picture.
    // Every tile at once (waiting, flying or landed), so no text tile is ever under a picture one; the
    // ends of the flight stay exact.
    if (pu.m.z > 0.0 && pu.res.w > 0.03 && pu.res.w < pu.m.w - 0.03 && ink.a > 0.0) {
      let t = ink.r / ink.a; let g = ink.g / ink.a;
      let c = grey_p(select(t / max(1.0 - g, 1e-6), 0.0, g > 0.999)) * max(1.0 - g, 0.0) + pu.accent.rgb * g;
      ink = vec4f(c * ink.a, ink.a);
      nr.g = pu.m.z;
    }
  }
  ink *= f.w.y;
  nr.r *= f.w.y;
  // Every tile is fully loose, as its element was at the hand-over.
  // (encoded as stage 1 does: 255 on the tile's ink, 125 in its halo; shader.wgsl `enc_loose`)
  nr.b = select(125.0 / 255.0, 1.0, ink.a > 0.5) * step(0.02, nr.r);
  return Px(ink, nr);
}
@fragment fn fs_part_ink(f: POut) -> @location(0) vec4f { return tile_px(f).ink; }
@fragment fn fs_part_near(f: POut) -> @location(0) vec4f { return tile_px(f).near; }
