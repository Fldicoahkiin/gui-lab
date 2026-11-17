// @material plain
// plain: what is held or just born is faint rather than loose; the ground is flat. The square root
// lets a partial looseness (beneath a dialog) read clearly; fully loose is 0.3.
fn m_plain(p: vec2f) -> vec3f {
  let d = dec(p);
  return mix(gnd(p), d.rgb, d.a * mix(1.0, 0.3, sqrt(clamp(loose_t(p) * loose_scale(), 0.0, 1.0))));
}
