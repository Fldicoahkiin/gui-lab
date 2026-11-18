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
  // Within `r0` of text to read only the still frame's ink; and nothing is read from where text to read
  // holds (a slipping row, a repeated block or a split channel never carries a copy of its glyphs).
  let core = core_at(p);
  if (core >= 1.0) { return here; }
  let tq = floor(u.fa.y * 1000.0 / hold);
  var q = p;
  // Up to half loose, rows slip and blocks repeat at half their size; past it, they grow to their full size.
  let grow = 0.5 + 0.5 * clamp((kr - 0.5) * 2.0, 0.0, 1.0);
  if (h1(vec2f(row, tq)) < kr * 0.7) { q.x += (h1(vec2f(row * 1.7, tq + 3.0)) - 0.5) * 2.0 * shift * kr * grow; }
  let bc = floor(lat(p) / block);
  // A block repeats only what is there: ink or a picture where it is read from, never bare ground.
  let bq = vec2f(q.x - block * grow * (1.0 + floor(h1(bc + 9.0) * 2.0)), q.y);
  let has = ink_t(bq).a > 0.02 || near_t(bq).g > 0.05;
  if (has && hold_at(bq) <= 0.0 && h1(bc + vec2f(tq * 0.37, 1.0)) < kr * 0.16) { q = bq; }
  if (hold_at(q) > 0.0) { q = p; }
  let sp = vec2f(split * kr, 0.0);
  // A picture keeps to itself: a sample that would leave it stays where it is.
  var qr = q + sp; var qg = q; var qb = q - sp;
  if (hold_at(qr) > 0.0) { qr = p; }
  if (hold_at(qg) > 0.0) { qg = p; }
  if (hold_at(qb) > 0.0) { qb = p; }
  if (pic) {
    if (near_t(qr).g <= 0.05) { qr = p; }
    if (near_t(qg).g <= 0.05) { qg = p; }
    if (near_t(qb).g <= 0.05) { qb = p; }
  }
  var c = vec3f(scene_at(qr).r, scene_at(qg).g, scene_at(qb).b);
  // A row's flash only where something is drawn: no grey bars over bare ground.
  let drawn = ink_t(q).a > 0.02 || near_t(q).g > 0.05 || ink_t(p).a > 0.02 || near_t(p).g > 0.05;
  if (drawn && h1(vec2f(row, tq + 11.0)) < kr * 0.12) { c = c * 0.75 + 0.25 * (1.0 - c) * step(0.5, h1(vec2f(row, tq + 2.0))); }
  return mix(c, here, core);
}
