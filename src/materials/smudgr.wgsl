// @material smudgr
// @param 纸纹 0.0 1.0 0.05 2 - 0.6
// @param 套印错位 0.0 6.0 0.25 2 px 1.5
// @param 斑点 0.0 1.0 0.05 2 - 0.6
// @param 洇开 0.0 10.0 0.5 1 px 3.0
// smudgr: paper grain on the ground and accent ink out of register at rest; loosened, photocopy speckle
// and ink spreading past its outline.
fn m_smudgr(p: vec2f) -> vec3f {
  let s = u.res.z;
  let a0 = mp(SELF, 0);
  let grain = a0.x; let mis = a0.y * s; let speckle = a0.z; let spread = a0.w * s;
  // Speckle, drop-outs and spreading only where the ink is really loose, not on text held at the cap.
  let k = kt_loose(p);
  let pa = lat(p);
  var g = gnd(p) * (1.0 - grain * (0.035 * (vnoise(pa * 0.45 / s) - 0.5) + 0.02 * (h1(floor(pa)) - 0.5)));
  let paper = lum(g) > 0.5;
  // Key (tone) ink from here, accent ink from slightly off register.
  var dk = dec(p);
  var cov_k = dk.a * (1.0 - dk.acc);
  let off = vec2f(mis, mis * 0.6);
  var da = dec(p - off);
  // Text keeps its register: where a glyph is (here or where the accent plate is read from), the
  // accent comes from here, so a label knocked out of an accent fill is not doubled or shifted.
  if (dk.txt || da.txt) { da = dk; }
  var cov_a = da.a * da.acc;
  // Accent ink is the hue itself.
  var acc_rgb = hue_col(da.hue);
  if (k > 0.0) {
    // Ink spreads out past its outline, raggedly, and thresholds like a photocopy.
    let n = vnoise(pa * 0.18 / s);
    let sd = sdist(p);
    let grow = spread * k * (0.5 + n);
    if (dk.a < 0.5 && sd + grow > 0.0) {
      let w = ink_inward(p);
      let c = clamp(sd + grow + 0.5, 0.0, 1.0);
      cov_k = max(cov_k, c * (1.0 - w.acc));
      if (w.acc > 0.5) { cov_a = max(cov_a, c); acc_rgb = w.rgb; }
      dk = w;
    }
    let th = 0.5 + (vnoise(pa * 0.6 / s) - 0.5) * 0.7 * k;
    cov_k = mix(cov_k, step(th, cov_k), smoothstep(0.0, 0.5, k));
    cov_a = mix(cov_a, step(th, cov_a), smoothstep(0.0, 0.5, k));
    // Toner specks on the paper and drop-outs in the ink.
    let cs = floor(pa / max(1.0, round(1.4 * s)));
    let hh = h1(cs);
    if (hh > 1.0 - speckle * k * 0.07 * (0.4 + vnoise(pa * 0.05 / s))) { cov_k = 1.0; dk.tone = 1.0; }
    if (h1(cs + 5.1) < speckle * k * 0.22) { cov_k *= 0.15; cov_a *= 0.15; }
  }
  // The key plate's tone (the ink stores it without the accent part).
  let key_tone = clamp(dk.tone / max(1.0 - dk.acc, 1e-3), 0.0, 1.0);
  var col = mix(g, mix(g, u.fg.rgb, key_tone), cov_k);
  if (paper) { col = col * mix(vec3f(1.0), acc_rgb / max(g, vec3f(0.05)), cov_a); }
  else { col = mix(col, acc_rgb, cov_a); }
  return clamp(col, vec3f(0.0), vec3f(1.0));
}
