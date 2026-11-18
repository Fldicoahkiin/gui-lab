//! Sample pictures computed in code (no image files): a lit sphere, a sky, terrain, a lone tree and its
//! cut-out, dusk, a planet and a flower. `offscreen::Offscreen` registers them all under these ids, so
//! recipes, tests and tools can show a picture without shipping one.
pub const SPHERE: u32 = 0;
pub const SKY: u32 = 1;
pub const TERRAIN: u32 = 2;
/// A lone windswept tree on a straw rise: the test picture for filters, dithers and text wrap.
pub const TREE: u32 = 3;
/// The tree with a slip of its ground, cut out: transparent around it and in the crown's holes, so it
/// has a true outline with an inside.
pub const CUTOUT: u32 = 4;
/// Saturated, high-contrast subjects with deep darks and bright rims, which is what marks read best:
/// a cloudscape crossed by a streak of light, a planet against the dark, a flower.
pub const DUSK: u32 = 5;
pub const PLANET: u32 = 6;
pub const FLOWER: u32 = 7;
pub const PICTURE_NAMES: [&str; 4] = ["球", "天空", "地形", "孤树"];
pub const PICTURE_SIZES: [(u32, u32); 8] = [(640, 320), (640, 400), (640, 400), (640, 400), (520, 400), (640, 400), (640, 400), (480, 480)];

/// Every demo picture: id, width, height, RGBA8.
pub fn pictures() -> Vec<(u32, u32, u32, Vec<u8>)> {
    let f: [fn(u32, u32) -> Vec<u8>; 8] = [picture_pixels, sky_pixels, terrain_pixels, tree_pixels, cutout_pixels, dusk_pixels, planet_pixels, flower_pixels];
    f.iter().enumerate().map(|(i, f)| { let (w, h) = PICTURE_SIZES[i]; (i as u32, w, h, f(w, h)) }).collect()
}

fn rgba(w: u32, h: u32, f: impl Fn(f32, f32) -> [f32; 3]) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h { for x in 0..w {
        let c = f(x as f32 + 0.5, y as f32 + 0.5);
        let o = ((y * w + x) * 4) as usize;
        for k in 0..3 { px[o + k] = (c[k].clamp(0.0, 1.0) * 255.0).round() as u8; }
        px[o + 3] = 255;
    } }
    px
}

/// A lit sphere over a horizon. RGBA8, `w` × `h`.
pub fn picture_pixels(w: u32, h: u32) -> Vec<u8> {
    let (cx, cy, r) = (w as f32 * 0.6, h as f32 * 0.46, h as f32 * 0.33);
    let light = norm([-0.5, -0.62, 0.6]);
    rgba(w, h, |fx, fy| {
        let v = fy / h as f32;
        let horizon = 0.64;
        let mut c = if v < horizon { let k = v / horizon; [0.05 + 0.13 * k, 0.07 + 0.17 * k, 0.09 + 0.2 * k] }
                    else { let k = (v - horizon) / (1.0 - horizon); let glow = (-((fx - cx) / (w as f32 * 0.3)).powi(2)).exp() * (1.0 - k) * 0.4; [0.08 + glow * 0.8, 0.1 + glow * 0.9, 0.12 + glow * 0.9] };
        let (dx, dy) = ((fx - cx) / r, (fy - cy) / r);
        let d2 = dx * dx + dy * dy;
        if d2 < 1.0 {
            let dz = (1.0 - d2).sqrt();
            let lam = (dx * light[0] + dy * light[1] + dz * light[2]).max(0.0);
            let spec = lam.powf(24.0) * 0.5;
            let k = 0.07 + lam * 0.85;
            c = [(0.62 * k + spec).min(1.0), (0.76 * k + spec).min(1.0), (0.78 * k + spec).min(1.0)];
        }
        if (fy - h as f32 * 0.2).abs() < 1.5 && fx > w as f32 * 0.07 && fx < w as f32 * 0.28 { c = [1.0, 0.42, 0.17]; }
        c
    })
}

