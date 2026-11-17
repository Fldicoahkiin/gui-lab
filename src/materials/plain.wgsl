// @material plain
// plain: what is held or just born is faint rather than loose; the ground is flat. The square root
// lets a partial looseness (beneath a dialog) read clearly; fully loose is 0.3. Up to the legibility
// cap (a press's flash on a control) it keeps what the theme allows (`Theme::press_keep`), so a label
// on its fill never falls under 4.5:1; from there on to 0.3.
fn m_plain(p: vec2f) -> vec3f {
  let d = dec(p);
  let k = clamp(loose_t(p) * loose_scale(), 0.0, 1.0);
  let cap = clamp(u.burst_t.w, 0.0, 0.99);
  let keep = select(u.scrq.z, 1.0 - 0.7 * sqrt(cap), u.scrq.z <= 0.0);
  let f = select(mix(keep, 0.3, clamp((sqrt(k) - sqrt(cap)) / (1.0 - sqrt(cap)), 0.0, 1.0)), mix(1.0, keep, k / max(cap, 1e-3)), k <= cap);
  return clamp(mix(gnd(p), d.rgb, d.a * f), vec3f(0.0), vec3f(1.0));
}
