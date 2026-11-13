// Two stages.
//   Stage 1 draws every instance exactly, into two intermediate images:
//     ink  (rgba8): what is there. rgb = tone or picture colour, a = coverage.
//     near (rg8):   r = how close this pixel is to the nearest ink, by true glyph shape (max-blended);
//                   g = 1 where a picture is.
//   Stage 2 (the material) reads both plus the disturbance field and decides the final pixel.

struct U {
  res: vec4f,      // width, height, scale, time
  params: vec4f,   // text pitch, picture pitch, glitch, jitter   (pitches in logical px)
  mode: vec4f,     // unit shape (0 dot, 1 square), srgb surface, spread px, _
  bg: vec4f,
  fg: vec4f,
  pointer: vec4f,  // x, y, strength, _
  ripples: array<vec4f, 8>, // x, y, start time, strength
};
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var tex0: texture_2d<f32>;
@group(0) @binding(3) var tex1: texture_2d<f32>;

const SPEED: f32 = 760.0;

fn ripple_at(p: vec2f, r: vec4f) -> vec2f { // (front profile, signed distance to front)
  let age = u.res.w - r.z;
  if (r.w <= 0.0 || age < 0.0) { return vec2f(0.0, 1e6); }
  let d = distance(p, r.xy) - age * SPEED * u.res.z;
  return vec2f(exp(-age * 0.75) * r.w, d);
}

// ── Stage 1 ───────────────────────────────────────────────────────────────────────────────────────
struct VIn { @builtin(vertex_index) vi: u32, @location(0) rect: vec4f, @location(1) uv: vec4f, @location(2) tone: vec4f };
struct VOut { @builtin(position) pos: vec4f, @location(0) uv: vec2f, @location(1) tone: vec4f, @location(2) local: vec2f, @location(3) size: vec2f };

@vertex fn vs_inst(i: VIn) -> VOut {
  let c = vec2f(f32(i.vi & 1u), f32((i.vi >> 1u) & 1u));
  var p = i.rect.xy + c * i.rect.zw;
  // Each instance rides the wave: it is pushed outward a little as a front passes under it.
  let centre = i.rect.xy + i.rect.zw * 0.5;
  for (var k = 0; k < 8; k++) {
    let r = ripple_at(centre, u.ripples[k]);
    let w = 34.0 * u.res.z;
    let dir = normalize(centre - u.ripples[k].xy + vec2f(1e-4));
    p += dir * exp(-(r.y * r.y) / (w * w)) * r.x * 5.0 * u.res.z;
  }
  var o: VOut;
  o.pos = vec4f(p.x / u.res.x * 2.0 - 1.0, 1.0 - p.y / u.res.y * 2.0, 0.0, 1.0);
  o.uv = mix(i.uv.xy, i.uv.zw, c);
  o.tone = i.tone; o.local = c * i.rect.zw; o.size = i.rect.zw;
  return o;
}
@fragment fn fs_glyph_ink(i: VOut) -> @location(0) vec4f {
  let a = textureSampleLevel(tex0, samp, i.uv, 0.0).r;
  return vec4f(i.tone.rgb * a, a);
}
@fragment fn fs_glyph_near(i: VOut) -> @location(0) vec4f {
  return vec4f(textureSampleLevel(tex0, samp, i.uv, 0.0).g, 0.0, 0.0, 0.0);
}
// Pictures: the instance rect is padded by the spread; `tone.a` carries nothing, uv spans the padded rect.
fn pic_d(i: VOut) -> f32 {
  let s = u.mode.z;
  let q = abs(i.local - i.size * 0.5) - (i.size * 0.5 - vec2f(s));
  return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0);
}
@fragment fn fs_pic_ink(i: VOut) -> @location(0) vec4f {
  if (pic_d(i) > 0.0) { discard; }
  return vec4f(textureSampleLevel(tex0, samp, i.uv, 0.0).rgb, 1.0);
}
@fragment fn fs_pic_near(i: VOut) -> @location(0) vec4f {
  let d = pic_d(i);
  return vec4f(clamp(1.0 - d / u.mode.z, 0.0, 1.0), select(0.0, 1.0, d <= 0.0), 0.0, 0.0);
}

