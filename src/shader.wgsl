// Two stages.
//   Stage 1 draws every instance exactly, into two intermediate images (same geometry, drawn twice):
//     ink  (rgba8): what is there. Glyphs and shapes: r = tone, g = accent, a = coverage (premultiplied).
//                   Pictures: rgb = their colour, a = coverage.
//                   b = a code (straight, b / a): below 0.45 it is (hue + 3 × is-shape) / 14; from 0.5
//                   up the instance is one the height materials (chrome, liquid) shade, and b is
//                   0.5 + 0.49 × its height coordinate (0 at the top of its line or shape, 1 at the bottom).
//     near (rgba8, max-blended): r = signed nearness to the outline, by true outline: 0.5 on it, 1 at
//                   SPREAD px inside, 0 at SPREAD px outside; g = where a picture is, (PicFx slot + 1) / 8;
//                   b = the instance's own looseness (held, born, or set at rest); a = a material
//                   override (≥ 128/255: 128 + level × 16 + id) or else the print pitch of glyphs × 2 / 255.
//     soft (rgba8, added; only when liquid is in use): a metaball field of the instances liquid shades,
//                   a = field / 2, rgb = their colour × a.
//   Stage 2 (the material) reads both plus the field and decides the final pixel. Pictures turn into
//   marks by their own PicFx slot (DESIGN.md §1.6.1); text and shapes follow the material.

struct U {
  res: vec4f,      // width, height, scale, time
  fa: vec4f,       // pitch, _, jitter, glitch          (lengths in logical px)
  fb: vec4f,       // wave speed, wave life, wave width, wave tail
  fc: vec4f,       // push, lens radius, enter time, square (0 dot, 1 square)
  fd: vec4f,       // material (0 plain, 1 dot), srgb surface, spread px, palette rows
  bg: vec4f,
  fg: vec4f,
  accent: vec4f,
  pointer: vec4f,  // x, y, strength, _    (device px)
  wipe: vec4f,     // half-plane source: line x, strength, _, _
  wipe_rect: vec4f,// where it acts: x0, y0, x1, y1
  split: vec4f,    // a commit: origin x, y (device px), start time, on
  fronts: array<vec4f, 8>, // x, y, start time, strength
  pics: array<vec4f, 56>,  // 8 PicFx slots × 7 (see style.rs PicFx::pack)
  ground2: vec4f,  // the ground at the bottom of the window; w: looseness scale
  second: vec4f,   // accent hues 1 and 2
  third: vec4f,
  burst: vec4f,    // a burst's rect, device px (x0, y0, x1, y1)
  burst_t: vec4f,  // its start time, strength, length (s)
  mats: array<vec4f, 24>, // every material's numbers, two vec4s each (style.rs MAT_PARAMS)
};
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var tex0: texture_2d<f32>;
@group(0) @binding(3) var tex1: texture_2d<f32>;
@group(0) @binding(4) var marks: texture_2d<f32>;   // rg: mark coverage, height; rows = sets, columns = levels
@group(0) @binding(5) var palettes: texture_2d<f32>; // 16 × rows
@group(0) @binding(6) var chars: texture_2d<f32>;    // character tiles cut per device pitch (marks.rs char_tiles)
@group(0) @binding(7) var before: texture_2d<f32>;   // the frame before the last commit
@group(0) @binding(8) var soft: texture_2d<f32>;     // the liquid's field

const PLAIN: i32 = 0; const DOT: i32 = 1; const HALFTONE: i32 = 2; const ASCII: i32 = 3; const PRINT: i32 = 4;
const LINO: i32 = 5; const GLITCH: i32 = 6; const XRAY: i32 = 7; const CHROME: i32 = 8; const LIQUID: i32 = 9; const COLLAGE: i32 = 10; const NEAR: i32 = 11;
fn mp(m: i32, i: i32) -> vec4f { return u.mats[m * 2 + i]; }
/// The material an instance (or a pixel) is drawn with: its override, or the global one.
fn eff_mat(code: f32) -> i32 { if (code >= 127.5) { return i32(round(code)) % 16; } return i32(u.fd.x); }
fn hue_col(h: f32) -> vec3f { if (h > 1.5) { return u.third.rgb; } if (h > 0.5) { return u.second.rgb; } return u.accent.rgb; }
fn loose_scale() -> f32 { return u.ground2.w; }

// (strength now, signed distance to the front in device px; negative = already passed)
fn front_at(p: vec2f, r: vec4f) -> vec2f {
  let age = u.res.w - r.z;
  if (r.w <= 0.0 || age < 0.0) { return vec2f(0.0, 1e6); }
  let d = distance(p, r.xy) - age * u.fb.x * u.res.z;
  return vec2f((1.0 - smoothstep(0.0, max(u.fb.y, 1e-3), age)) * r.w, d);
}

// ── Stage 1 ───────────────────────────────────────────────────────────────────────────────────────
struct VIn {
  @builtin(vertex_index) vi: u32,
  @location(0) rect: vec4f, @location(1) uv: vec4f, @location(2) paint: vec4f, @location(3) clip: vec4f, @location(4) extra: vec4f, @location(5) look: vec4f,
};
struct VOut {
  @builtin(position) pos: vec4f,
  @location(0) uv: vec2f,
  @location(1) local: vec2f,
  @location(2) @interpolate(flat) size: vec2f,
  @location(3) @interpolate(flat) paint: vec4f,
  @location(4) @interpolate(flat) clip: vec4f,
  @location(5) @interpolate(flat) extra: vec4f,
  @location(6) @interpolate(flat) radius: f32,
  @location(7) @interpolate(flat) look: vec4f,
};

@vertex fn vs_inst(i: VIn) -> VOut { return vs_pad(i, u.fd.z); }
// The liquid's field reaches further than the nearness, as far as shapes fuse.
@vertex fn vs_soft(i: VIn) -> VOut { return vs_pad(i, max(u.fd.z, mp(LIQUID, 0).x * u.res.z)); }
fn vs_pad(i: VIn, spad: f32) -> VOut {
  let c = vec2f(f32(i.vi & 1u), f32((i.vi >> 1u) & 1u));
  // Glyph rects already include the spread; shapes and pictures are padded here so their nearness fits.
  let pad = select(spad, 0.0, i.paint.w < 0.5);
  let r = vec4f(i.rect.xy - vec2f(pad), i.rect.zw + vec2f(pad * 2.0));
  var p = r.xy + c * r.zw;
  // Each instance rides the wave: it is pushed outward a little as a front passes under it.
  let centre = i.rect.xy + i.rect.zw * 0.5;
  let w = u.fb.z * 1.3 * u.res.z;
  for (var k = 0; k < 8; k++) {
    let f = front_at(centre, u.fronts[k]);
    let dir = normalize(centre - u.fronts[k].xy + vec2f(1e-4));
    p += dir * exp(-(f.y * f.y) / (w * w)) * f.x * u.fc.x * u.res.z;
  }
  var o: VOut;
  o.pos = vec4f(p.x / u.res.x * 2.0 - 1.0, 1.0 - p.y / u.res.y * 2.0, 0.0, 1.0);
  o.local = c * r.zw - vec2f(pad);
  o.size = i.rect.zw;
  o.uv = select(i.uv.xy + o.local / i.rect.zw * (i.uv.zw - i.uv.xy), mix(i.uv.xy, i.uv.zw, c), i.paint.w < 0.5);
  o.paint = i.paint; o.clip = i.clip; o.extra = i.extra; o.radius = i.uv.x; o.look = i.look;
  return o;
}

fn box_d(local: vec2f, size: vec2f, r: f32) -> f32 {
  let rr = min(r, min(size.x, size.y) * 0.5);
  let q = abs(local - size * 0.5) - (size * 0.5 - vec2f(rr));
  return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - rr;
}

