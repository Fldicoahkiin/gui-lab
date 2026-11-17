// @material charr
// @param 点距 4.0 40.0 1.0 0 px 10.0
// @param 级数 2.0 16.0 1.0 0 - 12.0
// @param 上色方式 0.0 1.0 1.0 0 - 0.0
// @param 法线 0.0 1.0 0.05 2 - 1.0
// charr: cells of the print pitch show characters from the character mark set, chosen by the ink
// they cover; the more loose, the coarser the cells.
fn m_charr(p: vec2f) -> vec3f {
  let s = u.res.z;
  let a0 = mp(SELF, 0);
  let P0 = clamp(round(a0.x * s), CHAR_MIN, CHAR_MAX);
  let coarse = floor(lat(p) / P0);
  let kc = kt_loose((coarse + 0.5) * P0 - anc);
  if (kc <= 0.0 && kt_loose(p) <= 0.0) { return exact(p); }
  // One cell size for the whole element at a time, by its own looseness, and a clean jump between
  // them: half the pitch first, the pitch from 0.3, one and a half times it from 0.7; from there the
  // levels fall to half, so the characters go on getting coarser to the end.
  let kl = clamp((loose_t(p) + u.rip.y) * loose_scale(), 0.0, u.rip.x);
  let levels = clamp(round(mix(round(a0.y), round(a0.y) * 0.5, smoothstep(0.7, 1.0, kl))), 2.0, 16.0);
  let P = select(select(P0, min(CHAR_MAX, round(P0 * 1.5)), kl >= 0.7), max(CHAR_MIN, round(P0 * 0.5)), kl < 0.3);
  let cell = floor(lat(p) / P);
  // Characters only where the ink is really loose: text held at the cap stays as it is.
  if (kt_loose((cell + 0.5) * P - anc) <= 0.1 + 0.4 * h1(cell + 1.3)) { return exact(p); }
  let centre = (cell + 0.5) * P - anc;
  // A cell over a picture's edge is not made into characters: the picture keeps its own edge.
  let o0 = cell * P - anc;
  for (var j = 0; j < 3; j++) { for (var i = 0; i < 3; i++) {
    if (near_t(o0 + (vec2f(f32(i), f32(j)) * 0.5) * P).g > 0.05) { return exact(p); }
  } }
  let avg = cell_avg(cell, P);
  let d = dec_at(avg, centre);
  var n = clamp(floor(avg.a * levels), 0.0, levels - 1.0);
  // A thin accent element (a rule) under a cell larger than it still shows as a row of characters in
  // the accent, so the rule never breaks up: at least a middle character wherever it lies.
  var acc_ink = ink_t(centre);
  for (var j = -3; j <= 3; j++) { let t = ink_t(centre + vec2f(0.0, f32(j) * P * 0.15)); if (t.a > acc_ink.a) { acc_ink = t; } }
  let rule = dec_at(acc_ink, centre).acc > 0.5 && acc_ink.a > 0.3;
  if (rule) { n = max(n, floor(levels * 0.55)); }
  var m = -1.0;
  // On a 3D element (DESIGN.md §1.18): density by the light on it, and on its edges and slanted faces a
  // stroke (| / - \) along the edge or across the slope, by the projected normal, as often as
  // `ascii_normal_weight` says.
  let code = round(near_t(centre).g * 255.0);
  if (code >= 1.0 && code <= 9.0) {
    n = clamp(floor(clamp(d.tone + d.acc, 0.0, 1.0) * avg.a * levels), 0.0, levels - 1.0);
    let wgt = clamp(a0.w, 0.0, 1.0);
    let sd = sdist(centre);
    var ang = -1.0;
    if (abs(sd) < P * 0.75) {
      // On an edge: along it (across the nearness's gradient).
      let g = sgrad(centre);
      if (length(g) > 1e-3) { ang = atan2(g.y, g.x) + 1.5707963; }
    } else if (code >= 2.0 && sd > 0.0) {
      // A face turned away: across its slope (the normal's direction turned a quarter).
      ang = (code - 2.0 + 0.5) / 8.0 * 3.14159265 + 1.5707963;
    }
    if (ang > -0.5 && h1(cell + 5.1) < wgt) {
      // The nearest of the four families, drawn as a line through the cell one stroke wide.
      let fam = round(fract(ang / 3.14159265) * 4.0) % 4.0;
      let dir = vec2f(cos(fam * 0.7853982), sin(fam * 0.7853982));
      let q = fract(lat(p) / P) - 0.5;
      let dist = abs(q.x * dir.y - q.y * dir.x) * P;
      m = clamp(max(1.0, P * 0.14) - dist + 0.5, 0.0, 1.0) * step(abs(dot(q, dir)), 0.42) * step(0.02, avg.a);
    }
  }
  if (m < 0.0) { m = char_mark(round(n * 15.0 / (levels - 1.0)), fract(lat(p) / P), P); }
  var colour = select(d.rgb, u.fg.rgb, a0.z >= 0.5);
  if (rule) { colour = dec_at(acc_ink, centre).rgb; }
  return mix(gnd(p), colour, m);
}
