// @material halftonr
// @param 点距 2.0 32.0 0.5 1 px 7.0
// @param 角度 0.0 90.0 1.0 0 ° 45.0
// @param 分色错开 0.0 8.0 0.25 2 px 1.25
// @param 增益 0.2 3.0 0.05 2 - 1.1
// halftonr: ink as round dots on a rotated screen, with the second colour out of register.
fn m_halftonr(p: vec2f) -> vec3f {
  let s = u.res.z;
  let a0 = mp(SELF, 0);
  let pitch = max(2.0, a0.x * s); let ang = radians(a0.y); let split = a0.z * s; let gain = a0.w;
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
  // Three screens, one per channel, at their own angles and slightly out of register.
  var out = ex;
  let angs = vec3f(radians(15.0), radians(75.0), 0.0) + vec3f(ang - radians(45.0));
  for (var j = 0; j < 3; j++) {
    let off = vec2f(cos(f32(j) * 2.094), sin(f32(j) * 2.094)) * split;
    let q = rot(lat(p), angs[j]);
    let cell = floor(q / pitch);
    let c = rot((cell + 0.5) * pitch, -angs[j]) - anc;
    if (kt_loose(c) <= 0.12 + 0.5 * h1(cell + vec2f(angs[j] * 7.0, 3.0))) { continue; }
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