// coverage, signed nearness, picture flag, signed distance (device px, negative inside; glyphs from
// the atlas' nearness)
fn inst(i: VOut) -> vec4f {
  if (i.pos.x < i.clip.x || i.pos.y < i.clip.y || i.pos.x >= i.clip.z || i.pos.y >= i.clip.w) { discard; }
  let kind = i.paint.w;
  if (kind > 1.5) {
    // A picture with an alpha channel (a cut-out) has its true outline: outside it, nothing is there.
    var d = box_d(i.local, i.size, 0.0);
    let ta = textureSampleLevel(tex1, samp, clamp(i.uv, vec2f(0.0), vec2f(1.0)), 0.0).a;
    let inside = d <= 0.0 && ta > 0.5;
    let near = clamp(0.5 - d / (2.0 * u.fd.z), 0.0, 1.0) * step(0.5, ta);
    return vec4f(clamp(0.5 - d, 0.0, 1.0) * ta, near, select(0.0, 1.0, inside), d);
  }
  if (kind < 0.5) {
    let g = textureSampleLevel(tex0, samp, i.uv, 0.0);
    // A weight (device px in extra.x) moves the outline: the coverage comes from the signed distance.
    if (i.extra.x != 0.0) {
      let sd = (g.g - 0.5) * 2.0 * u.fd.z + i.extra.x;
      return vec4f(clamp(sd + 0.5, 0.0, 1.0), clamp(0.5 + sd / (2.0 * u.fd.z), 0.0, 1.0), 0.0, -sd);
    }
    return vec4f(g.r, g.g, 0.0, (0.5 - g.g) * 2.0 * u.fd.z);
  }
  var d = box_d(i.local, i.size, i.radius);
  if (i.extra.x > 0.0) { d = abs(d + i.extra.x * 0.5) - i.extra.x * 0.5; }
  let near = clamp(0.5 - d / (2.0 * u.fd.z), 0.0, 1.0);
  return vec4f(clamp(0.5 - d, 0.0, 1.0), near, 0.0, d);
}

/// Whether the height materials (chrome, liquid) shade this instance: shapes whose shorter side and
/// glyphs whose type size is at least their `min_px`.
fn is_big(i: VOut) -> bool {
  let m = eff_mat(i.look.x);
  if (m != CHROME && m != LIQUID) { return false; }
  let min_px = mp(CHROME, 0).w * u.res.z;
  if (i.paint.w < 0.5) { return i.look.w >= min_px * 1.25; }
  // Filled shapes, and rings whose band is wide enough to carry a bevel.
  return i.paint.w < 1.5 && min(i.size.x, i.size.y) >= min_px && (i.extra.x == 0.0 || i.extra.x >= min_px * 0.25);
}
/// The ink code in b (see the top of this file).
fn ink_code(i: VOut) -> f32 {
  if (is_big(i)) {
    var ly = (i.pos.y - i.look.z) / max(i.look.w, 1.0);
    if (i.paint.w < 0.5) { ly = fract(ly); }
    return 0.5 + 0.49 * clamp(ly, 0.0, 1.0);
  }
  return (round(i.look.y) + select(0.0, 3.0, i.paint.w > 0.5)) / 14.0;
}
/// How loose an instance is here: its own looseness and the fronts passing.
fn loose1(i: VOut) -> f32 {
  let born = clamp((u.res.w - i.extra.w) / max(u.fc.z, 1e-3), 0.0, 1.0);
  return clamp((max(i.extra.z, 1.0 - born) + wave(i.pos.xy, 0.6)) * loose_scale(), 0.0, 1.0);
}

@fragment fn fs_ink(i: VOut) -> @location(0) vec4f {
  let s = inst(i);
  var a = s.x * i.paint.z;
  if (i.paint.w > 1.5) { return vec4f(textureSampleLevel(tex1, samp, clamp(i.uv, vec2f(0.0), vec2f(1.0)), 0.0).rgb * a, a); }
  // Transparent electronics: filled shapes are drawn by their outline, a faint fill and contour lines
  // inside, so what is under them shows through. Loosened, the outlines break into moving dashes.
  if (i.paint.w < 1.5 && i.paint.w > 0.5 && i.extra.x == 0.0 && eff_mat(i.look.x) == XRAY) {
    let line = max(mp(XRAY, 0).x * u.res.z, 0.5);
    let fill = mp(XRAY, 0).y;
    let cpitch = max(mp(XRAY, 0).z * u.res.z, 2.0);
    let d = s.w;
    let k = loose1(i);
    var stroke = clamp(0.5 - d, 0.0, 1.0) - clamp(0.5 - d - line, 0.0, 1.0);
    if (k > 0.0) {
      // Dashes along the edge: the edge's own coordinate is x on the long sides and y on the short ones.
      let edge_y = min(i.local.y, i.size.y - i.local.y) > min(i.local.x, i.size.x - i.local.x);
      let along = select(i.local.x, i.local.y, edge_y) + u.res.w * 40.0 * u.res.z;
      let dash = max(mp(XRAY, 0).w * u.res.z, 2.0);
      stroke *= mix(1.0, step(fract(along / (dash * 2.0)), 0.5), smoothstep(0.05, 0.4, k));
    }
    var contour = 0.0;
    if (-d > cpitch * 0.75 && -d < cpitch * 3.5) {
      let f = abs(fract(-d / cpitch + 0.5) - 0.5) * cpitch;
      contour = clamp(line * 0.5 + 0.5 - f, 0.0, 1.0) * 0.7 * (1.0 - smoothstep(0.0, 0.6, k));
    }
    a = max(max(stroke, contour), s.x * fill * (1.0 - 0.5 * k)) * i.paint.z;
  }
  return vec4f(i.paint.x * a, i.paint.y * a, ink_code(i) * a, a);
}
fn near_of(i: VOut) -> vec4f {
  let s = inst(i);
  let born = clamp((u.res.w - i.extra.w) / max(u.fc.z, 1e-3), 0.0, 1.0);
  let loose = max(i.extra.z, 1.0 - born) * step(0.02, s.y) * step(0.01, i.paint.z);
  let slot = select(0.0, (i.paint.y + 1.0) / 8.0, s.z > 0.5);
  var code = select(0.0, round(i.extra.y * 2.0) / 255.0, i.paint.w < 0.5 && s.y > 0.02);
  // A material override reaches through the nearness of shapes and pictures, but only just around
  // glyphs, so text set in its own material does not change the shape it sits on.
  if (i.look.x >= 127.5 && s.y > select(0.02, 0.42, i.paint.w < 0.5)) { code = i.look.x / 255.0; }
  return vec4f(s.y * step(0.01, i.paint.z), slot, loose, code);
}
// The liquid's field: half at the outline, falling to nothing `fuse` px outside (glyphs only know their
// nearness that far), so two outlines closer than `fuse` add up past the threshold and join.
@fragment fn fs_soft(i: VOut) -> @location(0) vec4f {
  let s = inst(i);
  if (!is_big(i) || eff_mat(i.look.x) != LIQUID) { discard; }
  var reach = max(mp(LIQUID, 0).x * u.res.z, 1.0);
  if (i.paint.w < 0.5) { reach = min(reach, u.fd.z); }
  var f = select(0.5 * clamp(1.0 - s.w / reach, 0.0, 1.0), 0.5 + 0.5 * clamp(-s.w / reach, 0.0, 1.0), s.w < 0.0);
  if (i.paint.w < 0.5 && s.y < 0.01) { f = 0.0; }
  f *= step(0.01, i.paint.z);
  let col = mix(mix(u.bg.rgb, u.fg.rgb, i.paint.x), hue_col(i.look.y), i.paint.y);
  return vec4f(col * f * 0.5, f * 0.5);
}

// ── Stage 3: annotations, exactly, in the gaze colour (paint.y mixes toward the page ground, for
// the plates under labels) ─────────────────────────────────────────────────────────────────────────
@vertex fn vs_annot(i: VIn) -> VOut {
  let c = vec2f(f32(i.vi & 1u), f32((i.vi >> 1u) & 1u));
  let p = i.rect.xy + c * i.rect.zw;
  var o: VOut;
  o.pos = vec4f(p.x / u.res.x * 2.0 - 1.0, 1.0 - p.y / u.res.y * 2.0, 0.0, 1.0);
  o.local = c * i.rect.zw;
  o.size = i.rect.zw;
  o.uv = mix(i.uv.xy, i.uv.zw, c);
  o.paint = i.paint; o.clip = i.clip; o.extra = i.extra; o.radius = 0.0; o.look = i.look;
  return o;
}
@fragment fn fs_annot(i: VOut) -> @location(0) vec4f {
  if (i.pos.x < i.clip.x || i.pos.y < i.clip.y || i.pos.x >= i.clip.z || i.pos.y >= i.clip.w) { discard; }
  var a = 1.0;
  if (i.paint.w < 0.5) { a = textureSampleLevel(tex0, samp, i.uv, 0.0).r; }
  a *= i.paint.z;
  var c = mix(i.look.rgb, u.bg.rgb, i.paint.y);
  if (u.fd.y > 0.5) { c = pow(c, vec3f(2.2)); }
  return vec4f(c * a, a);
}
@fragment fn fs_near(i: VOut) -> @location(0) vec4f { return near_of(i); }

