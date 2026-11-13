// Two stages.
//   Stage 1 draws every instance exactly, into two intermediate images (same geometry, drawn twice):
//     ink  (rgba8): what is there. Glyphs and shapes: r = tone, g = accent, a = coverage (premultiplied).
//                   Pictures: rgb = their colour, a = coverage.
//     near (rgba8, max-blended): r = how close this pixel is to the nearest ink, by true outline;
//                   g = where a picture is, (PicFx slot + 1) / 8; b = the instance's own looseness
//                   (held, born, or set at rest); a = print pitch of glyphs / 64 (0 = not printed).
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
  fronts: array<vec4f, 8>, // x, y, start time, strength
  pics: array<vec4f, 56>,  // 8 PicFx slots × 7 (see style.rs PicFx::pack)
};
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var tex0: texture_2d<f32>;
@group(0) @binding(3) var tex1: texture_2d<f32>;
@group(0) @binding(4) var marks: texture_2d<f32>;   // rg: mark coverage, height; rows = sets, columns = levels
@group(0) @binding(5) var palettes: texture_2d<f32>; // 16 × rows

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
  @location(0) rect: vec4f, @location(1) uv: vec4f, @location(2) paint: vec4f, @location(3) clip: vec4f, @location(4) extra: vec4f,
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
};

@vertex fn vs_inst(i: VIn) -> VOut {
  let c = vec2f(f32(i.vi & 1u), f32((i.vi >> 1u) & 1u));
  // Glyph rects already include the spread; shapes and pictures are padded here so their nearness fits.
  let pad = select(u.fd.z, 0.0, i.paint.w < 0.5);
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
  o.paint = i.paint; o.clip = i.clip; o.extra = i.extra; o.radius = i.uv.x;
  return o;
}

fn box_d(local: vec2f, size: vec2f, r: f32) -> f32 {
  let rr = min(r, min(size.x, size.y) * 0.5);
  let q = abs(local - size * 0.5) - (size * 0.5 - vec2f(rr));
  return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - rr;
}

// coverage, nearness, picture flag
fn inst(i: VOut) -> vec3f {
  if (i.pos.x < i.clip.x || i.pos.y < i.clip.y || i.pos.x >= i.clip.z || i.pos.y >= i.clip.w) { discard; }
  let kind = i.paint.w;
  if (kind < 0.5) {
    let g = textureSampleLevel(tex0, samp, i.uv, 0.0);
    return vec3f(g.r, g.g, 0.0);
  }
  var d = box_d(i.local, i.size, select(i.radius, 0.0, kind > 1.5));
  if (i.extra.x > 0.0) { d = abs(d + i.extra.x * 0.5) - i.extra.x * 0.5; }
  let near = clamp(1.0 - max(d, 0.0) / u.fd.z, 0.0, 1.0);
  return vec3f(clamp(0.5 - d, 0.0, 1.0), near, select(0.0, 1.0, kind > 1.5 && d <= 0.0));
}