/// Evening sky: a gradient, a low sun with its glow, layered clouds lit from below, and ridges that
/// fade into the haze with distance.
pub fn sky_pixels(w: u32, h: u32) -> Vec<u8> {
    let (sw, sh) = (w as f32, h as f32);
    let sun = (sw * 0.68, sh * 0.6);
    rgba(w, h, |x, y| {
        let v = y / sh;
        let top = [0.10, 0.14, 0.32];
        let mid = [0.55, 0.38, 0.52];
        let low = [1.0, 0.62, 0.32];
        let mut c = if v < 0.45 { mix3(top, mid, v / 0.45) } else { mix3(mid, low, ((v - 0.45) / 0.25).min(1.0)) };
        let d = ((x - sun.0).powi(2) + (y - sun.1).powi(2)).sqrt() / sh;
        let glow = (-d * 5.0).exp() * 0.8;
        c = add3(c, scale3([1.0, 0.75, 0.45], glow));
        if d < 0.055 { c = mix3(c, [1.0, 0.95, 0.8], smooth(0.055, 0.045, d)); }
        // Clouds: two layers of drifting noise, brighter on their sunward underside.
        for (layer, (yc, thick, sc, dens)) in [(0.22f32, 0.16f32, 0.006f32, 0.55f32), (0.42, 0.10, 0.011, 0.6)].into_iter().enumerate() {
            let band = (-((v - yc) / thick).powi(2)).exp();
            let n = fbm(x * sc * 1.0 + layer as f32 * 31.0, y * sc * 2.6 + layer as f32 * 7.0, 5);
            let a = smooth(dens, dens + 0.18, n * band + 0.2 * band);
            let lit = fbm(x * sc + 3.0 + layer as f32 * 31.0, (y + 6.0) * sc * 2.6 + layer as f32 * 7.0, 5);
            let under = mix3([0.30, 0.22, 0.34], [1.0, 0.68, 0.48], (lit - n + 0.5).clamp(0.0, 1.0) * (1.0 - v * 0.4) + glow * 0.4);
            c = mix3(c, under, a * 0.92);
        }
        // Ridges, far to near: each is a 1-D noise skyline; distance mixes it toward the haze.
        for (i, (base, amp, haze)) in [(0.62f32, 0.10f32, 0.55f32), (0.70, 0.12, 0.28), (0.80, 0.14, 0.0)].into_iter().enumerate() {
            let line = base - amp * fbm(x * 0.004 * (1.0 + i as f32 * 0.6) + i as f32 * 13.0, i as f32 * 5.0, 5);
            if v > line {
                let rock = mix3([0.05, 0.04, 0.08], [0.32, 0.22, 0.30], 1.0 - haze);
                let tex = fbm(x * 0.03, y * 0.03 + i as f32 * 9.0, 3) * 0.06;
                c = mix3(add3(rock, [tex, tex, tex]), mix3(low, mid, 0.5), haze * 0.7);
                c = scale3(c, 1.0 - (v - line).min(0.2) * 0.8);
            }
        }
        c
    })
}

/// Terrain seen from above: ridged noise as height, hill-shaded, coloured by height from deep water
/// to snow.
pub fn terrain_pixels(w: u32, h: u32) -> Vec<u8> {
    let height = |x: f32, y: f32| {
        let base = fbm(x * 0.006, y * 0.006, 6);
        let ridge = 1.0 - (fbm(x * 0.012 + 40.0, y * 0.012, 5) * 2.0 - 1.0).abs();
        base * 0.75 + ridge * ridge * 0.35 - 0.12
    };
    let light = norm([-0.6, -0.5, 0.62]);
    rgba(w, h, |x, y| {
        let z = height(x, y);
        let (zx, zy) = (height(x + 1.0, y) - height(x - 1.0, y), height(x, y + 1.0) - height(x, y - 1.0));
        let n = norm([-zx * 90.0, -zy * 90.0, 1.0]);
        let lam = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0);
        let stops: [(f32, [f32; 3]); 8] = [
            (0.0, [0.02, 0.08, 0.20]), (0.28, [0.05, 0.25, 0.42]), (0.33, [0.78, 0.72, 0.52]), (0.36, [0.30, 0.50, 0.20]),
            (0.48, [0.14, 0.32, 0.12]), (0.60, [0.40, 0.34, 0.26]), (0.70, [0.55, 0.52, 0.50]), (0.78, [0.95, 0.96, 0.98]),
        ];
        let mut c = stops[0].1;
        for k in 1..stops.len() { if z > stops[k - 1].0 { c = mix3(stops[k - 1].1, stops[k].1, ((z - stops[k - 1].0) / (stops[k].0 - stops[k - 1].0)).clamp(0.0, 1.0)); } }
        if z < 0.3 { c } else { scale3(c, 0.25 + 0.95 * lam) }
    })
}

/// A lone windswept tree on a straw rise under a pale sky, with a small orange flag far off: a smooth
/// ramp (the sky), a broad textured mid-tone (the field), a dark mass with holes (the crown), one
/// saturated colour and thin hard lines (the flag, the branch). A test picture, not art.
pub fn tree_pixels(w: u32, h: u32) -> Vec<u8> { tree(w, h, false) }
/// The tree alone with a slip of its ground, on a transparent ground (alpha is coverage, edges
/// anti-aliased, the crown's holes transparent).
pub fn cutout_pixels(w: u32, h: u32) -> Vec<u8> { tree(w, h, true) }

