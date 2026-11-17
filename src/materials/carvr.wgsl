// @material carvr
// @param 刻痕 0.0 1.0 0.05 2 - 0.6
// @param 刻带 2.0 40.0 1.0 0 px 12.0
// @param 刀长 2.0 40.0 1.0 0 px 12.0
// @param 刀距 2.0 16.0 0.5 1 px 7.0
// @param 抖动 0.0 1.0 0.05 2 - 0.4
// carvr: ink or ground, nothing between. Loosened, the outline is cut by hand (displaced by noise) and
// a band around it fills with gouge strokes that follow the outline: ridges of ink left standing
// outside, white cuts inside; the band widens with the looseness until shapes break into strokes.
fn m_carvr(p: vec2f) -> vec3f {
  let s = u.res.z;
  let a0 = mp(SELF, 0); let a1 = mp(SELF, 1);
  // Cut only where the ink is really loose: text held at the cap stays as it is.
  let k = kt_loose(p);
  if (k <= 0.0) { return exact(p); }
  let carve = a0.x; let band = min(a0.y * s * k, u.fd.z - 0.5); let slen = max(a0.z * s, 2.0); let spitch = max(a0.w * s, 2.0); let wob = a1.x;
  let sd = sdist(p);
  let w = ink_inward(p);
  if (w.a < 0.02 && sd < -u.fd.z + 0.6) { return gnd(p); }
  // The cut edge: a slow wander of the knife and small chips.
  let n = vnoise(lat(p) / (slen * 2.0)) - 0.5;
  let chip = vnoise(lat(p) / (slen * 0.35) + 3.0) - 0.5;
  let cut = sd + (n * 2.0 + chip * 0.8) * carve * (1.0 + 1.5 * k) * s;
  var inked = smoothstep(-0.6, 0.6, cut);
  if (abs(sd) < band && abs(sd) < u.fd.z - 0.5) {
    let g = sgrad(p);
    let nrm = g / max(length(g), 1e-3);
    let tng = vec2f(-nrm.y, nrm.x);
    let wv = wob * spitch * (vnoise(lat(p) / slen + 7.0) - 0.5);
    let along = dot(lat(p), tng) / spitch + wv / spitch;
    let lane = floor(along);
    if (sd > 0.0) {
      // Gouges: white wedges cut in from the edge, each of its own length.
      let len = band * (0.35 + 0.65 * h1(vec2f(lane, 4.0)));
      let width = 0.42 * clamp(1.0 - sd / max(len, 1e-3), 0.0, 1.0);
      let g1 = smoothstep(width + 0.06, width - 0.06, abs(fract(along) - 0.5)) * step(0.4, h1(vec2f(lane, 9.0)));
      inked = min(inked, 1.0 - g1);
    } else {
      // Ridges of ink left standing outside, echoing the edge, in long broken lines.
      let echo = -sd / (spitch * 1.6) + wv / spitch * 0.4;
      let on_line = smoothstep(0.6 * s, 0.0, abs(fract(echo + 0.5) - 0.5) * spitch * 1.6 - 0.4 * s) * step(0.6, -sd / spitch);
      let dash = step(0.35, vnoise(vec2f(dot(lat(p), tng) / (slen * 1.5), floor(echo + 0.5) * 3.1)));
      inked = max(inked, on_line * dash);
    }
  }
  let lino = mix(gnd(p), select(w.rgb, u.fg.rgb, w.a < 0.02), inked);
  return mix(exact(p), lino, smoothstep(0.0, 0.12, k));
}
