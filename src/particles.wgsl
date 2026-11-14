// Morph tiles (DESIGN.md §1.10). A tile is one lattice cell. It shows what the source drew in its
// source cell and, as it lands, what the target draws in its target cell: both are read texel for
// texel from pictures of the two elements' stage-1 output (ink and near), made once per morph. Tiles
// are drawn into ink (blended over) and near (max) like any instance, so the material draws them.
// At progress 0 a tile is its source cell exactly, at 1 its target cell exactly.

struct PU {
  res: vec4f, // width, height, scale, seconds into the flight
  m: vec4f,   // curl (device px), ease (0 linear, 1 in-out cubic, 2 out-expo), picture flag (near.g) of the morph's picture or 0, flight length (s)
  bg: vec4f, fg: vec4f, accent: vec4f, // theme colours
};
@group(0) @binding(0) var<uniform> pu: PU;
@group(0) @binding(1) var src_ink: texture_2d<f32>;
@group(0) @binding(2) var dst_ink: texture_2d<f32>;
@group(0) @binding(3) var src_near: texture_2d<f32>;
@group(0) @binding(4) var dst_near: texture_2d<f32>;

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
fn alpha(k: f32, u: f32) -> f32 {
  if (k > 0.5 && k < 1.5) { return smoothstep(0.0, 0.2, u); }
  if (k > 1.5 && k < 2.5) { return 1.0 - smoothstep(0.8, 1.0, u); }
  if (k > 2.5 && k < 3.5) { return 1.0 - smoothstep(0.0, 0.2, u); }
  if (k > 3.5) { return smoothstep(0.8, 1.0, u); }
  return 1.0;
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
  if (misc.x < 2.5 && u > 0.0 && u < 1.0) {
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
  if (i.misc.x > 2.5 && i.misc.x < 3.5) { w = 0.0; }
  if (i.misc.x > 3.5) { w = 1.0; }
  o.org = org; o.ab = i.ab; o.w = vec4f(w, a, i.misc.w, u);
  return o;
}

fn texel(t: texture_2d<f32>, p: vec2f) -> vec4f { return textureLoad(t, vec2i(clamp(floor(p), vec2f(0.0), pu.res.xy - 1.0)), 0); }

struct Px { ink: vec4f, near: vec4f };
fn tile_px(f: POut) -> Px {
  let o = f.pos.xy - f.org;
  let sp = f.ab.xy + o;
  let dp = f.ab.zw + o;
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
      let c = mix(mix(pu.bg.rgb, pu.fg.rgb, t), pu.accent.rgb, g);
      ink = vec4f(c * ink.a, ink.a);
      nr.g = pu.m.z;
    }
  }
  ink *= f.w.y;
  nr.r *= f.w.y;
  // Every tile is fully loose, as its element was at the hand-over.
  nr.b = step(0.02, nr.r);
  return Px(ink, nr);
}
@fragment fn fs_part_ink(f: POut) -> @location(0) vec4f { return tile_px(f).ink; }
@fragment fn fs_part_near(f: POut) -> @location(0) vec4f { return tile_px(f).near; }