/// The tree's crown: discs in tree space (base at 0, 0, up is negative y, units are px at 640 wide).
struct Disc { x: f32, y: f32, r: f32 }
fn crown() -> Vec<Disc> {
    (0..34u64).map(|i| {
        let a = h01(i * 2 + 1) * std::f32::consts::TAU;
        let r = exp_d(0.6 * ln_d(h01(i * 2 + 2).max(1e-6)));
        let (s, c) = (sin_d(a), sin_d(a + std::f32::consts::FRAC_PI_2));
        Disc { x: 36.0 + c * 104.0 * r + 16.0 * r, y: -126.0 + s * 52.0 * r - 10.0 * c * r, r: 17.0 + 17.0 * h01(i * 7 + 3) * (1.15 - 0.5 * r) }
    }).collect()
}
/// One sample of the tree in tree space: (alpha, colour), straight. `sharp` is samples per tree unit,
/// so each sample's edge is a third of a pixel wide.
fn tree_at(discs: &[Disc], x: f32, y: f32, sharp: f32) -> (f32, [f32; 3]) {
    // Trunk: leans leeward as t^1.5, half-width 5.5 → 2.5, lit on its windward (left) side.
    let t = (-y / 78.0).clamp(0.0, 1.0);
    let cx = 22.0 * t * t.sqrt();
    let half = 5.5 - 3.0 * t;
    let trunk = if y <= 0.0 && y > -90.0 { ((half - (x - cx).abs()) * sharp + 0.5).clamp(0.0, 1.0) } else { 0.0 };
    let branch = if x < 12.0 && x > -34.0 { ((1.6 - ((y + 52.0) + (x - 10.0) * 0.75).abs()) * sharp + 0.5).clamp(0.0, 1.0) } else { 0.0 };
    let wood = trunk.max(branch);
    let bark = scale3(hex(0x3a2e26), 0.8 + 0.5 * (0.5 - (x - cx) / 8.0).clamp(0.0, 1.0));
    if !(-240.0..=-15.0).contains(&y) { return (wood, bark); }
    // Crown: the union of the discs, each edge moved by noise; the disc that covers most shades it,
    // as a ball lit from the upper left.
    let wobble = (fbm(x / 9.0 + 40.0, y / 9.0 + 40.0, 3) - 0.5) * 9.0;
    let (mut cov, mut shade) = (0.0f32, 0.0f32);
    for d in discs {
        let (dx, dy) = (x - d.x, (y - d.y) * 1.12);
        if dx.abs() > d.r + 6.0 || dy.abs() > d.r + 6.0 { continue; }
        let c = ((d.r - (dx * dx + dy * dy).sqrt() - wobble) * sharp + 0.5).clamp(0.0, 1.0);
        if c > cov { shade = (0.20 - ((x - d.x) * 0.6 + (y - d.y) * 0.75) / d.r * 0.7).clamp(0.0, 1.0); cov = c; }
    }
    // Sky holes in the crown's lower-middle band.
    if cov > 0.0 && y > -150.0 && y < -90.0 && fbm(x / 22.0 + 44.8, y / 22.0 + 20.6, 3) > 0.735 { cov = 0.0; }
    if cov <= 0.0 { return (wood, bark); }
    let mottle = fbm(x / 3.2 + 77.0, y / 3.2, 3) - 0.5;
    let tone = (shade + mottle * 0.42 - ((y + 96.0) / 40.0).clamp(0.0, 1.0) * 0.28).clamp(0.0, 1.0);
    let leaf = if tone < 0.55 { mix3(hex(0x1c2f22), hex(0x40683a), tone / 0.55) } else { mix3(hex(0x40683a), hex(0xb3bf68), (tone - 0.55) / 0.45) };
    let a = cov + wood * (1.0 - cov);
    (a, scale3(add3(scale3(leaf, cov), scale3(bark, wood * (1.0 - cov))), 1.0 / a))
}
/// The 3 × 3 sample offsets inside a pixel.
const SUB: [f32; 3] = [-1.0 / 3.0, 0.0, 1.0 / 3.0];