@fragment fn fs_ink(i: VOut) -> @location(0) vec4f {
  let s = inst(i);
  let a = s.x * i.paint.z;
  if (i.paint.w > 1.5) { return vec4f(textureSampleLevel(tex1, samp, clamp(i.uv, vec2f(0.0), vec2f(1.0)), 0.0).rgb * a, a); }
  return vec4f(i.paint.x * a, i.paint.y * a, 0.0, a);
}
@fragment fn fs_near(i: VOut) -> @location(0) vec4f {
  let s = inst(i);
  let born = clamp((u.res.w - i.extra.w) / max(u.fc.z, 1e-3), 0.0, 1.0);
  let loose = max(i.extra.z, 1.0 - born) * step(0.02, s.y) * step(0.01, i.paint.z);
  let slot = select(0.0, (i.paint.y + 1.0) / 8.0, s.z > 0.5);
  let print = select(0.0, i.extra.y / 64.0, i.paint.w < 0.5 && s.y > 0.02);
  return vec4f(s.y * step(0.01, i.paint.z), slot, loose, print);
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
  let m = mark(ms, col, uv, lod).x;
  // 7. Ink and ground colours. The page's ground colour is not among them.
  let mode = round(a0.w);
  var inkc = cu;
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

@fragment fn fs_material(@builtin(position) frag: vec4f) -> @location(0) vec4f {
  let s = u.res.z; let t = u.res.w;
  let dot_material = u.fd.x > 0.5;
  var p = frag.xy;

  // Glitch: while a front is passing, some thin horizontal bands slip sideways.
  let bh = 5.0 * s; let band = floor(p.y / bh);
  let w0 = wave(vec2f(p.x, (band + 0.5) * bh), 0.6);
  let pick = step(0.55, h1(vec2f(band, floor(t * 9.0))));
  p.x += (h1(vec2f(band, floor(t * 17.0))) - 0.5) * 70.0 * s * u.fa.w * pick * smoothstep(0.2, 0.9, w0);

  let here = near_t(frag.xy);
  let is_pic = here.g > 0.05;
  var a: vec4f; // rgb: tone, accent (or picture colour); a: coverage
  if (is_pic) {
    // A picture is drawn from its own colours only; a slip that would leave it keeps it in place.
    let slot = clamp(i32(round(here.g * 8.0)) - 1, 0, 7);
    let q = select(frag.xy, p, near_t(p).g > 0.05);
    a = vec4f(picture(q, slot), 1.0);
    // The compare wipe's line.
    let wx = floor(u.wipe.x);
    let r = u.wipe_rect;
    if (u.wipe.y > 0.0 && frag.x >= wx && frag.x < wx + max(1.0, round(s)) && frag.y >= r.y && frag.y < r.w) { a = vec4f(u.accent.rgb, 1.0); }
  } else {
    // The lattice this point belongs to. Its pitch is a parameter; jitter moves each unit off the grid.
    let pitch = max(1.5, u.fa.x * s);
    let cell = floor(p / pitch);
    let block = (cell + 0.5) * pitch;
    let centre = block + (h2(cell) - 0.5) * u.fa.z * pitch;
    // Fronts loosen everything; each instance adds its own looseness (held down, or just born).
    let own = near(centre).b;
    let loose = select(0.0, field(centre, 0.6) + own + (h1(cell + 7.0) - 0.5) * 0.22, dot_material);
    let printed = max(here.a, near_t(block).a) * 64.0;
    if (loose > 0.3) {
      let q = pitch * 0.25;
      let c = (ink(block + vec2f(-q, -q)) + ink(block + vec2f(q, -q)) + ink(block + vec2f(-q, q)) + ink(block + vec2f(q, q))) * 0.25;
      let grow = smoothstep(0.3, 0.42, loose);
      let col = c.rgb / max(c.a, 1e-3);
      // Text and shapes have no ground of their own: they loosen into units sized by their ink.
      let r = pitch * mix(0.5, 0.44 * sqrt(c.a), grow) * select(1.0, 0.96, u.fc.w > 0.5);
      let m = unit(p - centre, r) * step(0.06, c.a);
      a = vec4f(col, m * mix(c.a, 1.0, grow));
    } else if (printed > 0.0) {
      // Printed type: the glyph seen through a coarse lattice, each cell filled when more than half
      // of it is covered.
      let pp = max(1.0, round(printed * s));
      let cc = cell_avg(floor(p / pp), pp);
      a = vec4f(cc.rgb / max(cc.a, 1e-3), step(0.5, cc.a));
    } else {
      let c = ink(p);
      a = vec4f(c.rgb / max(c.a, 1e-3), c.a);
    }
    // Plain material: what is held or just born is faint rather than loose.
    if (!dot_material) { a.a *= mix(1.0, 0.3, here.b); }
  }

  var col = u.bg.rgb;
  if (dot_material) {
    // Ground: a field of small units. They follow the fronts and the pointer, drift slowly at rest, and
    // shrink away from ink by its true outline (the nearness image), never by a rectangle.
    let gp = 9.0 * s; let gc = floor(frag.xy / gp); let gcen = (gc + 0.5) * gp;
    let drift = (sin(gc.x * 0.21 + t * 0.33) + sin(gc.y * 0.37 - t * 0.21) + sin((gc.x + gc.y) * 0.13 + t * 0.17)) / 3.0;
    var lvl = max(0.0, drift - 0.25) * 0.55 * step(h1(gc), 0.4) + wave(gcen, 0.16) * 0.95 + lens(gcen) * 0.7;
    lvl *= 1.0 - smoothstep(0.2, 0.7, near(gcen).r);
    let g = unit(frag.xy - gcen, gp * 0.42 * clamp(lvl, 0.0, 1.0)) * step(0.05, lvl);
    col = mix(col, u.fg.rgb, g * 0.34);
  }
  let inkc = select(mix(mix(u.bg.rgb, u.fg.rgb, a.r), u.accent.rgb, a.g), a.rgb, is_pic);
  col = mix(col, inkc, a.a);
  if (u.fd.y > 0.5) { col = pow(col, vec3f(2.2)); }
  return vec4f(col, 1.0);
}
