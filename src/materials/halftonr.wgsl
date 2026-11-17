// @material halftonr
// @param 点距 2.0 32.0 0.5 1 px 7.0
// @param 角度 0.0 90.0 1.0 0 ° 45.0
// @param 分色错开 0.0 8.0 0.25 2 px 1.25
// @param 增益 0.2 3.0 0.05 2 - 1.1
// halftonr: ink as round dots on a rotated screen, with the second colour out of register.
fn m_halftonr(p: vec2f) -> vec3f {
  let s = u.res.z;
  let a0 = mp(SELF, 0);
  // Past half loose the screen goes on opening (to 1.5 times the pitch) and the separations part twice
  // as far (the element's own looseness, so one element keeps one screen).
  let on = smoothstep(0.5, 1.0, clamp((loose_t(p) + u.rip.y) * loose_scale(), 0.0, u.rip.x));
  let pitch = max(2.0, a0.x * s) * mix(1.0, 1.5, on); let ang = radians(a0.y); let split = a0.z * s * (1.0 + on); let gain = a0.w;
  // Dots only where the ink is really loose: text held at the cap stays as it is.
  if (kt_loose(p) <= 0.0 && kt_loose(p + vec2f(pitch)) <= 0.0 && kt_loose(p - vec2f(pitch)) <= 0.0) { return exact(p); }
  let g = gnd(p);
  let ex = exact(p);
  if (split <= 0.0) {
    let q = rot(lat(p), ang);
    let cell = floor(q / pitch);
    let c = rot((cell + 0.5) * pitch, -ang) - anc;
    if (kt_loose(c) <= 0.12 + 0.5 * h1(cell + vec2f(ang * 7.0, 3.0))) { return ex; }
    let e = pitch * 0.25;
    let avg = (ink_t(c + vec2f(-e, -e)) + ink_t(c + vec2f(e, -e)) + ink_t(c + vec2f(-e, e)) + ink_t(c + vec2f(e, e))) * 0.25;
    let r = pitch * 0.56 * sqrt(clamp(avg.a * gain, 0.0, 1.0));
    let m = smoothstep(r + 0.6, r - 0.6, length(q - (cell + 0.5) * pitch)) * step(0.02, avg.a);
    return mix(g, dec_at(avg, c).rgb, m);
  }
  // Accent ink (a rule in the accent) is one screen in its own colour, not separated.
  let here = dec(p);
  if (here.acc > 0.5 || dec(p + vec2f(pitch * 0.5)).acc > 0.5) {
    let q = rot(lat(p), ang);
    let cell = floor(q / pitch);
    let c = rot((cell + 0.5) * pitch, -ang) - anc;
    if (kt_loose(c) <= 0.12 + 0.5 * h1(cell + vec2f(ang * 7.0, 3.0))) { return ex; }
    // A thin rule falls between the taps of a coarse screen: the dot takes the most ink near it.
    let e = pitch * 0.3;
    var best = ink_t(c);
    for (var i = 0; i < 4; i++) {
      let t = ink_t(c + vec2f(select(-e, e, i % 2 == 1), select(-e, e, i >= 2)));
      if (t.a > best.a) { best = t; }
    }
    let r = pitch * 0.5 * sqrt(clamp(best.a * gain, 0.0, 1.0));
    let m = smoothstep(r + 0.6, r - 0.6, length(q - (cell + 0.5) * pitch)) * step(0.02, best.a);
    return mix(g, dec_at(best, c).rgb, m);
  }
  // Three screens, one per separation, at their own angles and slightly out of register. Each prints
  // its separation's colour drawn halfway to the ink's own, so the page keeps to its ink and its
  // accent instead of pure red, green and blue.
  // On a light ground (ink darker than it) the separations are cyan and magenta drawn a third of the
  // way to the ink, and the third is the ink itself: no yellow, which only goes olive toward black.
  let light = dot(u.fg.rgb, vec3f(0.3, 0.59, 0.11)) < dot(g, vec3f(0.3, 0.59, 0.11));
  var out = ex;
  let angs = vec3f(radians(15.0), radians(75.0), 0.0) + vec3f(ang - radians(45.0));
  for (var j = 0; j < 3; j++) {
    let off = vec2f(cos(f32(j) * 2.094), sin(f32(j) * 2.094)) * split;
    let q = rot(lat(p), angs[j]);
    let cell = floor(q / pitch);
    let c = rot((cell + 0.5) * pitch, -angs[j]) - anc;
    if (kt_loose(c) <= 0.12 + 0.5 * h1(cell + vec2f(angs[j] * 7.0, 3.0))) { continue; }
    // A cell on accent ink is left to the accent's own screen.
    if (dec(c).acc > 0.5 || dec(c + off).acc > 0.5) { continue; }
    let e = pitch * 0.25;
    let avg = (ink_t(c + off + vec2f(-e, -e)) + ink_t(c + off + vec2f(e, -e)) + ink_t(c + off + vec2f(-e, e)) + ink_t(c + off + vec2f(e, e))) * 0.25;
    let d = dec_at(avg, c);
    // Ink that is partly the accent's (a rule's edge) is the accent screen's too.
    if (d.acc > 0.3) { continue; }
    // How much of this channel the ink changes from the ground decides the dot.
    let amt = clamp(abs(d.rgb[j] - g[j]) / max(abs(u.fg.rgb[j] - g[j]), 0.25), 0.0, 1.0) * avg.a;
    let r = pitch * 0.56 * sqrt(clamp(amt * gain, 0.0, 1.0));
    let m = smoothstep(r + 0.6, r - 0.6, length(q - (cell + 0.5) * pitch)) * step(0.02, amt);
    var sep = g; sep[j] = d.rgb[j];
    var col = mix(sep, d.rgb, 0.5);
    if (light) { col = select(mix(sep, d.rgb, 0.35), d.rgb, j == 2); }
    out = mix(out, col, m);
  }
  return out;
}
