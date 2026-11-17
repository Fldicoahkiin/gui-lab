// @material glitchr
// @param 行高 1.0 40.0 1.0 0 px 6.0
// @param 位移 0.0 200.0 2.0 0 px 48.0
// @param 色差 0.0 30.0 0.5 1 px 6.0
// @param 保持 16.0 500.0 2.0 0 ms 90.0
// @param 块 4.0 120.0 2.0 0 px 28.0
// glitchr: nothing at rest. Loosened, rows slip sideways (held for `hold` ms), the channels split and
// blocks repeat; bursts (hover, press) do the same for a moment.
fn m_glitchr(p: vec2f, pic: bool) -> vec3f {
  let s = u.res.z;
  let a0 = mp(SELF, 0); let a1 = mp(SELF, 1);
  let slice = max(a0.x * s, 1.0); let shift = a0.y * s; let split = a0.z * s; let hold = max(a0.w, 16.0); let block = max(a1.x * s, 2.0);
  let row = floor(lat(p).y / slice);
  let rc = vec2f(p.x, (row + 0.5) * slice - anc.y);
  // Rows slip only where the ink is really loose (or a burst is on it): text held at the cap stays.
  var kr = clamp(kt_loose(rc) + burst_k(rc), 0.0, 1.0);
  // On the ground (no outline near), the field map's energy is the looseness the rows slip by.
  if (fmap_on() && near_t(p).r < 0.02 && !calm_at(rc)) { kr = max(kr, clamp(fmap_e(rc), 0.0, 1.0)); }
  let here = select(exact(p), scene_at(p), pic);
  if (kr <= 0.0) { return here; }
  let tq = floor(u.fa.y * 1000.0 / hold);
  var q = p;
  if (h1(vec2f(row, tq)) < kr * 0.7) { q.x += (h1(vec2f(row * 1.7, tq + 3.0)) - 0.5) * 2.0 * shift * kr; }
  let bc = floor(lat(p) / block);
  if (h1(bc + vec2f(tq * 0.37, 1.0)) < kr * 0.16) { q.x -= block * (1.0 + floor(h1(bc + 9.0) * 2.0)); }
  let sp = vec2f(split * kr, 0.0);
  // A picture keeps to itself: a sample that would leave it stays where it is.
  var qr = q + sp; var qg = q; var qb = q - sp;
  if (pic) {
    if (near_t(qr).g <= 0.05) { qr = p; }
    if (near_t(qg).g <= 0.05) { qg = p; }
    if (near_t(qb).g <= 0.05) { qb = p; }
  }
  var c = vec3f(scene_at(qr).r, scene_at(qg).g, scene_at(qb).b);
  if (h1(vec2f(row, tq + 11.0)) < kr * 0.12) { c = c * 0.75 + 0.25 * (1.0 - c) * step(0.5, h1(vec2f(row, tq + 2.0))); }
  return c;
}
