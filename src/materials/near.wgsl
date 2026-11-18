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
  let line = clamp(1.0 - f / (0.6 * s + 0.2), 0.0, 1.0) * (1.0 - smoothstep(0.3, 1.0, abs(sd) / u.fd.z));
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
