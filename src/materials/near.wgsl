// @material near
// @param 间距 1.5 12.0 0.5 1 px 3.0
// @param 填充 0.0 1.0 0.05 2 - 0.25
// @param 墨带 0.0 3.0 1.0 0 - 0.0
// @param 带宽 0.0 12.0 1.0 0 px 0.0
// near: the intermediate images made visible. The signed nearness in bands every `pitch` px, in the
// hairline grey inside the ink (the accent is for what is held) and cool outside, fading toward the edge
// of its reach; the outline in the ink colour;
// the ink itself as a faint fill; the looseness of each instance as a tint over it.
// With `墨带` (ink bands) set, 1 to 3: only that many bands outside the ink instead, in the ink at 0.5,
// 0.3 and 0.15 (two: 0.5 and 0.25), each `带宽` wide with as much ground before it (half the band pitch
// when 0), square round the ink's pixel corners: for pixel type on whole pixels and on the lattice.
fn m_near(p: vec2f) -> vec3f {
  let s = u.res.z;
  let a0 = mp(SELF, 0);
  if (a0.z >= 0.5) { return near_bands(p, a0); }
  let n = near_t(p);
  let g = gnd(p);
  let d = dec(p);
  var col = mix(g, d.rgb, d.a * a0.y);
  if (n.r <= 0.001) { return col; }
  let sd = sdist(p);
  let band = max(a0.x * s, 1.0);
  let f = abs(fract(sd / band + 0.5) - 0.5) * band;
  var line = clamp(1.0 - f / (0.6 * s + 0.2), 0.0, 1.0) * (1.0 - smoothstep(0.3, 1.0, abs(sd) / u.fd.z));
  // No bands round text at text sizes (its strokes under 3 px): only the shapes' nearness reaches
  // round a label in a control's material, so a label's bands showed there as stray dashes over its
  // ascenders and under its descenders. A title's strokes are wider, and it keeps its bands.
  if (sd < -0.5 && line > 0.0) {
    let gr = sgrad(p);
    let nn = gr / max(length(gr), 1e-3);
    // Up the gradient to the first ink (the nearness of a glyph is its atlas', so its scale is rough):
    // text that is not on a shape, with strokes thinner than 3 px.
    for (var k = 0; k < 16; k++) {
      let q = p + nn * (0.5 + f32(k));
      let i = dec(q);
      if (i.a > 0.25) {
        // Thin: 3 px on, or to either side, is out of the ink (a stem met end on is long, not wide).
        let side = vec2f(-nn.y, nn.x) * 3.0 * s;
        let out = dec(q + nn * 3.0 * s).a < 0.25 || dec(q + side).a < 0.25 || dec(q - side).a < 0.25;
        if (i.txt && !i.shape && out) { line = 0.0; }
        break;
      }
      if (f32(k) > 2.0 - sd * 2.0) { break; }
    }
  }
  let tint = select(u.second.rgb, u.greys[1].rgb, sd > 0.0);
  col = mix(col, tint, line * 0.85);
  col = mix(col, u.fg.rgb, clamp(1.0 - abs(sd) / (0.8 * s), 0.0, 1.0));
  col = mix(col, u.third.rgb, clamp(loose_t(p) + half_plane(p), 0.0, 1.0) * 0.35 * step(0.5, n.r) * (1.0 - hold_at(p)));
  // Text keeps its own ink whole, the bands around it: a glyph's outline alone is too thin to read
  // at text sizes, and on a filled panel (a selected row, a button) the panel's nearness, max-blended
  // over the glyph's, leaves it no outline at all. (Text in the ground's colour is written in the ink's
  // colour by the first pass, `ink_of`.)
  if (d.txt) { col = mix(col, d.rgb, d.a); }
  return col;
}

/// Whether any ink is in the ring of cells `r` away (the Chebyshev ring) round `cell`, `c` px cells.
fn near_ring(cell: vec2f, r: i32, c: f32) -> bool {
  for (var i = -r; i <= r; i++) {
    let a = vec2f(f32(i), f32(r)); let b = vec2f(f32(r), f32(i));
    for (var q = 0; q < 4; q++) {
      let o = select(select(a, -a, q == 1), select(b, -b, q == 3), q >= 2);
      if (q >= 2 && abs(i) == r) { continue; }
      if (dec((cell + o + 0.5) * c - anc).a > 0.5) { return true; }
    }
  }
  return false;
}
/// `near` in ink bands. The ink's edges are on the lattice of the band pitch and the bands' on whole
/// multiples of their width, so the distance is taken in cells of their common divisor: the first
/// ring of cells with ink in it.
fn near_bands(p: vec2f, a0: vec4f) -> vec3f {
  let s = u.res.z;
  let d = dec(p);
  let base = mix(gnd(p), d.rgb, d.a);
  if (d.a > 0.5) { return base; }
  let nb = i32(round(min(a0.z, 3.0)));
  let pitch = u32(max(round(a0.x * s), 1.0));
  let w = u32(max(round(select(a0.x * 0.5, a0.w, a0.w > 0.0) * s), 1.0));
  var x = pitch; var y = w;
  loop { if (y == 0u) { break; } let t = x % y; x = y; y = t; }
  let c = f32(x);
  let cell = floor(lat(p) / c);
  let reach = i32(ceil(f32(2 * nb) * f32(w) / c));
  var m = 0;
  for (var r = 1; r <= reach; r++) { if (near_ring(cell, r, c)) { m = r; break; } }
  if (m == 0) { return base; }
  // Slots of `w` from the edge: even ones ground, odd ones a band.
  let slot = i32(floor(f32(m - 1) * c / f32(w)));
  if (slot % 2 == 0 || slot / 2 >= nb) { return base; }
  let i = slot / 2;
  var al = array<f32, 3>(0.5, 0.3, 0.15);
  if (nb == 2) { al = array<f32, 3>(0.5, 0.25, 0.0); }
  if (nb == 1) { al = array<f32, 3>(0.5, 0.0, 0.0); }
  return mix(base, u.fg.rgb, al[i]);
}