fn tree(w: u32, h: u32, cut: bool) -> Vec<u8> {
    let (sw, sh) = (w as f32, h as f32);
    let discs = crown();
    let mut px = vec![0u8; (w * h * 4) as usize];
    let put = |px: &mut Vec<u8>, x: u32, y: u32, c: [f32; 3], a: f32| {
        let o = ((y * w + x) * 4) as usize;
        for k in 0..3 { px[o + k] = (c[k].clamp(0.0, 1.0) * 255.0).round() as u8; }
        px[o + 3] = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
    };
    if cut {
        // The tree at 1.52 × (of 640 wide), base at (232, 366), over a slip of the rise.
        let k = 1.52 * sw / 520.0;
        let (bx, by) = (232.0 * sw / 520.0, 366.0 * sh / 400.0);
        for y in 0..h { for x in 0..w {
            let (mut a, mut c) = (0.0f32, [0.0f32; 3]);
            for sy in SUB { for sx in SUB {
                let (fx, fy) = (x as f32 + 0.5 + sx, y as f32 + 0.5 + sy);
                let (ta, tc) = tree_at(&discs, (fx - bx) / k, (fy - by) / k, k * 3.0);
                let (ga, gc) = slip(fx * 520.0 / sw, fy * 400.0 / sh);
                let sa = ta + ga * (1.0 - ta);
                a += sa;
                c = add3(c, add3(scale3(tc, ta), scale3(gc, ga * (1.0 - ta))));
            } }
            if a > 0.0 { put(&mut px, x, y, scale3(c, 1.0 / a), a / 9.0) } else { put(&mut px, x, y, hex(0x40683a), 0.0) }
        } }
        return px;
    }
    let k = sw / 640.0;
    let (bx, by) = (0.62 * sw, 0.65 * sh + 3.0 * k);
    for y in 0..h { for x in 0..w {
        let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
        let ground = land(fx, fy, sw, sh, k);
        let (tx, ty) = ((fx - bx) / k, (fy - by) / k);
        let near_tree = (-135.0..205.0).contains(&tx) && (-240.0..3.0).contains(&ty);
        let near_flag = (fx / k - 118.0).abs() < 13.0 && (250.0..285.0).contains(&(fy / k));
        if !near_tree && !near_flag { put(&mut px, x, y, ground, 1.0); continue; }
        let mut c = [0.0f32; 3];
        for sy in SUB { for sx in SUB {
            let (sx, sy) = (fx + sx, fy + sy);
            let (a, tc) = if near_tree { tree_at(&discs, (sx - bx) / k, (sy - by) / k, k * 3.0) } else { flag(sx / k, sy / k) };
            c = add3(c, mix3(ground, tc, a));
        } }
        put(&mut px, x, y, scale3(c, 1.0 / 9.0), 1.0);
    } }
    px
}
/// The marker flag far off (one sample, picture px at 640 wide): a pole and an orange pennant, the
/// picture's only saturated colour.
fn flag(x: f32, y: f32) -> (f32, [f32; 3]) {
    if (x - 118.0).abs() < 0.75 && y > 257.0 && y < 282.0 { return (1.0, hex(0x2a2b2d)); }
    if x > 118.7 && x < 129.0 && (y - 261.0).abs() < (129.0 - x) * 0.35 { return (1.0, [1.0, 0.42, 0.17]); }
    (0.0, [0.0; 3])
}
/// Sky, thin cloud, the far ridge and the straw field, one pixel at its centre; the ridge and the
/// ground line are anti-aliased analytically (coverage of the pixel below the line).
fn land(x: f32, y: f32, sw: f32, sh: f32, k: f32) -> [f32; 3] {
    let (u, v) = (x / sw, y / sh);
    let mut c = mix3(hex(0x5b8fc7), hex(0xdfe6e2), (v / 0.70).min(1.0));
    c = mix3(c, hex(0xf6f4ee), cloud(x, y, sh, k));
    let ridge = 0.70 * sh - (6.0 + 12.0 * fbm(x / (120.0 * k) + 8.0, 0.0, 4)) * k;
    c = mix3(c, hex(0xb3c1c6), (y + 0.5 - ridge).clamp(0.0, 1.0));
    let line = ground_line(u, sh);
    let cover = (y + 0.5 - line).clamp(0.0, 1.0);
    if cover <= 0.0 { return c; }
    // The straw's texture is finer than a pixel near the horizon: average it over 3 × 3 samples.
    let mut f = [0.0f32; 3];
    for sy in SUB { for sx in SUB { f = add3(f, field(x + sx, y + sy, line, sh, k)); } }
    let f = scale3(f, 1.0 / 9.0);
    mix3(c, f, cover)
}
/// The straw field at one sample: rows that widen toward the viewer, broad wind marks, stalks in the
/// nearest rows and the tree's shadow, in luminance over a ramp.
fn field(x: f32, y: f32, line: f32, sh: f32, k: f32) -> [f32; 3] {
    let v = y / sh;
    let d = ((y - line) / (sh - line)).clamp(0.0, 1.0);
    let rows = (fbm(x / ((14.0 + 150.0 * d) * k) + 7.0, y / ((2.0 + 10.0 * d) * k), 4) - 0.5) * 0.157;
    let wind = (fbm(x / (260.0 * k) + y / (90.0 * k) + 2.0, y / (200.0 * k), 2) - 0.5) * 0.165;
    let mut f = add3(mix3(hex(0xcdbb78), hex(0xa8893c), d), [rows + wind; 3]);
    let near = ((v - 0.88) / 0.12).clamp(0.0, 1.0);
    let stalks = (fbm(x / (1.6 * k) + 12.0, y / (26.0 * k), 3) - 0.5) * 0.235;
    f = add3(scale3(f, 1.0 - near * 0.3), [stalks * near; 3]);
    let (ex, ey) = ((x / k - 455.0) / 95.0, (y / k - 276.0) / 7.1);
    f = scale3(f, 1.0 - 0.3 * exp_d(-(ex * ex + ey * ey)));
    f
}
/// The thin cloud's opacity: banded noise, at most 0.55.
fn cloud(x: f32, y: f32, sh: f32, k: f32) -> f32 {
    let b = (y / sh - 0.25) / 0.13;
    ((fbm(x / (190.0 * k) + 3.0, y / (30.0 * k), 5) - 0.56) * 4.5).clamp(0.0, 1.0) * exp_d(-b * b) * 0.55
}
/// The horizon, v = 0.70, raised into a rise under the tree.
fn ground_line(u: f32, sh: f32) -> f32 { let r = (u - 0.62) / 0.22; (0.70 - 0.050 * exp_d(-r * r)) * sh }
/// The cut-out's slip of the rise (one sample, cut-out px at 520 × 400): x 90 to 390, its top broken
/// into stalk tips, its ends fading out over 25 px.
fn slip(x: f32, y: f32) -> (f32, [f32; 3]) {
    if !(90.0..390.0).contains(&x) || !(340.0..393.0).contains(&y) { return (0.0, [0.0; 3]); }
    let (b, t) = ((x - 240.0) / 105.0, (x - 240.0) / 120.0);
    let tips = (2.0 + 14.0 * smooth(0.3, 0.75, fbm(x / 2.2 + 4.0, 0.0, 2))) * exp_d(-t * t);
    let top = 372.0 - 12.0 * exp_d(-b * b) - tips;
    let ends = ((x - 90.0) / 25.0).min((390.0 - x) / 25.0).clamp(0.0, 1.0);
    let a = ((y - top) * 3.0 + 0.5).clamp(0.0, 1.0) * ((392.0 - y) * 0.6).clamp(0.0, 1.0) * ends;
    (a, scale3(hex(0xb89a4a), 0.75 + 0.5 * fbm(x / 2.0 + 12.0, y / 14.0, 3)))
}
/// The cut-out's alpha, for its outline (`wrap::Silhouette::from_alpha`).
pub fn cutout_alpha() -> (u32, u32, Vec<u8>) {
    let (w, h) = PICTURE_SIZES[CUTOUT as usize];
    (w, h, cutout_pixels(w, h).chunks(4).map(|p| p[3]).collect())
}

