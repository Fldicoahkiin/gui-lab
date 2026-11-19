// @material near
// @param 间距 1.5 12.0 0.5 1 px 3.0
// @param 填充 0.0 1.0 0.05 2 - 0.25
// near: the intermediate images made visible. The signed nearness in bands every `pitch` px, warm
// inside the ink and cool outside, fading toward the edge of its reach; the outline in the ink colour;
// the ink itself as a faint fill; the looseness of each instance as a tint over it.
fn m_near(p: vec2f) -> vec3f {
  let s = u.res.z;
  let a0 = mp(SELF, 0);
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
  let tint = select(u.second.rgb, u.accent.rgb, sd > 0.0);
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
