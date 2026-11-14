// The WebGPU tier of the particle simulation (appended to particles.wgsl): the same `integrate`, as a
// compute shader writing the next state texture.
@group(0) @binding(6) var next: texture_storage_2d<rgba32float, write>;
@compute @workgroup_size(8, 8) fn cs_sim(@builtin(global_invocation_id) g: vec3u) {
  let w = u32(pu.gd.z);
  if (g.x >= w || g.y >= textureDimensions(next).y) { return; }
  let i = g.y * w + g.x;
  textureStore(next, vec2i(g.xy), integrate(i, textureLoad(state, vec2i(g.xy), 0)));
}