fn hex(c: u32) -> [f32; 3] { [((c >> 16) & 255) as f32 / 255.0, ((c >> 8) & 255) as f32 / 255.0, (c & 255) as f32 / 255.0] }
/// A hash of one integer, 0..1.
fn h01(n: u64) -> f32 {
    let mut n = (n.wrapping_mul(374_761_393).wrapping_add(668_265_263)) & 0xffff_ffff;
    n = ((n ^ (n >> 13)).wrapping_mul(1_274_126_177)) & 0xffff_ffff;
    ((n ^ (n >> 16)) & 0xffff) as f32 / 65535.0
}
// exp, ln and sin from basic arithmetic only, so the tree's bytes do not depend on a platform's libm.
fn exp_d(x: f32) -> f32 {
    let x = x.clamp(-80.0, 80.0);
    let n = (x * std::f32::consts::LOG2_E).round();
    let r = x - n * std::f32::consts::LN_2;
    let p = 1.0 + r * (1.0 + r / 2.0 * (1.0 + r / 3.0 * (1.0 + r / 4.0 * (1.0 + r / 5.0 * (1.0 + r / 6.0)))));
    p * f32::from_bits(((n as i32 + 127) as u32) << 23)
}
fn ln_d(x: f32) -> f32 {
    let b = x.to_bits();
    let e = ((b >> 23) & 255) as f32 - 127.0;
    let m = f32::from_bits((b & 0x007f_ffff) | 0x3f80_0000);
    let s = (m - 1.0) / (m + 1.0);
    let s2 = s * s;
    e * std::f32::consts::LN_2 + 2.0 * s * (1.0 + s2 * (1.0 / 3.0 + s2 * (1.0 / 5.0 + s2 * (1.0 / 7.0 + s2 / 9.0))))
}
fn sin_d(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let mut x = a - (a / TAU).round() * TAU;
    if x > PI / 2.0 { x = PI - x } else if x < -PI / 2.0 { x = -PI - x }
    let x2 = x * x;
    x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0 * (1.0 - x2 / 110.0)))))
}