// ── Stage 2: material ─────────────────────────────────────────────────────────────────────────────
@vertex fn vs_full(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
  return vec4f(f32((vi << 1u) & 2u) * 2.0 - 1.0, f32(vi & 2u) * 2.0 - 1.0, 0.0, 1.0);
}
fn h1(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn h2(p: vec2f) -> vec2f { return vec2f(h1(p), h1(p + vec2f(19.19, 73.3))); }

/// How disturbed a point is by passing fronts: 1 on a front, a soft tail behind it.
fn wave(p: vec2f) -> f32 { return wave2(p, 0.6); }
fn wave2(p: vec2f, tail_gain: f32) -> f32 {
  var k = 0.0;
  let s = u.res.z;
  for (var j = 0; j < 8; j++) {
    let r = ripple_at(p, u.ripples[j]);
    let front = exp(-(r.y * r.y) / (26.0 * 26.0 * s * s));
    let tail = select(0.0, exp(r.y / (150.0 * s)) * tail_gain, r.y < 0.0);
    k = max(k, max(front, tail) * r.x);
  }
  return k;
}
fn lens(p: vec2f) -> f32 {
  let d = (p - u.pointer.xy) / (84.0 * u.res.z);
  return exp(-dot(d, d)) * u.pointer.z;
}
fn ink(p: vec2f) -> vec4f { return textureSampleLevel(tex0, samp, p / u.res.xy, 0.0); }
fn near(p: vec2f) -> vec4f { return textureSampleLevel(tex1, samp, p / u.res.xy, 0.0); }
fn unit(d: vec2f, r: f32) -> f32 {
  if (u.mode.x < 0.5) { return smoothstep(r + 0.75, r - 0.75, length(d)); }
  return smoothstep(r + 0.5, r - 0.5, max(abs(d.x), abs(d.y)));
}

@fragment fn fs_material(@builtin(position) frag: vec4f) -> @location(0) vec4f {
  let s = u.res.z; let t = u.res.w;
  var p = frag.xy;

  // Glitch: while a front is passing, some thin horizontal bands slip sideways.
  let bh = 5.0 * s; let band = floor(p.y / bh);
  let w0 = wave(vec2f(p.x, (band + 0.5) * bh));
  let pick = step(0.55, h1(vec2f(band, floor(t * 9.0))));
  p.x += (h1(vec2f(band, floor(t * 17.0))) - 0.5) * 70.0 * s * u.params.z * pick * smoothstep(0.2, 0.9, w0);

  let n = near(p);
  let is_pic = n.g > 0.5;

  // The lattice this point belongs to. Its pitch is a parameter; jitter moves each unit off the grid.
  let pitch = max(1.5, select(u.params.x, u.params.y, is_pic) * s);
  let cell = floor(p / pitch);
  let centre = (cell + 0.5) * pitch + (h2(cell) - 0.5) * u.params.w * pitch;
  // Fronts loosen everything; the pointer loosens only pictures, so reading is never disturbed by it.
  let loose = wave(centre) + select(0.0, lens(centre), is_pic) + (h1(cell + 7.0) - 0.5) * 0.22;

  var a: vec4f;
  if (loose > 0.3) {
    // Loosened: this lattice unit shows the average ink under it as one dot or square.
    let q = pitch * 0.25;
    let c = (ink(centre + vec2f(-q, -q)) + ink(centre + vec2f(q, -q)) + ink(centre + vec2f(-q, q)) + ink(centre + vec2f(q, q))) * 0.25;
    let grow = smoothstep(0.3, 0.42, loose);
    let r = pitch * mix(0.5, 0.44 * sqrt(c.a), grow) * select(1.0, 0.96, u.mode.x > 0.5);
    let m = unit(p - centre, r) * step(0.06, c.a);
    a = vec4f(c.rgb / max(c.a, 1e-3), m * mix(c.a, 1.0, grow));
  } else {
    let c = ink(p);
    a = vec4f(c.rgb / max(c.a, 1e-3), c.a);
  }

  // Ground: a field of small units. They follow the fronts and the pointer, drift slowly at rest, and
  // shrink away from ink by its true outline (the nearness image), never by a rectangle.
  let gp = 9.0 * s; let gc = floor(frag.xy / gp); let gcen = (gc + 0.5) * gp;
  let drift = (sin(gc.x * 0.21 + t * 0.33) + sin(gc.y * 0.37 - t * 0.21) + sin((gc.x + gc.y) * 0.13 + t * 0.17)) / 3.0;
  var lvl = max(0.0, drift - 0.25) * 0.55 * step(h1(gc), 0.4) + wave2(gcen, 0.16) * 0.95 + lens(gcen) * 0.7;
  lvl *= 1.0 - smoothstep(0.2, 0.7, near(gcen).r);
  let g = unit(frag.xy - gcen, gp * 0.42 * clamp(lvl, 0.0, 1.0)) * step(0.05, lvl);

  var col = mix(u.bg.rgb, u.fg.rgb, g * 0.34);
  let inkc = select(mix(u.bg.rgb, u.fg.rgb, a.r), a.rgb, is_pic);
  col = mix(col, inkc, a.a);
  if (u.mode.y > 0.5) { col = pow(col, vec3f(2.2)); }
  return vec4f(col, 1.0);
}