// The modal layer (and parts in flight), drawn without blending: wherever it has any ink it replaces what is beneath, so beneath's
// looseness and picture flag stop at its outline.
@fragment fn fs_near_top(i: VOut) -> @location(0) vec4f {
  let s = inst(i);
  if (s.x < 0.02 || i.paint.z < 0.01) { discard; }
  return near_of(i);
}

// ── Stage 2: material ─────────────────────────────────────────────────────────────────────────────
@vertex fn vs_full(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
  return vec4f(f32((vi << 1u) & 2u) * 2.0 - 1.0, f32(vi & 2u) * 2.0 - 1.0, 0.0, 1.0);
}
fn h1(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn h2(p: vec2f) -> vec2f { return vec2f(h1(p), h1(p + vec2f(19.19, 73.3))); }

/// How disturbed a point is by passing fronts: 1 on a front, a soft tail behind it.
fn wave(p: vec2f, tail_gain: f32) -> f32 {
  var k = 0.0;
  let s = u.res.z;
  let w = max(u.fb.z, 1e-3) * s;
  let tail = max(u.fb.w, 1e-3) * s;
  for (var j = 0; j < 8; j++) {
    let f = front_at(p, u.fronts[j]);
    let front = exp(-(f.y * f.y) / (w * w));
    let behind = select(0.0, exp(f.y / tail) * tail_gain, f.y < 0.0);
    k = max(k, max(front, behind) * f.x);
  }
  return k;
}
/// The half-plane source (compare wipe): `strength` right of the line, inside its rect.
fn half_plane(p: vec2f) -> f32 {
  let r = u.wipe_rect;
  let inside = p.x >= r.x && p.y >= r.y && p.x < r.z && p.y < r.w && p.x >= u.wipe.x;
  return select(0.0, u.wipe.y, inside && u.wipe.y > 0.0);
}
/// Everything in the field that disturbs a point: fronts and their tails, and the half-plane.
fn field(p: vec2f, tail_gain: f32) -> f32 { return max(wave(p, tail_gain), half_plane(p)); }
fn lens(p: vec2f) -> f32 {
  let d = (p - u.pointer.xy) / (max(u.fc.y, 1e-3) * u.res.z);
  return exp(-dot(d, d)) * u.pointer.z * step(0.5, u.fc.y);
}
fn ink(p: vec2f) -> vec4f { return textureSampleLevel(tex0, samp, p / u.res.xy, 0.0); }
fn near(p: vec2f) -> vec4f { return textureSampleLevel(tex1, samp, p / u.res.xy, 0.0); }
// Exact texels (no filtering), for averaging cells and for pitch 1 to be the image itself.
fn ink_t(p: vec2f) -> vec4f { return textureLoad(tex0, vec2i(clamp(floor(p), vec2f(0.0), u.res.xy - 1.0)), 0); }
fn near_t(p: vec2f) -> vec4f { return textureLoad(tex1, vec2i(clamp(floor(p), vec2f(0.0), u.res.xy - 1.0)), 0); }
fn unit(d: vec2f, r: f32) -> f32 {
  if (u.fc.w < 0.5) { return smoothstep(r + 0.75, r - 0.75, length(d)); }
  return smoothstep(r + 0.5, r - 0.5, max(abs(d.x), abs(d.y)));
}

// ── Pictures into marks (DESIGN.md §1.6.1) ────────────────────────────────────────────────────────
const SETS: f32 = 12.0;
const CHAR_MIN: f32 = 4.0;
const CHAR_MAX: f32 = 40.0;
/// A character tile read texel for texel at the pitch it was cut for (no resampling, so it stays sharp).
fn char_mark(col: f32, uv: vec2f, P: f32) -> f32 {
  let row = (P * (P - 1.0) - CHAR_MIN * (CHAR_MIN - 1.0)) * 0.5;
  let t = min(floor(uv * P), vec2f(P - 1.0));
  return textureLoad(chars, vec2i(i32(col * P + t.x), i32(row + t.y)), 0).r;
}
fn pv(slot: i32, i: i32) -> vec4f { return u.pics[slot * 7 + i]; }
fn lum(c: vec3f) -> f32 { return dot(c, vec3f(0.299, 0.587, 0.114)); }
fn straight(c: vec4f) -> vec3f { return c.rgb / max(c.a, 1e-3); }
/// 8 × 8 Bayer threshold in (0, 1), the same bit-interleaving as marks.rs `bayer8`.
fn bayer8(cell: vec2f) -> f32 {
  let x = u32(i32(cell.x) & 7); let y = u32(i32(cell.y) & 7);
  let a = x ^ y;
  let v = ((a & 1u) << 5u) | ((y & 1u) << 4u) | ((a & 2u) << 2u) | ((y & 2u) << 1u) | ((a & 4u) >> 1u) | ((y & 4u) >> 2u);
  return (f32(v) + 0.5) / 64.0;
}
/// Average of the ink in one cell (premultiplied), from 4 texels, or 9 when cells are large.
fn cell_avg(cell: vec2f, P: f32) -> vec4f {
  let n = select(2, 3, P > 16.0);
  var acc = vec4f(0.0);
  for (var j = 0; j < n; j++) {
    for (var i = 0; i < n; i++) {
      acc += ink_t(floor((cell + (vec2f(f32(i), f32(j)) + 0.5) / f32(n)) * P));
    }
  }
  return acc / f32(n * n);
}
fn nearest_colour(c: vec3f, row: f32, count: f32) -> vec3f {
  var best = c; var bd = 1e9;
  for (var i = 0; i < 16; i++) {
    if (f32(i) >= count) { break; }
    let q = textureLoad(palettes, vec2i(i, i32(row)), 0).rgb;
    let d = dot(c - q, c - q);
    if (d < bd) { bd = d; best = q; }
  }
  return best;
}
/// Mark (coverage, height) of set `s`, atlas column `col`, at tile coordinate `uv`, read at mip `lod`
/// and kept half a texel inside the tile so neighbouring tiles never bleed in.
fn mark(s: f32, col: f32, uv: vec2f, lod: f32) -> vec2f {
  let h = 0.5 * exp2(max(lod, 0.0)) / 32.0;
  let q = clamp(uv, vec2f(h), vec2f(1.0 - h));
  return textureSampleLevel(marks, samp, vec2f((col + q.x) / 16.0, (s + q.y) / SETS), lod).rg;
}
/// The picture's own looseness at a point: the field (fronts, half-plane), the lens, and the
/// instance's own (held, born, set at rest).
fn pic_k(q: vec2f) -> f32 { return clamp(field(q, 0.6) + lens(q) + near_t(q).b, 0.0, 1.0); }
/// Step 2: the pitch grows from 1 to `pitch` over the first half of loosening.
fn pic_p(k: f32, pitch: f32) -> f32 {
  return max(1.0, round(mix(1.0, max(1.0, round(pitch * u.res.z)), smoothstep(0.0, 0.5, k))));
}

fn picture(frag: vec2f, slot: i32) -> vec3f {
  let a0 = pv(slot, 0); let a1 = pv(slot, 1); let a2 = pv(slot, 2); let a3 = pv(slot, 3);
  let pitch = clamp(a0.x, 1.0, 64.0);
  let levels = clamp(round(a0.y), 2.0, 16.0);
  // Looseness is read at the centre of the cell the point falls in, so a cell is all one pitch.
  let p0 = pic_p(pic_k(frag), pitch);
  let k = pic_k((floor(frag / p0) + 0.5) * p0);
  // 1. Not loose: the picture itself.
  if (k <= 0.0) { return straight(ink_t(frag)); }
  let P = pic_p(k, pitch);
  // A cell of one pixel cannot hold a mark: marks, palette and parallax all come in from P = 1 to 2,
  // so pitch 1 is the picture at any looseness.
  let w = clamp(P - 1.0, 0.0, 1.0);
  var p = frag;
  if (a2.x > 0.0 && u.pointer.z > 0.0) {
    // Parallax: bright parts move further toward the pointer.
    let dir = normalize(u.pointer.xy - frag + vec2f(1e-3));
    p += dir * lum(straight(ink_t(frag))) * a2.x * u.res.z * w;
  }
  // 2–3. One lattice for the whole screen; the cell's colour is the average under it.
  let cell = floor(p / P);
  var cc = cell_avg(cell, P);
  if (cc.a < 0.5) { cc = ink_t(frag); }
  let c = straight(cc);
  let b = bayer8(cell);
  // Limited colour: the nearest palette colour after a dithered nudge.
  var cu = c;
  if (a2.w >= 1.0) { cu = mix(c, nearest_colour(c + (b - 0.5) * a1.z, clamp(a2.z, 0.0, u.fd.w - 1.0), min(a2.w, 16.0)), w); }
  // 4. Brightness into levels, dithered across the hand-over by `spread`.
  let L = pow(clamp(lum(c), 0.0, 1.0), clamp(a1.y, 0.3, 3.0));
  let n = clamp(floor(L * levels + (b - 0.5) * a1.z), 0.0, levels - 1.0);
  // Regions: a coarser lattice picks the set, by hash or by its own brightness band.
  var ms = a0.z;
  if (a3.y >= 1.0 && a3.w >= 1.0) {
    let rp = P * round(a3.y);
    let big = floor(p / rp);
    let cnt = min(a3.w, 4.0);
    var idx = floor(h1(big + 3.7) * cnt);
    if (a3.z >= 0.5) { idx = floor(clamp(lum(straight(cell_avg(big, rp))), 0.0, 0.999) * cnt); }
    ms = pv(slot, 6)[i32(clamp(idx, 0.0, 3.0))];
  }
  ms = clamp(round(ms), 0.0, SETS - 1.0);
  // 5. Inside the cell; marks turn to lie along the contours (the gradient turned a quarter).
  var uv = fract(p / P);
  if (a1.w > 0.0) {
    // Central differences of the neighbouring cells' averages, so the direction flows smoothly; where
    // the picture is flat the marks stay unturned rather than pointing at noise.
    let gx = lum(straight(cell_avg(cell + vec2f(1.0, 0.0), P))) - lum(straight(cell_avg(cell - vec2f(1.0, 0.0), P)));
    let gy = lum(straight(cell_avg(cell + vec2f(0.0, 1.0), P))) - lum(straight(cell_avg(cell - vec2f(0.0, 1.0), P)));
    let gm = abs(gx) + abs(gy);
    if (gm > 1e-3) {
      let th = (atan2(gy, gx) + 1.5707963) * a1.w * smoothstep(0.004, 0.03, gm);
      let cs = cos(th); let sn = sin(th);
      let d = uv - 0.5;
      uv = fract(vec2f(cs * d.x + sn * d.y, -sn * d.x + cs * d.y) + 0.5);
    }
  }
  // 6. The mark for this level.
  let col = round(n * 15.0 / (levels - 1.0));
  let lod = max(0.0, log2(32.0 / P));
  var m = mark(ms, col, uv, lod).x;
  let crisp = ms == 0.0 && P >= CHAR_MIN && P <= CHAR_MAX;
  if (crisp) { m = char_mark(col, uv, P); }
  // 7. Ink and ground colours. The page's ground colour is not among them.
  let mode = round(a0.w);
  var inkc = cu;
  // Characters carry the tone by which glyph is drawn; ink at the cell's own darkness would count the
  // darkness twice and dark pictures would vanish. So their original colour is lifted to full value.
  if (ms == 0.0 && mode == 0.0) { inkc = cu / max(max(cu.r, max(cu.g, cu.b)), 0.04); }
  if (mode == 1.0) { inkc = u.fg.rgb; } else if (mode == 2.0) { inkc = mix(pv(slot, 4).rgb, pv(slot, 5).rgb, L); } else if (mode == 3.0) { inkc = pv(slot, 4).rgb; }
  let g = cu * clamp(a1.x, 0.0, 1.0);
  // Relief: the height channel's slope, lit from the upper left.
  var shade = 1.0;
  if (a3.x > 0.0) {
    let e = exp2(lod) / 32.0;
    let hx = mark(ms, col, uv + vec2f(e, 0.0), lod).y - mark(ms, col, uv - vec2f(e, 0.0), lod).y;
    let hy = mark(ms, col, uv + vec2f(0.0, e), lod).y - mark(ms, col, uv - vec2f(0.0, e), lod).y;
    shade = clamp(1.0 + a3.x * (hx + hy), 0.15, 2.0);
  }
  // 8. Marks over the cell's ground, plus a glow: light from this cell and its eight neighbours,
  // each by its own brightness, falling off with distance, so bright marks bloom across their gaps.
  var glow = vec3f(0.0);
  if (a2.y > 0.0) {
    for (var j = -1; j <= 1; j++) {
      for (var i = -1; i <= 1; i++) {
        let nb = cell + vec2f(f32(i), f32(j));
        let nc = straight(ink_t((nb + 0.5) * P));
        let nl = pow(clamp(lum(nc), 0.0, 1.0), clamp(a1.y, 0.3, 3.0));
        let d = (p - (nb + 0.5) * P) / (P * (0.35 + 0.5 * nl));
        var gc = nc;
        if (mode == 1.0) { gc = u.fg.rgb; } else if (mode == 2.0) { gc = mix(pv(slot, 4).rgb, pv(slot, 5).rgb, nl); } else if (mode == 3.0) { gc = pv(slot, 4).rgb; }
        glow += gc * nl * nl * exp(-dot(d, d) * 2.0);
      }
    }
  }
  let r = mix(g, inkc * shade, m) + glow * a2.y * 0.5 * w;
  // 9. First half only coarsens; the marks come up in the second half.
  return mix(cu, r, smoothstep(0.5, 1.0, k) * w);
}

// ── Materials (DESIGN.md §1.6) ───────────────────────────────────────────────────────────────────
fn vnoise(p: vec2f) -> f32 {
  let i = floor(p); let f = fract(p); let w = f * f * (3.0 - 2.0 * f);
  return mix(mix(h1(i), h1(i + vec2f(1.0, 0.0)), w.x), mix(h1(i + vec2f(0.0, 1.0)), h1(i + vec2f(1.0, 1.0)), w.x), w.y);
}
fn rot(p: vec2f, a: f32) -> vec2f { let c = cos(a); let s = sin(a); return vec2f(c * p.x - s * p.y, s * p.x + c * p.y); }
/// The page ground here (a gradient from top to bottom when the theme has one).
fn gnd(p: vec2f) -> vec3f { return mix(u.bg.rgb, u.ground2.rgb, clamp(p.y / u.res.y, 0.0, 1.0)); }

/// Text and shape ink, decoded: straight colour and coverage, whether it is a shape, and for what the
/// height materials shade, its height coordinate (`ly` ≥ 0; −1 for everything else).
struct Ink { rgb: vec3f, a: f32, shape: bool, ly: f32, acc: f32, tone: f32 };
fn dec_at(c: vec4f, p: vec2f) -> Ink {
  var o: Ink;
  o.a = c.a; o.shape = false; o.ly = -1.0; o.rgb = gnd(p); o.acc = 0.0; o.tone = 0.0;
  if (c.a < 1e-3) { return o; }
  let t = c.r / c.a; let g = c.g / c.a; let b = c.b / c.a;
  var hue = 0.0;
  if (b >= 0.45) { o.ly = clamp((b - 0.5) / 0.49, 0.0, 1.0); }
  else { let v = round(b * 14.0); hue = v % 3.0; o.shape = v >= 2.5; }
  o.acc = g; o.tone = t;
  o.rgb = mix(mix(gnd(p), u.fg.rgb, t), hue_col(hue), g);
  return o;
}
fn dec(p: vec2f) -> Ink { return dec_at(ink_t(p), p); }
/// The exact picture of text and shapes over the ground: what every material shows at rest.
fn exact(p: vec2f) -> vec3f { let d = dec(p); return mix(gnd(p), d.rgb, d.a); }
/// Signed distance to the nearest outline (device px, positive inside), up to SPREAD either way.
fn sdist(p: vec2f) -> f32 { return (near_t(p).r - 0.5) * 2.0 * u.fd.z; }
fn sgrad(p: vec2f) -> vec2f {
  return vec2f(sdist(p + vec2f(1.0, 0.0)) - sdist(p - vec2f(1.0, 0.0)), sdist(p + vec2f(0.0, 1.0)) - sdist(p - vec2f(0.0, 1.0))) * 0.5;
}
/// The ink at the nearest outline (stepping inward along the distance's gradient), for colouring what
/// a material draws just outside the exact ink.
fn ink_inward(p: vec2f) -> Ink {
  let d = sdist(p);
  if (d > 0.5) { return dec(p); }
  let g = sgrad(p);
  let n = g / max(length(g), 1e-3);
  let q = p + n * (1.5 - d);
  // A picture's ink is its own colour, not text's: nothing to extend from there.
  if (near_t(q).g > 0.05) { var o = dec(vec2f(-10.0)); o.a = 0.0; o.shape = false; return o; }
  return dec(q);
}
fn burst_k(p: vec2f) -> f32 {
  let age = u.res.w - u.burst_t.x;
  let r = u.burst;
  if (u.burst_t.y <= 0.0 || age < 0.0 || age > u.burst_t.z || p.x < r.x || p.y < r.y || p.x >= r.z || p.y >= r.w) { return 0.0; }
  return u.burst_t.y * (1.0 - age / u.burst_t.z);
}
/// How loose text and shapes are here: fronts and their tails, the half-plane, and each instance's
/// own looseness, scaled by the style.
fn kt(p: vec2f) -> f32 { return clamp((field(p, 0.6) + near_t(p).b) * loose_scale(), 0.0, 1.0); }
/// Whatever is drawn at a point, pictures as their marks: for materials that move the picture around.
fn scene_at(q: vec2f) -> vec3f {
  let n = near_t(q);
  if (n.g > 0.05) { return picture(q, clamp(i32(round(n.g * 8.0)) - 1, 0, 7)); }
  return exact(q);
}

// plain: what is held or just born is faint rather than loose; the ground is flat. The square root
// lets a partial looseness (beneath a dialog) read clearly; fully loose is 0.3.
fn m_plain(p: vec2f) -> vec3f {
  let d = dec(p);
  return mix(gnd(p), d.rgb, d.a * mix(1.0, 0.3, sqrt(clamp(near_t(p).b * loose_scale(), 0.0, 1.0))));
}

// dot: the global lattice; text loosens into units sized by the ink under them, the ground is a field
// of small units.
fn m_dot(frag: vec2f) -> vec3f {
  let s = u.res.z; let t = u.res.w;
  var p = frag;
  // While a front is passing, some thin horizontal bands slip sideways.
  let bh = 5.0 * s; let band = floor(p.y / bh);
  let w0 = wave(vec2f(p.x, (band + 0.5) * bh), 0.6);
  let pick = step(0.55, h1(vec2f(band, floor(t * 9.0))));
  p.x += (h1(vec2f(band, floor(t * 17.0))) - 0.5) * 70.0 * s * u.fa.w * pick * smoothstep(0.2, 0.9, w0);
  let here = near_t(frag);
  let pitch = max(1.5, u.fa.x * s);
  let cell = floor(p / pitch);
  let block = (cell + 0.5) * pitch;
  let centre = block + (h2(cell) - 0.5) * u.fa.z * pitch;
  let loose = (field(centre, 0.6) + near(centre).b + (h1(cell + 7.0) - 0.5) * 0.22) * loose_scale();
  let printed = select(0.0, here.a * 255.0 * 0.5, here.a < 0.5);
  let printed_b = select(0.0, near_t(block).a * 255.0 * 0.5, near_t(block).a < 0.5);
  var col: vec3f; var alpha: f32;
  if (loose > 0.3) {
    let q = pitch * 0.25;
    let c = (ink(block + vec2f(-q, -q)) + ink(block + vec2f(q, -q)) + ink(block + vec2f(-q, q)) + ink(block + vec2f(q, q))) * 0.25;
    let grow = smoothstep(0.3, 0.42, loose);
    let d = dec_at(c, frag);
    let r = pitch * mix(0.5, 0.44 * sqrt(c.a), grow) * select(1.0, 0.96, u.fc.w > 0.5);
    let m = unit(p - centre, r) * step(0.06, c.a);
    col = d.rgb; alpha = m * mix(c.a, 1.0, grow);
  } else if (max(printed, printed_b) > 0.0) {
    // Printed type: the glyph seen through a coarse lattice, each cell filled when more than half of
    // it is covered.
    let pp = max(1.0, round(max(printed, printed_b) * s));
    let cc = cell_avg(floor(p / pp), pp);
    let d = dec_at(cc, frag);
    col = d.rgb; alpha = step(0.5, cc.a);
  } else {
    let d = dec_at(ink(p), frag);
    col = d.rgb; alpha = d.a;
  }
  // Ground: units that follow the fronts and the pointer, drift slowly at rest, and shrink away from
  // ink by its true outline (the nearness image), never by a rectangle.
  var g = gnd(frag);
  let gp = 9.0 * s; let gc = floor(frag / gp); let gcen = (gc + 0.5) * gp;
  let drift = (sin(gc.x * 0.21 + t * 0.33) + sin(gc.y * 0.37 - t * 0.21) + sin((gc.x + gc.y) * 0.13 + t * 0.17)) / 3.0;
  var lvl = max(0.0, drift - 0.25) * 0.55 * step(h1(gc), 0.4) + wave(gcen, 0.16) * 0.95 + lens(gcen) * 0.7;
  lvl *= 1.0 - smoothstep(0.1, 0.35, near(gcen).r);
  let gu = unit(frag - gcen, gp * 0.42 * clamp(lvl, 0.0, 1.0)) * step(0.05, lvl);
  g = mix(g, u.fg.rgb, gu * 0.34);
  return mix(g, col, alpha);
}

fn m_halftone(p: vec2f) -> vec3f {
  let s = u.res.z;
  let a0 = mp(HALFTONE, 0);
  let pitch = max(2.0, a0.x * s); let ang = radians(a0.y); let split = a0.z * s; let gain = a0.w;
  if (kt(p) <= 0.0 && kt(p + vec2f(pitch)) <= 0.0 && kt(p - vec2f(pitch)) <= 0.0) { return exact(p); }
  let g = gnd(p);
  let ex = exact(p);
  if (split <= 0.0) {
    let q = rot(p, ang);
    let cell = floor(q / pitch);
    let c = rot((cell + 0.5) * pitch, -ang);
    if (kt(c) <= 0.12 + 0.5 * h1(cell + vec2f(ang * 7.0, 3.0))) { return ex; }
    let e = pitch * 0.25;
    let avg = (ink_t(c + vec2f(-e, -e)) + ink_t(c + vec2f(e, -e)) + ink_t(c + vec2f(-e, e)) + ink_t(c + vec2f(e, e))) * 0.25;
    let r = pitch * 0.56 * sqrt(clamp(avg.a * gain, 0.0, 1.0));
    let m = smoothstep(r + 0.6, r - 0.6, length(q - (cell + 0.5) * pitch)) * step(0.02, avg.a);
    return mix(g, dec_at(avg, c).rgb, m);
  }
  // Three screens, one per channel, at their own angles and slightly out of register.
  var out = ex;
  let angs = vec3f(radians(15.0), radians(75.0), 0.0) + vec3f(ang - radians(45.0));
  for (var j = 0; j < 3; j++) {
    let off = vec2f(cos(f32(j) * 2.094), sin(f32(j) * 2.094)) * split;
    let q = rot(p, angs[j]);
    let cell = floor(q / pitch);
    let c = rot((cell + 0.5) * pitch, -angs[j]);
    if (kt(c) <= 0.12 + 0.5 * h1(cell + vec2f(angs[j] * 7.0, 3.0))) { continue; }
    let e = pitch * 0.25;
    let avg = (ink_t(c + off + vec2f(-e, -e)) + ink_t(c + off + vec2f(e, -e)) + ink_t(c + off + vec2f(-e, e)) + ink_t(c + off + vec2f(e, e))) * 0.25;
    let d = dec_at(avg, c);
    // How much of this channel the ink changes from the ground decides the dot.
    let amt = clamp(abs(d.rgb[j] - g[j]) / max(abs(u.fg.rgb[j] - g[j]), 0.25), 0.0, 1.0) * avg.a;
    let r = pitch * 0.56 * sqrt(clamp(amt * gain, 0.0, 1.0));
    let m = smoothstep(r + 0.6, r - 0.6, length(q - (cell + 0.5) * pitch)) * step(0.02, amt);
    out[j] = mix(g[j], d.rgb[j], m);
  }
  return out;
}

// ascii: cells of the print pitch show characters from the character mark set, chosen by the ink
// they cover; the more loose, the coarser the cells.
fn m_ascii(p: vec2f) -> vec3f {
  let s = u.res.z;
  let a0 = mp(ASCII, 0);
  let P0 = clamp(round(a0.x * s), CHAR_MIN, CHAR_MAX);
  let levels = clamp(round(a0.y), 2.0, 16.0);
  let coarse = floor(p / P0);
  let kc = kt((coarse + 0.5) * P0);
  if (kc <= 0.0 && kt(p) <= 0.0) { return exact(p); }
  let P = select(P0, max(CHAR_MIN, round(P0 * 0.5)), kc < 0.6);
  let cell = floor(p / P);
  if (kt((cell + 0.5) * P) <= 0.1 + 0.4 * h1(cell + 1.3)) { return exact(p); }
  let avg = cell_avg(cell, P);
  let d = dec_at(avg, (cell + 0.5) * P);
  let n = clamp(floor(avg.a * levels), 0.0, levels - 1.0);
  let m = char_mark(round(n * 15.0 / (levels - 1.0)), fract(p / P), P);
  let colour = select(d.rgb, u.fg.rgb, a0.z >= 0.5);
  return mix(gnd(p), colour, m);
}

// print: paper grain on the ground and accent ink out of register at rest; loosened, photocopy speckle
// and ink spreading past its outline.
fn m_print(p: vec2f) -> vec3f {
  let s = u.res.z;
  let a0 = mp(PRINT, 0);
  let grain = a0.x; let mis = a0.y * s; let speckle = a0.z; let spread = a0.w * s;
  let k = kt(p);
  var g = gnd(p) * (1.0 - grain * (0.035 * (vnoise(p * 0.45 / s) - 0.5) + 0.02 * (h1(floor(p)) - 0.5)));
  let paper = lum(g) > 0.5;
  // Key (tone) ink from here, accent ink from slightly off register.
  var dk = dec(p);
  var cov_k = dk.a * (1.0 - dk.acc);
  let off = vec2f(mis, mis * 0.6);
  let da = dec(p - off);
  var cov_a = da.a * da.acc;
  var acc_rgb = da.rgb;
  if (k > 0.0) {
    // Ink spreads out past its outline, raggedly, and thresholds like a photocopy.
    let n = vnoise(p * 0.18 / s);
    let sd = sdist(p);
    let grow = spread * k * (0.5 + n);
    if (dk.a < 0.5 && sd + grow > 0.0) {
      let w = ink_inward(p);
      let c = clamp(sd + grow + 0.5, 0.0, 1.0);
      cov_k = max(cov_k, c * (1.0 - w.acc));
      if (w.acc > 0.5) { cov_a = max(cov_a, c); acc_rgb = w.rgb; }
      dk = w;
    }
    let th = 0.5 + (vnoise(p * 0.6 / s) - 0.5) * 0.7 * k;
    cov_k = mix(cov_k, step(th, cov_k), smoothstep(0.0, 0.5, k));
    cov_a = mix(cov_a, step(th, cov_a), smoothstep(0.0, 0.5, k));
    // Toner specks on the paper and drop-outs in the ink.
    let cs = floor(p / max(1.0, round(1.4 * s)));
    let hh = h1(cs);
    if (hh > 1.0 - speckle * k * 0.07 * (0.4 + vnoise(p * 0.05 / s))) { cov_k = 1.0; dk.tone = 1.0; }
    if (h1(cs + 5.1) < speckle * k * 0.22) { cov_k *= 0.15; cov_a *= 0.15; }
  }
  var col = mix(g, mix(g, u.fg.rgb, dk.tone), cov_k);
  if (paper) { col = col * mix(vec3f(1.0), acc_rgb / max(g, vec3f(0.05)), cov_a); }
  else { col = mix(col, acc_rgb, cov_a); }
  return clamp(col, vec3f(0.0), vec3f(1.0));
}

// lino: ink or ground, nothing between. Loosened, the outline is cut by hand (displaced by noise) and
// a band around it fills with gouge strokes that follow the outline: ridges of ink left standing
// outside, white cuts inside; the band widens with the looseness until shapes break into strokes.
fn m_lino(p: vec2f) -> vec3f {
  let s = u.res.z;
  let a0 = mp(LINO, 0); let a1 = mp(LINO, 1);
  let k = kt(p);
  if (k <= 0.0) { return exact(p); }
  let carve = a0.x; let band = min(a0.y * s * k, u.fd.z - 0.5); let slen = max(a0.z * s, 2.0); let spitch = max(a0.w * s, 2.0); let wob = a1.x;
  let sd = sdist(p);
  let w = ink_inward(p);
  if (w.a < 0.02 && sd < -u.fd.z + 0.6) { return gnd(p); }
  // The cut edge: a slow wander of the knife and small chips.
  let n = vnoise(p / (slen * 2.0)) - 0.5;
  let chip = vnoise(p / (slen * 0.35) + 3.0) - 0.5;
  let cut = sd + (n * 2.0 + chip * 0.8) * carve * (1.0 + 1.5 * k) * s;
  var inked = smoothstep(-0.6, 0.6, cut);
  if (abs(sd) < band && abs(sd) < u.fd.z - 0.5) {
    let g = sgrad(p);
    let nrm = g / max(length(g), 1e-3);
    let tng = vec2f(-nrm.y, nrm.x);
    let wv = wob * spitch * (vnoise(p / slen + 7.0) - 0.5);
    let along = dot(p, tng) / spitch + wv / spitch;
    let lane = floor(along);
    if (sd > 0.0) {
      // Gouges: white wedges cut in from the edge, each of its own length.
      let len = band * (0.35 + 0.65 * h1(vec2f(lane, 4.0)));
      let width = 0.42 * clamp(1.0 - sd / max(len, 1e-3), 0.0, 1.0);
      let g1 = smoothstep(width + 0.06, width - 0.06, abs(fract(along) - 0.5)) * step(0.4, h1(vec2f(lane, 9.0)));
      inked = min(inked, 1.0 - g1);
    } else {
      // Ridges of ink left standing outside, echoing the edge, in long broken lines.
      let echo = -sd / (spitch * 1.6) + wv / spitch * 0.4;
      let on_line = smoothstep(0.6 * s, 0.0, abs(fract(echo + 0.5) - 0.5) * spitch * 1.6 - 0.4 * s) * step(0.6, -sd / spitch);
      let dash = step(0.35, vnoise(vec2f(dot(p, tng) / (slen * 1.5), floor(echo + 0.5) * 3.1)));
      inked = max(inked, on_line * dash);
    }
  }
  let lino = mix(gnd(p), select(w.rgb, u.fg.rgb, w.a < 0.02), inked);
  return mix(exact(p), lino, smoothstep(0.0, 0.12, k));
}

// glitch: nothing at rest. Loosened, rows slip sideways (held for `hold` ms), the channels split and
// blocks repeat; bursts (hover, press) do the same for a moment.
fn m_glitch(p: vec2f, pic: bool) -> vec3f {
  let s = u.res.z;
  let a0 = mp(GLITCH, 0); let a1 = mp(GLITCH, 1);
  let slice = max(a0.x * s, 1.0); let shift = a0.y * s; let split = a0.z * s; let hold = max(a0.w, 16.0); let block = max(a1.x * s, 2.0);
  let row = floor(p.y / slice);
  let rc = vec2f(p.x, (row + 0.5) * slice);
  let kr = clamp(kt(rc) + burst_k(rc), 0.0, 1.0);
  let here = select(exact(p), scene_at(p), pic);
  if (kr <= 0.0) { return here; }
  let tq = floor(u.res.w * 1000.0 / hold);
  var q = p;
  if (h1(vec2f(row, tq)) < kr * 0.7) { q.x += (h1(vec2f(row * 1.7, tq + 3.0)) - 0.5) * 2.0 * shift * kr; }
  let bc = floor(p / block);
  if (h1(bc + vec2f(tq * 0.37, 1.0)) < kr * 0.16) { q.x -= block * (1.0 + floor(h1(bc + 9.0) * 2.0)); }
  let sp = vec2f(split * kr, 0.0);
  // A picture keeps to itself: a sample that would leave it stays where it is.
  var qr = q + sp; var qg = q; var qb = q - sp;
  if (pic) {
    if (near_t(qr).g <= 0.05) { qr = p; }
    if (near_t(qg).g <= 0.05) { qg = p; }
    if (near_t(qb).g <= 0.05) { qb = p; }
  }
  var c = vec3f(scene_at(qr).r, scene_at(qg).g, scene_at(qb).b);
  if (h1(vec2f(row, tq + 11.0)) < kr * 0.12) { c = c * 0.75 + 0.25 * (1.0 - c) * step(0.5, h1(vec2f(row, tq + 2.0))); }
  return c;
}

// xray: shapes are drawn by outline in stage 1; here loosened text turns hollow.
fn m_xray(p: vec2f) -> vec3f {
  let k = kt(p);
  let d = dec(p);
  if (k <= 0.0 || d.shape || d.a <= 0.0) { return exact(p); }
  let line = max(mp(XRAY, 0).x * u.res.z, 0.5);
  let hollow = 1.0 - smoothstep(line + 0.3, line + 1.0, sdist(p));
  return mix(gnd(p), d.rgb, d.a * mix(1.0, hollow, smoothstep(0.1, 0.6, k)));
}

/// Classic chrome: the reflected direction looks up a band of sky over a hard horizon: deep blue-black
/// overhead, bright white-cyan just above the horizon, a dark horizon line, warm brown and orange
/// just below, a pale ground further down. `e` is where on that band it lands (negative = sky).
fn chrome_env(e: f32, x: f32) -> vec3f {
  let sky = mix(mix(vec3f(0.02, 0.04, 0.16), vec3f(0.30, 0.52, 0.92), smoothstep(-0.75, -0.28, e)), vec3f(0.92, 1.0, 1.0), smoothstep(-0.30, -0.03, e));
  let ground = mix(mix(vec3f(0.30, 0.14, 0.05), vec3f(0.95, 0.52, 0.18), smoothstep(0.02, 0.18, e)), vec3f(1.0, 0.94, 0.84), smoothstep(0.22, 0.6, e));
  var c = select(ground, sky, e < 0.0);
  c = mix(c, vec3f(0.02, 0.02, 0.05), smoothstep(0.028, 0.012, abs(e - 0.004)));
  return c + vec3f(0.04) * sin(x * 0.02);
}
/// The height field of the height materials: 0 at the outline, 1 at `bevel` px inside.
fn chrome_h(q: vec2f, bevel: f32) -> f32 { return smoothstep(0.0, bevel, sdist(q)); }
/// Metal at a point, at looseness `k` (0..0.6 the bevel softens and dulls before anything breaks up).
fn chrome_shade(p: vec2f, ly: f32, k: f32) -> vec3f {
  let a0 = mp(CHROME, 0); let a1 = mp(CHROME, 1);
  let soft = smoothstep(0.0, 0.6, k);
  let bevel = clamp(a0.x * u.res.z * (1.0 + soft), 1.0, u.fd.z - 0.5);
  let hx = chrome_h(p + vec2f(1.0, 0.0), bevel) - chrome_h(p - vec2f(1.0, 0.0), bevel);
  let hy = chrome_h(p + vec2f(0.0, 1.0), bevel) - chrome_h(p - vec2f(0.0, 1.0), bevel);
  let n = normalize(vec3f(-hx * bevel * 0.8, -hy * bevel * 0.8, 1.0));
  // The band is read by the height in the line and by the normal's y: bevels facing up see the sky,
  // facing down the ground.
  let e = (ly - 0.5) * a0.y * 1.15 - n.y * 0.85 * (1.0 - 0.5 * soft);
  var c = chrome_env(e, p.x + n.x * 60.0);
  // A sharp white rim on the bevels that face the light (upper left), and a soft sheen.
  let facing = dot(normalize(n.xy + vec2f(1e-4)), normalize(vec2f(-0.7, -0.75))) * length(n.xy) / 0.62;
  let rim = smoothstep(0.55, 0.8, facing) * a0.z * (1.0 - soft);
  let hv = normalize(vec3f(-0.45, -0.6, 0.66) + vec3f(0.0, 0.0, 1.0));
  let sp = pow(max(dot(n, hv), 0.0), 60.0) * a0.z * 0.9;
  c = mix(c, vec3f(1.0), clamp(rim + sp, 0.0, 1.0));
  // A dark rim right at the outline keeps the letterform crisp.
  c *= mix(0.25, 1.0, smoothstep(0.0, 1.2 * u.res.z, sdist(p)));
  c = mix(c, c * (u.accent.rgb * 1.6 + 0.2), a1.x);
  return clamp(c, vec3f(0.0), vec3f(1.0));
}
/// Whether a point just outside the ink belongs to a big chrome element: a thin dark outline there.
fn chrome_outline(p: vec2f) -> f32 {
  let sd = sdist(p);
  if (sd >= 0.0 || sd < -1.6 * u.res.z - 0.5) { return 0.0; }
  let w = ink_inward(p);
  if (w.a < 0.02 || w.ly < 0.0) { return 0.0; }
  return smoothstep(-1.6 * u.res.z - 0.5, -0.8 * u.res.z, sd);
}
// chrome: large shapes and type become bevelled metal at rest (its one rest treatment), with a thin
// dark outline; small text stays flat. Loosened, the bevel softens first; past 0.6 it melts into
// dots of the same metal.
fn m_chrome(p: vec2f) -> vec3f {
  let d = dec(p);
  let k = kt(p);
  let s = u.res.z;
  if (k > 0.6) {
    let pitch = max(1.5, u.fa.x * s * 1.4);
    let cell = floor(p / pitch);
    let centre = (cell + 0.5) * pitch;
    let kc = kt(centre);
    if (kc > 0.62 + 0.35 * h1(cell + 2.2)) {
      let e = pitch * 0.25;
      let avg = (ink_t(centre + vec2f(-e, -e)) + ink_t(centre + vec2f(e, -e)) + ink_t(centre + vec2f(-e, e)) + ink_t(centre + vec2f(e, e))) * 0.25;
      let dc = dec_at(avg, centre);
      let r = pitch * 0.52 * sqrt(avg.a);
      let m = smoothstep(r + 0.6, r - 0.6, length(p - centre)) * step(0.05, avg.a);
      var col = dc.rgb;
      if (dc.ly >= 0.0) {
        // A little sphere of chrome per dot.
        let l = (p - centre) / max(r, 1.0);
        let nz = sqrt(max(1.0 - dot(l, l), 0.0));
        col = chrome_env((dc.ly - 0.5) * mp(CHROME, 0).y + l.y * 0.8, p.x) + vec3f(pow(max(dot(normalize(vec3f(l, nz)), normalize(vec3f(-0.3, -0.5, 1.0))), 0.0), 30.0)) * 0.8;
      }
      return mix(gnd(p), col, m);
    }
  }
  if (d.ly < 0.0) {
    let o = chrome_outline(p);
    return mix(exact(p), vec3f(0.03, 0.03, 0.08), o * (1.0 - smoothstep(0.3, 0.6, k)));
  }
  return mix(gnd(p), chrome_shade(p, d.ly, k), d.a);
}

// liquid: the large shapes join where they come closer than `fuse` (a metaball field from stage 1),
// shaded as a glossy liquid that bends what is behind its thin edges; the pointer pulls a bulge out
// of it. Loosened, it drips.
fn soft_t(p: vec2f) -> vec4f { return textureLoad(soft, vec2i(clamp(floor(p), vec2f(0.0), u.res.xy - 1.0)), 0); }
fn lfield(p: vec2f) -> f32 {
  var f = soft_t(p).a * 2.0;
  let d = (p - u.pointer.xy) / (max(u.fc.y, 1e-3) * 0.45 * u.res.z);
  // The pointer is a blob of its own only next to the liquid, so it pulls a bulge rather than drawing.
  f += exp(-dot(d, d)) * 0.62 * u.pointer.z * smoothstep(0.05, 0.3, f);
  return f;
}
fn lfield_drip(p: vec2f, k: f32) -> f32 {
  var f = lfield(p);
  if (k > 0.0) {
    let s = u.res.z;
    let colx = floor(p.x / (4.0 * s));
    let len = mp(LIQUID, 0).w * s * k * pow(h1(vec2f(colx, 4.0)), 2.0);
    for (var j = 1; j <= 4; j++) {
      let t = f32(j) / 4.0;
      f = max(f, lfield(p - vec2f(0.0, len * t)) - t * 0.18 - abs(fract(p.x / (4.0 * s)) - 0.5) * 0.25 * t);
    }
  }
  return f;
}
fn m_liquid(p: vec2f) -> vec3f {
  let a0 = mp(LIQUID, 0);
  let s = u.res.z;
  let k = kt(p);
  let f = lfield_drip(p, k);
  let base = exact(p);
  if (f < 0.42) { return base; }
  let e = 1.5;
  let gx = lfield_drip(p + vec2f(e, 0.0), k) - lfield_drip(p - vec2f(e, 0.0), k);
  let gy = lfield_drip(p + vec2f(0.0, e), k) - lfield_drip(p - vec2f(0.0, e), k);
  let gl = max(length(vec2f(gx, gy)) / (2.0 * e), 1e-3);
  let alpha = clamp((f - 0.5) / gl + 0.5, 0.0, 1.0);
  if (alpha <= 0.0) { return base; }
  let h = smoothstep(0.5, 0.75, f);
  let n = normalize(vec3f(-gx * 3.0, -gy * 3.0, 1.0) * vec3f(1.0, 1.0, 0.6 + h));
  // Colour: the field's own, so joins between two colours blend.
  var sc = soft_t(p);
  var colr = select(u.accent.rgb, sc.rgb / max(sc.a, 1e-3), sc.a > 0.004);
  // Behind its thin edges, the page is bent by the surface.
  let behind = gnd(p + n.xy * a0.y * s * 4.0) ;
  let body = mix(behind, colr * (0.72 + 0.38 * h), 0.6 + 0.4 * smoothstep(0.0, 0.4, h));
  let r = vec3f(2.0 * n.z * n.x, 2.0 * n.z * n.y, 2.0 * n.z * n.z - 1.0);
  let env = mix(vec3f(1.0), vec3f(0.55, 0.6, 0.75), smoothstep(-0.3, 0.6, r.y));
  let hv = normalize(vec3f(-0.4, -0.65, 0.65) + vec3f(0.0, 0.0, 1.0));
  let sp = pow(max(dot(n, hv), 0.0), 90.0) * 1.5 + pow(max(dot(n, hv), 0.0), 10.0) * 0.18;
  let fres = pow(1.0 - n.z, 2.5);
  var c = body + (env * fres * 0.55 + vec3f(sp)) * a0.z;
  c = mix(c, c * 0.55, smoothstep(0.55, 0.5, f) * 0.6);
  return mix(base, clamp(c, vec3f(0.0), vec3f(1.0)), alpha);
}

// collage: the base of mixed media. Shapes are torn paper: their outline wanders by low-frequency
// noise, with a lighter lip of paper fibre and a small shadow. Text is untouched; other materials come
// in per element (`El::material`).
fn m_collage(p: vec2f) -> vec3f {
  let s = u.res.z;
  let a0 = mp(COLLAGE, 0);
  let k = kt(p);
  let tear = a0.x * s * (1.0 + 1.5 * k); let lip = a0.y * s;
  let sd = sdist(p);
  let w = ink_inward(p);
  let d = dec(p);
  if (!w.shape || (d.a > 0.5 && !d.shape)) { return exact(p); }
  let nsd = sd + (vnoise(p / (22.0 * s)) - 0.5) * 2.0 * tear + (vnoise(p / (4.0 * s)) - 0.5) * tear * 0.35 - tear * 0.2;
  // Shadow, from the same edge a little up and to the left.
  let ps = p - vec2f(2.0, 3.0) * s;
  let sh = sdist(ps) + (vnoise(ps / (22.0 * s)) - 0.5) * 2.0 * tear - tear * 0.2;
  var g = gnd(p) * (1.0 - 0.16 * smoothstep(-3.0 * s, 1.0, sh) * step(nsd, 0.0));
  let paper = mix(u.third.rgb, vec3f(1.0), 0.4);
  g = mix(g, paper, smoothstep(-lip - 0.6, -lip + 0.6, nsd) * step(nsd, 0.0));
  let inside = smoothstep(-0.6, 0.6, nsd);
  let top = select(w.rgb, d.rgb, d.a > 0.5);
  return mix(g, top, inside * max(w.a, d.a));
}

// near: the intermediate images made visible. The signed nearness in bands every `pitch` px, warm
// inside the ink and cool outside, fading toward the edge of its reach; the outline in the ink colour;
// the ink itself as a faint fill; the looseness of each instance as a tint over it.
fn m_near(p: vec2f) -> vec3f {
  let s = u.res.z;
  let a0 = mp(NEAR, 0);
  let n = near_t(p);
  let g = gnd(p);
  let d = dec(p);
  var col = mix(g, d.rgb, d.a * a0.y);
  if (n.r <= 0.001) { return col; }
  let sd = sdist(p);
  let band = max(a0.x * s, 1.0);
  let f = abs(fract(sd / band + 0.5) - 0.5) * band;
  let line = clamp(1.0 - f / (0.6 * s + 0.2), 0.0, 1.0) * (1.0 - smoothstep(0.3, 1.0, abs(sd) / u.fd.z));
  let tint = select(u.second.rgb, u.accent.rgb, sd > 0.0);
  col = mix(col, tint, line * 0.85);
  col = mix(col, u.fg.rgb, clamp(1.0 - abs(sd) / (0.8 * s), 0.0, 1.0));
  col = mix(col, u.third.rgb, clamp(n.b + field(p, 0.6), 0.0, 1.0) * 0.35 * step(0.5, n.r));
  return col;
}

@fragment fn fs_material(@builtin(position) frag: vec4f) -> @location(0) vec4f {
  let s = u.res.z; let t = u.res.w;
  let p = frag.xy;
  let here = near_t(p);
  let is_pic = here.g > 0.05;
  let m = eff_mat(here.a * 255.0);
  var col: vec3f;
  if (is_pic && m != GLITCH) {
    // A picture is drawn from its own colours only (DESIGN.md §1.6.1), in every material.
    let slot = clamp(i32(round(here.g * 8.0)) - 1, 0, 7);
    var q = p;
    if (m == DOT) {
      // Glitch slip of the dot material: a slip that would leave the picture keeps it in place.
      let bh = 5.0 * s; let band = floor(p.y / bh);
      let w0 = wave(vec2f(p.x, (band + 0.5) * bh), 0.6);
      let pick = step(0.55, h1(vec2f(band, floor(t * 9.0))));
      q.x += (h1(vec2f(band, floor(t * 17.0))) - 0.5) * 70.0 * s * u.fa.w * pick * smoothstep(0.2, 0.9, w0);
      q = select(p, q, near_t(q).g > 0.05);
    }
    col = picture(q, slot);
    // The compare wipe's line.
    let wx = floor(u.wipe.x);
    let r = u.wipe_rect;
    if (u.wipe.y > 0.0 && p.x >= wx && p.x < wx + max(1.0, round(s)) && p.y >= r.y && p.y < r.w) { col = u.accent.rgb; }
  } else {
    switch m {
      case 1: { col = m_dot(p); }
      case 2: { col = m_halftone(p); }
      case 3: { col = m_ascii(p); }
      case 4: { col = m_print(p); }
      case 5: { col = m_lino(p); }
      case 6: { col = m_glitch(p, is_pic); }
      case 7: { col = m_xray(p); }
      case 8: { col = m_chrome(p); }
      case 9: { col = m_liquid(p); }
      case 10: { col = m_collage(p); }
      case 11: { col = m_near(p); }
      default: { col = m_plain(p); }
    }
  }
  if (u.fd.y > 0.5) { col = pow(col, vec3f(2.2)); }
  // A commit: beyond the front travelling out from where it was pressed, the frame is still the old
  // one (already encoded for the surface).
  if (u.split.w > 0.5) {
    let r = (t - u.split.z) * u.fb.x * s;
    if (distance(frag.xy, u.split.xy) > r) {
      col = textureLoad(before, vec2i(clamp(floor(frag.xy), vec2f(0.0), u.res.xy - 1.0)), 0).rgb;
    }
  }
  return vec4f(col, 1.0);
}