fn norm(v: [f32; 3]) -> [f32; 3] { let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); [v[0] / l, v[1] / l, v[2] / l] }
/// Towering clouds at dusk: layered fbm with a lit top edge, a deep violet-to-orange ground, and one
/// diagonal streak of light breaking through.
pub fn dusk_pixels(w: u32, h: u32) -> Vec<u8> {
    let (sw, sh) = (w as f32, h as f32);
    rgba(w, h, |x, y| {
        let (u, v) = (x / sw, y / sh);
        let sky = if v < 0.5 { mix3([0.03, 0.03, 0.14], [0.36, 0.08, 0.42], v / 0.5) } else { mix3([0.36, 0.08, 0.42], [1.0, 0.42, 0.10], ((v - 0.5) / 0.32).min(1.0)) };
        // A towering cloud bank rising from the horizon: broad fbm, its density highest low down.
        let dens = |x: f32, y: f32| {
            let (u, v) = (x / sw, y / sh);
            fbm(x * 0.0042 + 3.0, y * 0.0058, 5) + v * 0.55 - 0.18 * (u * 6.0).sin().abs() - 0.15
        };
        let d = dens(x, y);
        let a = smooth(0.50, 0.56, d);
        // Lit from the upper right: the side facing it is bright, a thin rim at the top edge.
        let toward = (d - dens(x + 7.0, y - 9.0)).clamp(-0.06, 0.06) / 0.12 + 0.5;
        let rim = smooth(0.50, 0.53, d) * (1.0 - smooth(0.53, 0.60, d));
        let cloud = add3(mix3([0.08, 0.03, 0.16], [0.95, 0.48, 0.30], toward.clamp(0.0, 1.0)), scale3([1.0, 0.85, 0.6], rim * 0.9));
        let mut c = mix3(sky, cloud, a);
        // The streak: a soft band of light breaking through from the upper right.
        let s = (u * 0.75 + v - 0.98) / 0.045;
        let band = (-s * s).exp() * smooth(0.15, 0.9, u);
        c = add3(c, scale3([1.0, 0.9, 0.7], band * (0.55 - a * 0.3)));
        // The sea: dark, with the light laid on it in short horizontal strokes.
        if v > 0.84 {
            let k = (v - 0.84) / 0.16;
            c = mix3([0.20, 0.05, 0.16], [0.02, 0.01, 0.05], k);
            let glint = (-((u - 0.66) / (0.05 + 0.1 * k)).powi(2)).exp() * smooth(0.55, 0.75, fbm(x * 0.08, y * 0.9, 2));
            c = add3(c, scale3([1.0, 0.55, 0.2], glint * (1.0 - k * 0.5)));
        }
        c
    })
}

/// A planet with an atmosphere's bright rim against stars, lit from the right, a ring crossing it.
pub fn planet_pixels(w: u32, h: u32) -> Vec<u8> {
    let (sw, sh) = (w as f32, h as f32);
    let (cx, cy, r) = (sw * 0.42, sh * 0.56, sh * 0.36);
    let light = norm([0.75, -0.35, 0.45]);
    rgba(w, h, |x, y| {
        let star = hash(x as i32, y as i32);
        let mut c = [0.01, 0.01, 0.04];
        if star > 0.9985 { c = [0.9, 0.9, 1.0]; } else if star > 0.996 { c = [0.35, 0.35, 0.5]; }
        let (dx, dy) = ((x - cx) / r, (y - cy) / r);
        let d2 = dx * dx + dy * dy;
        // The ring behind and in front: an ellipse band.
        let ring = |x: f32, y: f32| { let (rx, ry) = ((x - cx) / (r * 1.9), (y - cy) / (r * 0.42)); let rr = (rx * rx + ry * ry).sqrt(); smooth(0.78, 0.8, rr) * smooth(1.0, 0.97, rr) };
        let rg = ring(x, y);
        if rg > 0.0 && (y < cy || d2 > 1.0) { c = mix3(c, [0.95, 0.72, 0.42], rg * 0.85); }
        if d2 < 1.0 {
            let dz = (1.0 - d2).sqrt();
            let lam = (dx * light[0] + dy * light[1] + dz * light[2]).max(0.0);
            let bands = fbm(dx * 2.0 + 3.0, dy * 9.0, 4);
            let base = mix3([0.85, 0.25, 0.10], [0.15, 0.55, 0.85], smooth(0.35, 0.65, bands));
            c = scale3(base, 0.04 + lam * 1.05);
            // The rim: the atmosphere, bright on the lit limb.
            let rim = smooth(0.80, 1.0, d2.sqrt()) * (0.3 + lam);
            c = add3(c, scale3([0.55, 0.85, 1.0], rim * 0.9));
            if rg > 0.0 && y > cy { c = mix3(c, [0.95, 0.72, 0.42], rg * 0.9); }
        } else {
            let halo = (-(d2.sqrt() - 1.0) * 14.0).exp() * 0.6;
            let side = (dx * light[0] + dy * light[1]).max(0.0) + 0.15;
            c = add3(c, scale3([0.35, 0.65, 1.0], halo * side));
        }
        c
    })
}

