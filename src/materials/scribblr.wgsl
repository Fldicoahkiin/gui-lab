// @material scribblr
// @param 线宽 0.5 3.0 0.25 2 px 1.0
// @param 取样 1.0 6.0 0.5 1 px 2.0
// @param 游走 0.0 64.0 1.0 0 px 40.0
// @param 打结 0.0 1.0 0.05 2 - 0.6
// @param 结的半径 4.0 120.0 1.0 0 px 48.0
// @param 线头 0.0 1200.0 10.0 0 px 480.0
// @param 跟场走 0.0 2.0 0.05 2 - 1.0
// @param 折角 0.0 1.0 0.05 2 - 0.9
// scribblr: a flat ground and the ink exactly as stage 1 left it. The fill's fade and the thread that
// leaves the outline are worked out on the CPU per element (strand.rs, DESIGN.md §1.30) and drawn into
// stage 1, so a line over a loose glyph is never faded with it; at looseness 0 this is plain's frame.
fn m_scribblr(p: vec2f) -> vec3f { return exact(p); }
