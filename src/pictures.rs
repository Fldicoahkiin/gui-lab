//! Sample pictures computed in code (no image files): a lit sphere, a sky, terrain, a bust and its
//! cutout, dusk, a planet and a flower. `offscreen::Offscreen` registers them all under these ids, so
//! recipes, tests and tools can show a picture without shipping one.
pub const SPHERE: u32 = 0;
pub const SKY: u32 = 1;
pub const TERRAIN: u32 = 2;
pub const BUST: u32 = 3;
/// The bust cut out of its ground: transparent around its silhouette, so it has a true outline.
pub const CUTOUT: u32 = 4;
/// Saturated, high-contrast subjects with deep darks and bright rims, which is what marks read best:
/// a cloudscape crossed by a streak of light, a planet against the dark, a flower.
pub const DUSK: u32 = 5;
pub const PLANET: u32 = 6;
pub const FLOWER: u32 = 7;
pub const PICTURE_NAMES: [&str; 4] = ["球", "天空", "地形", "胸像"];
pub const PICTURE_SIZES: [(u32, u32); 8] = [(640, 320), (640, 400), (640, 400), (640, 400), (520, 400), (640, 400), (640, 400), (480, 480)];

/// Every demo picture: id, width, height, RGBA8.
pub fn pictures() -> Vec<(u32, u32, u32, Vec<u8>)> {
    let f: [fn(u32, u32) -> Vec<u8>; 8] = [picture_pixels, sky_pixels, terrain_pixels, bust_pixels, cutout_pixels, dusk_pixels, planet_pixels, flower_pixels];
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

/// A plaster bust in the dark, lit from the upper left: a height field made from soft ellipsoids
/// (head, brow, nose, cheeks, chin, neck, shoulders) with carved eyes and noisy hair, then shaded.
pub fn bust_pixels(w: u32, h: u32) -> Vec<u8> { bust(w, h, false) }
/// The bust with its ground transparent (alpha 0 outside the figure, edges anti-aliased).
pub fn cutout_pixels(w: u32, h: u32) -> Vec<u8> { bust(w, h, true) }
fn bust(w: u32, h: u32, cut: bool) -> Vec<u8> {
    let (sw, sh) = (w as f32, h as f32);
    let k = sh / 400.0;
    let cx = sw * 0.5;
    let ell = |x: f32, y: f32, c: V3, r: V3| {
        let (dx, dy) = ((x - c.0) / r.0, (y - c.1) / r.1);
        let q = 1.0 - dx * dx - dy * dy;
        if q > 0.0 { c.2 + r.2 * q.sqrt() } else { -400.0 }
    };
    let smax = |a: f32, b: f32, s: f32| { let m = a.max(b); m + s * ((-(a - m).abs() / s).exp() + (-(b - m).abs() / s).exp()).ln() };
    let z = move |x: f32, y: f32| {
        let parts: [(V3, V3); 9] = [
            ((cx, 150.0 * k, 0.0), (82.0 * k, 108.0 * k, 85.0 * k)),
            ((cx, 118.0 * k, 50.0 * k), (60.0 * k, 16.0 * k, 26.0 * k)),
            ((cx, 160.0 * k, 66.0 * k), (12.0 * k, 34.0 * k, 26.0 * k)),
            ((cx - 34.0 * k, 178.0 * k, 50.0 * k), (26.0 * k, 22.0 * k, 16.0 * k)),
            ((cx + 34.0 * k, 178.0 * k, 50.0 * k), (26.0 * k, 22.0 * k, 16.0 * k)),
            ((cx, 228.0 * k, 38.0 * k), (34.0 * k, 26.0 * k, 32.0 * k)),
            ((cx, 300.0 * k, -10.0 * k), (46.0 * k, 90.0 * k, 50.0 * k)),
            ((cx, 430.0 * k, -30.0 * k), (240.0 * k, 120.0 * k, 80.0 * k)),
            ((cx, 205.0 * k, 52.0 * k), (22.0 * k, 7.0 * k, 10.0 * k)),
        ];
        let mut h = -400.0;
        for (c, r) in parts { h = smax(h, ell(x, y, c, r), 6.0 * k); }
        // Eye sockets and the mouth line are carved; hair is noise on the crown.
        for ex in [-30.0f32, 30.0] {
            let d = ((x - cx - ex * k) / (17.0 * k)).powi(2) + ((y - 140.0 * k) / (10.0 * k)).powi(2);
            h -= 14.0 * k * (-d).exp();
        }
        let dm = ((x - cx) / (18.0 * k)).powi(2) + ((y - 212.0 * k) / (2.5 * k)).powi(2);
        h -= 5.0 * k * (-dm).exp();
        let crown = smooth(120.0 * k, 70.0 * k, y) * smooth(-400.0, 0.0, h);
        h += crown * (fbm(x * 0.05, y * 0.012, 4) * 14.0 * k);
        h
    };
    let light = norm([-0.55, -0.6, 0.58]);
    let mut px = rgba(w, h, |x, y| {
        let hz = z(x, y);
        let bg = { let d = ((x - sw * 0.35).powi(2) + (y - sh * 0.3).powi(2)).sqrt() / sh; scale3([0.10, 0.10, 0.13], (1.0 - d).max(0.0).powi(3)) };
        if hz < -300.0 { return bg; }
        let (zx, zy) = (z(x + 1.0, y) - z(x - 1.0, y), z(x, y + 1.0) - z(x, y - 1.0));
        let n = norm([-zx * 0.5, -zy * 0.5, 1.0]);
        let lam = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0);
        let half = norm([light[0], light[1], light[2] + 1.0]);
        let spec = (n[0] * half[0] + n[1] * half[1] + n[2] * half[2]).max(0.0).powf(30.0) * 0.25;
        let rim = (n[0] * 0.8).max(0.0).powi(3) * 0.35;
        let albedo = [0.86, 0.79, 0.70];
        let l = lam.powf(1.4);
        add3(scale3(albedo, 0.03 + 0.95 * l), [spec + rim * 0.5, spec + rim * 0.6, spec + rim * 0.8])
    });
    if cut {
        // Coverage from four samples a texel: the figure is where the height field is.
        for y in 0..h { for x in 0..w {
            let n = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)].iter().filter(|(dx, dy)| z(x as f32 + dx, y as f32 + dy) >= -300.0).count();
            px[((y * w + x) * 4 + 3) as usize] = (n as f32 / 4.0 * 255.0).round() as u8;
        } }
    }
    px
}
/// The cut-out's alpha, for its outline (`wrap::Silhouette::from_alpha`).
pub fn cutout_alpha() -> (u32, u32, Vec<u8>) {
    let (w, h) = PICTURE_SIZES[CUTOUT as usize];
    (w, h, cutout_pixels(w, h).chunks(4).map(|p| p[3]).collect())
}

/// Centre or radii of an ellipsoid, px.
type V3 = (f32, f32, f32);
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