/// A flower: six petals from a polar SDF, saturated magenta with lit edges, on deep green.
pub fn flower_pixels(w: u32, h: u32) -> Vec<u8> {
    let (sw, sh) = (w as f32, h as f32);
    let (cx, cy) = (sw * 0.5, sh * 0.5);
    let s = sw.min(sh);
    rgba(w, h, |x, y| {
        let (dx, dy) = ((x - cx) / s, (y - cy) / s);
        let r = (dx * dx + dy * dy).sqrt();
        let a = dy.atan2(dx);
        let leaf = fbm(x * 0.02, y * 0.02, 4);
        let mut c = mix3([0.0, 0.07, 0.03], [0.05, 0.28, 0.10], leaf * smooth(0.7, 0.2, r));
        // Two rings of petals.
        for (k, (n, len, rot, col)) in [(6.0f32, 0.42f32, 0.0f32, [0.95f32, 0.12, 0.45]), (6.0, 0.30, 0.52, [1.0, 0.35, 0.60])].into_iter().enumerate() {
            let p = ((a + rot) * n * 0.5).cos().abs();
            let edge = len * p.powf(0.7);
            if r < edge {
                let t = r / edge;
                let shade = 0.35 + 0.65 * (1.0 - t * 0.6) * (0.6 + 0.4 * p);
                let mut pc = scale3(col, shade);
                pc = add3(pc, scale3([1.0, 0.85, 0.9], smooth(0.82, 1.0, t) * 0.55));
                let vein = ((a + rot) * n * 4.0).sin().abs();
                pc = scale3(pc, 1.0 - 0.12 * smooth(0.9, 1.0, vein) * (1.0 - t));
                c = if k == 0 { pc } else { mix3(c, pc, 1.0) };
            }
        }
        if r < 0.085 { let t = r / 0.085; c = mix3([1.0, 0.85, 0.2], [0.55, 0.25, 0.02], t); if hash((x * 0.5) as i32, (y * 0.5) as i32) > 0.7 { c = scale3(c, 0.6); } }
        c
    })
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] { [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t] }
fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn scale3(a: [f32; 3], k: f32) -> [f32; 3] { [a[0] * k, a[1] * k, a[2] * k] }
fn smooth(e0: f32, e1: f32, x: f32) -> f32 { let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }
fn hash(x: i32, y: i32) -> f32 {
    let mut n = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    ((n ^ (n >> 16)) & 0xffff) as f32 / 65535.0
}
fn noise(x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let (ux, uy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let a = hash(xi, yi) + (hash(xi + 1, yi) - hash(xi, yi)) * ux;
    let b = hash(xi, yi + 1) + (hash(xi + 1, yi + 1) - hash(xi, yi + 1)) * ux;
    a + (b - a) * uy
}
/// Sum of octaves of value noise, about 0..1.
fn fbm(x: f32, y: f32, octaves: u32) -> f32 {
    let (mut s, mut a, mut f, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for _ in 0..octaves { s += a * noise(x * f, y * f); norm += a; a *= 0.5; f *= 2.03; }
    s / norm
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u32 = 640;
    const H: u32 = 400;
    fn luma(p: &[u8]) -> f32 { (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0 }
    fn picture() -> Vec<u8> { tree_pixels(W, H) }
    /// The tree in the picture, sampled at a pixel's centre: (alpha, is in the crown).
    fn tree_here(x: u32, y: u32) -> (f32, bool) {
        let (bx, by) = (0.62 * W as f32, 0.65 * H as f32 + 3.0);
        let (tx, ty) = (x as f32 + 0.5 - bx, y as f32 + 0.5 - by);
        let (a, _) = tree_at(&crown(), tx, ty, 3.0);
        (a, ty < -70.0 && a >= 0.999)
    }

    #[test]
    fn luminance_spans_the_palette_from_crown_shade_to_horizon() {
        let px = picture();
        let (lo, hi) = px.chunks(4).map(luma).fold((1.0f32, 0.0f32), |(lo, hi), l| (lo.min(l), hi.max(l)));
        assert!(lo <= 0.17 && hi >= 0.87, "luminance spans {lo:.3}..{hi:.3}");
    }

    #[test]
    fn the_sky_is_a_ramp_that_the_cloud_barely_touches() {
        let px = picture();
        // Left of the tree and above the ridge, a row's darkest pixel is the ramp itself (cloud only
        // brightens), and it brightens row by row toward the horizon.
        let mut last = 0.0;
        for y in 0..250 {
            let m = (0..260).map(|x| luma(&px[((y * W + x) * 4) as usize..])).fold(1.0f32, f32::min);
            assert!(m >= last, "row {y}: {m} after {last}");
            last = m;
        }
        let (mut sky, mut cloudy) = (0, 0);
        for y in 0..H { for x in 0..W {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            if fy < ground_line(fx / W as f32, H as f32) - 1.0 && fy < 0.70 * H as f32 - 19.0 {
                sky += 1;
                if cloud(fx, fy, H as f32, 1.0) > 0.02 { cloudy += 1; }
            }
        } }
        assert!((cloudy as f32) < 0.15 * sky as f32, "cloud covers {cloudy} of {sky} sky pixels");
    }

    #[test]
    fn one_bit_dither_keeps_crown_field_and_sky_apart() {
        let px = picture();
        let b4 = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
        let (mut crown, mut field, mut top, mut horizon) = ([0u32; 2], [0u32; 2], [0u32; 2], [0u32; 2]);
        let line = |x: u32| ground_line((x as f32 + 0.5) / W as f32, H as f32);
        for y in 0..H / 2 { for x in 0..W / 2 {
            let src = [(2 * x, 2 * y), (2 * x + 1, 2 * y), (2 * x, 2 * y + 1), (2 * x + 1, 2 * y + 1)];
            let g = src.iter().map(|&(sx, sy)| luma(&px[((sy * W + sx) * 4) as usize..])).sum::<f32>() / 4.0;
            let ink = (g <= b4[(y % 4) as usize][(x % 4) as usize] as f32 / 16.0 + 1.0 / 32.0) as u32;
            let add = |c: &mut [u32; 2]| { c[0] += ink; c[1] += 1; };
            if src.iter().all(|&(sx, sy)| tree_here(sx, sy).1) { add(&mut crown); }
            if src.iter().all(|&(sx, sy)| sy as f32 > line(sx) + 1.0 && tree_here(sx, sy).0 == 0.0) { add(&mut field); }
            if x < 130 && y < 4 { add(&mut top); }
            if x < 130 && (124..128).contains(&y) { add(&mut horizon); }
        } }
        let f = |c: [u32; 2]| c[0] as f32 / c[1] as f32;
        assert!(f(crown) > 0.70, "crown {:.3} ink over {} px", f(crown), crown[1]);
        assert!((0.20..0.50).contains(&f(field)), "field {:.3} ink", f(field));
        assert!(f(horizon) <= 0.75 * f(top), "sky ink {:.3} at the top, {:.3} at the horizon", f(top), f(horizon));
    }

    #[test]
    fn the_cut_out_has_an_inside_and_an_overhang() {
        let (w, h, a) = cutout_alpha();
        let (w, h) = (w as usize, h as usize);
        let solid = |i: usize| a[i] >= 128;
        // Flood the transparent pixels from the border; a hole is one it never reaches.
        let mut seen = vec![false; w * h];
        let mut stack: Vec<usize> = (0..w * h).filter(|&i| (i % w == 0 || i % w == w - 1 || i / w == 0 || i / w == h - 1) && !solid(i)).collect();
        while let Some(i) = stack.pop() {
            if seen[i] || solid(i) { continue; }
            seen[i] = true;
            let (x, y) = (i % w, i / w);
            if x > 0 { stack.push(i - 1); }
            if x + 1 < w { stack.push(i + 1); }
            if y > 0 { stack.push(i - w); }
            if y + 1 < h { stack.push(i + w); }
        }
        let holes = (0..w * h).filter(|&i| !solid(i) && !seen[i]).count();
        assert!(holes > 0, "the crown has no hole");
        let span = |y: usize| { let xs: Vec<usize> = (0..w).filter(|&x| solid(y * w + x)).collect(); (xs[0], *xs.last().unwrap()) };
        let widest = (0..300).filter(|&y| (0..w).any(|x| solid(y * w + x))).map(span).max_by_key(|(l, r)| r - l).unwrap();
        // 40 rows above the base: the trunk's own run sits 90 px inside the crown on both sides; with
        // the windward branch the row still clears the leeward overhang by 90 px.
        let y = 366 - 40;
        let (l, r) = span(y);
        let mut tl = 232;
        while !solid(y * w + tl) { tl += 1; }
        while solid(y * w + tl - 1) { tl -= 1; }
        let mut tr = tl;
        while solid(y * w + tr + 1) { tr += 1; }
        assert!(tl >= widest.0 + 90 && tr + 90 <= widest.1, "trunk {tl}..{tr}, crown {widest:?}");
        assert!(r + 90 <= widest.1 && l >= widest.0 + 40, "row {l}..{r}, crown {widest:?}");
    }

    #[test]
    fn the_flag_is_the_only_saturated_colour() {
        let px = picture();
        let sat = |i: usize| { let p = &px[i * 4..i * 4 + 3]; let (mx, mn) = (*p.iter().max().unwrap() as f32, *p.iter().min().unwrap() as f32); mx > 0.0 && (mx - mn) / mx > 0.8 };
        let (w, h) = (W as usize, H as usize);
        let mut seen = vec![false; w * h];
        let mut regions = 0;
        for start in 0..w * h {
            if seen[start] || !sat(start) { continue; }
            regions += 1;
            let mut stack = vec![start];
            while let Some(i) = stack.pop() {
                if seen[i] || !sat(i) { continue; }
                seen[i] = true;
                let (x, y) = ((i % w) as i32, (i / w) as i32);
                for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx >= 0 && ny >= 0 && nx < w as i32 && ny < h as i32 { stack.push(ny as usize * w + nx as usize); }
                }
            }
        }
        assert_eq!(regions, 1);
    }

    #[test]
    fn the_same_bytes_every_time() {
        let fnv = |b: &[u8]| b.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &x| (h ^ x as u64).wrapping_mul(0x100_0000_01b3));
        let (p, c) = (picture(), cutout_pixels(520, 400));
        assert_eq!(p, picture());
        assert_eq!((fnv(&p), fnv(&c)), (14_585_934_577_113_153_037, 17_751_255_081_525_652_210), "the tree's bytes changed: pin the new hashes once the picture is meant to change");
    }
}
