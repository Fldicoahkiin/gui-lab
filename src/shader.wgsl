// Two stages.
//   Stage 1 draws every instance exactly, into two intermediate images (same geometry, drawn twice):
//     ink  (rgba8): what is there. Glyphs and shapes: r = tone, g = accent, a = coverage (premultiplied).
//                   Pictures: rgb = their colour, a = coverage.
//     near (rgba8, max-blended): r = how close this pixel is to the nearest ink, by true outline;
//                   g = 1 where a picture is; b = the instance's own looseness (held, or just born).
//   Stage 2 (the material) reads both plus the field and decides the final pixel.

struct U {
  res: vec4f,      // width, height, scale, time
  fa: vec4f,       // pitch, picture pitch, jitter, glitch          (lengths in logical px)
  fb: vec4f,       // wave speed, wave life, wave width, wave tail
  fc: vec4f,       // push, lens radius, enter time, square (0 dot, 1 square)
  fd: vec4f,       // light, srgb surface, spread px, _
  bg: vec4f,
  fg: vec4f,
  accent: vec4f,
  pointer: vec4f,  // x, y, strength, _    (device px)
  fronts: array<vec4f, 8>, // x, y, start time, strength
};
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var tex0: texture_2d<f32>;
@group(0) @binding(3) var tex1: texture_2d<f32>;

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
  return vec4f(s.y * step(0.01, i.paint.z), s.z, loose, 0.0);
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
fn lens(p: vec2f) -> f32 {
  let d = (p - u.pointer.xy) / (max(u.fc.y, 1e-3) * u.res.z);
  return exp(-dot(d, d)) * u.pointer.z * step(0.5, u.fc.y);
}
fn ink(p: vec2f) -> vec4f { return textureSampleLevel(tex0, samp, p / u.res.xy, 0.0); }
fn near(p: vec2f) -> vec4f { return textureSampleLevel(tex1, samp, p / u.res.xy, 0.0); }
fn unit(d: vec2f, r: f32) -> f32 {
  if (u.fc.w < 0.5) { return smoothstep(r + 0.75, r - 0.75, length(d)); }
  return smoothstep(r + 0.5, r - 0.5, max(abs(d.x), abs(d.y)));
}

@fragment fn fs_material(@builtin(position) frag: vec4f) -> @location(0) vec4f {
  let s = u.res.z; let t = u.res.w;
  var p = frag.xy;

  // Glitch: while a front is passing, some thin horizontal bands slip sideways.
  let bh = 5.0 * s; let band = floor(p.y / bh);
  let w0 = wave(vec2f(p.x, (band + 0.5) * bh), 0.6);
  let pick = step(0.55, h1(vec2f(band, floor(t * 9.0))));
  p.x += (h1(vec2f(band, floor(t * 17.0))) - 0.5) * 70.0 * s * u.fa.w * pick * smoothstep(0.2, 0.9, w0);

  let is_pic = near(p).g > 0.5;

  // The lattice this point belongs to. Its pitch is a parameter; jitter moves each unit off the grid.
  let pitch = max(1.5, select(u.fa.x, u.fa.y, is_pic) * s);
  let cell = floor(p / pitch);
  let block = (cell + 0.5) * pitch;
  let centre = block + (h2(cell) - 0.5) * u.fa.z * pitch;
  // Fronts loosen everything; the pointer loosens only pictures, so reading is never disturbed by it.
  // Each instance adds its own looseness (held down, or just born) from near.b.
  let loose = wave(centre, 0.6) + select(0.0, lens(centre), is_pic) + near(centre).b + (h1(cell + 7.0) - 0.5) * 0.22;

  var a: vec4f; // rgb: tone, accent (or picture colour); a: coverage
  if (loose > 0.3) {
    let q = pitch * 0.25;
    let c = (ink(block + vec2f(-q, -q)) + ink(block + vec2f(q, -q)) + ink(block + vec2f(-q, q)) + ink(block + vec2f(q, q))) * 0.25;
    let grow = smoothstep(0.3, 0.42, loose);
    let col = c.rgb / max(c.a, 1e-3);
    if (is_pic) {
      // A picture becomes its own marks: first seamless blocks of the colour under them, then a unit
      // inside each block over a darkened copy of that colour. Its gaps never show the page.
      let m = unit(p - centre, pitch * 0.46 * select(1.0, 0.96, u.fc.w > 0.5));
      a = vec4f(col * mix(1.0, mix(0.28, 1.0, m), grow), step(0.5, c.a));
    } else {
      // Text and shapes have no ground of their own: they loosen into units sized by their ink.
      let r = pitch * mix(0.5, 0.44 * sqrt(c.a), grow) * select(1.0, 0.96, u.fc.w > 0.5);
      let m = unit(p - centre, r) * step(0.06, c.a);
      a = vec4f(col, m * mix(c.a, 1.0, grow));
    }
  } else {
    let c = ink(p);
    a = vec4f(c.rgb / max(c.a, 1e-3), c.a);
  }

  // Ground: a field of small units. They follow the fronts and the pointer, drift slowly at rest, and
  // shrink away from ink by its true outline (the nearness image), never by a rectangle.
  let gp = 9.0 * s; let gc = floor(frag.xy / gp); let gcen = (gc + 0.5) * gp;
  let drift = (sin(gc.x * 0.21 + t * 0.33) + sin(gc.y * 0.37 - t * 0.21) + sin((gc.x + gc.y) * 0.13 + t * 0.17)) / 3.0;
  var lvl = max(0.0, drift - 0.25) * 0.55 * step(h1(gc), 0.4) + wave(gcen, 0.16) * 0.95 + lens(gcen) * 0.7;
  lvl *= 1.0 - smoothstep(0.2, 0.7, near(gcen).r);
  let g = unit(frag.xy - gcen, gp * 0.42 * clamp(lvl, 0.0, 1.0)) * step(0.05, lvl);

  var col = mix(u.bg.rgb, u.fg.rgb, g * 0.34);
  let inkc = select(mix(mix(u.bg.rgb, u.fg.rgb, a.r), u.accent.rgb, a.g), a.rgb, is_pic);
  col = mix(col, inkc, a.a);
  if (u.fd.y > 0.5) { col = pow(col, vec3f(2.2)); }
  return vec4f(col, 1.0);
}
