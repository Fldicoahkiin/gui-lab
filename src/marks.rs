//! The mark atlas and the palettes, made on the CPU at start-up (DESIGN.md §1.6.1).
//!
//! The atlas is RG8: R is how much of the mark covers a texel, G is its height (for relief). One row
//! per mark set, one column per brightness level (16), each tile 32 × 32 and seamless across its
//! edges, so marks rotated or tiled through `fract` never show a seam.
use crate::style::set;
use crate::text::Text;

pub const TILE: usize = 32;
pub const LEVELS: usize = 16;
pub const WIDTH: usize = TILE * LEVELS;
pub const HEIGHT: usize = TILE * set::COUNT;
/// Mip levels down to one texel per tile, so small cells read an average of the mark, not a few
/// aliased texels of it.
pub const MIPS: usize = 6;

/// Candidate characters for the character set: printable ASCII. They are ordered by measured ink,
/// never by how dense they look.
pub const CANDIDATES: &str = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";

/// 8 × 8 Bayer threshold, 0..63, by bit-interleaving (the same formula as the shader's `bayer8`).
pub fn bayer8(x: u32, y: u32) -> u32 {
    let (x, y) = (x & 7, y & 7);
    let a = x ^ y;
    ((a & 1) << 5) | ((y & 1) << 4) | ((a & 2) << 2) | ((y & 2) << 1) | ((a & 4) >> 1) | ((y & 4) >> 2)
}

pub struct Atlas {
    /// RG8, `WIDTH` × `HEIGHT`, level 0 only.
    pub pixels: Vec<u8>,
    /// The characters chosen for each level of the character set.
    pub chars: [char; LEVELS],
    /// Their measured ink (sum of coverage over the tile, 0..1 per texel).
    pub ink: [f32; LEVELS],
}

impl Atlas {
    /// Mark coverage and height at texel (x, y) of tile (set, level), 0..1.
    pub fn at(&self, s: usize, level: usize, x: usize, y: usize) -> (f32, f32) {
        let o = ((s * TILE + y) * WIDTH + level * TILE + x) * 2;
        (self.pixels[o] as f32 / 255.0, self.pixels[o + 1] as f32 / 255.0)
    }

    /// `face` is the face the character set is cut from (a monospaced pixel face reads best).
    pub fn build(text: &mut Text, face: u8) -> Self {
        let mut pixels = vec![0u8; WIDTH * HEIGHT * 2];
        let (chars, ink, tiles) = characters(text, face);
        for (level, tile) in tiles.iter().enumerate() {
            put(&mut pixels, set::CHARS as usize, level, |x, y| { let c = tile[y * TILE + x]; (c, c) });
        }
        for s in 1..set::COUNT as u8 {
            for level in 0..LEVELS {
                let t = level as f32 / (LEVELS - 1) as f32;
                put(&mut pixels, s as usize, level, |x, y| geometric(s, t, x, y));
            }
        }
        Self { pixels, chars, ink }
    }

    /// Every mip level, largest first, each RG8 and half the size of the one before.
    pub fn mips(&self) -> Vec<(usize, usize, Vec<u8>)> {
        let mut out = vec![(WIDTH, HEIGHT, self.pixels.clone())];
        for _ in 1..MIPS {
            let (w, h, prev) = out.last().expect("level 0");
            let (nw, nh) = (w / 2, h / 2);
            let mut next = vec![0u8; nw * nh * 2];
            for y in 0..nh { for x in 0..nw { for c in 0..2 {
                let at = |dx: usize, dy: usize| prev[((y * 2 + dy) * w + x * 2 + dx) * 2 + c] as u32;
                next[(y * nw + x) * 2 + c] = ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4) as u8;
            } } }
            out.push((nw, nh, next));
        }
        out
    }
}

fn put(pixels: &mut [u8], s: usize, level: usize, f: impl Fn(usize, usize) -> (f32, f32)) {
    for y in 0..TILE { for x in 0..TILE {
        let (m, h) = f(x, y);
        let o = ((s * TILE + y) * WIDTH + level * TILE + x) * 2;
        pixels[o] = (m.clamp(0.0, 1.0) * 255.0).round() as u8;
        pixels[o + 1] = (h.clamp(0.0, 1.0) * 255.0).round() as u8;
    } }
}

/// Rasterise every candidate into a tile, measure its ink, then pick the 16 whose ink lies nearest to
/// evenly spaced targets from none to the most, keeping the order strict.
fn characters(text: &mut Text, face: u8) -> ([char; LEVELS], [f32; LEVELS], Vec<Vec<f32>>) {
    let mut cands: Vec<(char, f32, Vec<f32>)> = CANDIDATES.chars().map(|ch| {
        let tile = char_tile(text, face, ch);
        let ink = tile.iter().sum::<f32>();
        (ch, ink, tile)
    }).collect();
    cands.sort_by(|a, b| a.1.total_cmp(&b.1));
    let max = cands.last().map(|c| c.1).unwrap_or(1.0);
    let mut chosen: Vec<usize> = vec![0];
    for j in 1..LEVELS {
        let target = max * j as f32 / (LEVELS - 1) as f32;
        let last = *chosen.last().expect("first");
        // Strictly more ink than the previous pick, and leave enough candidates for the levels after.
        let room = cands.len() - (LEVELS - 1 - j);
        let pick = (last + 1..room)
            .filter(|&i| cands[i].1 > cands[last].1)
            .min_by(|&a, &b| (cands[a].1 - target).abs().total_cmp(&(cands[b].1 - target).abs()))
            .unwrap_or((last + 1).min(cands.len() - 1));
        chosen.push(pick);
    }
    let mut chars = [' '; LEVELS];
    let mut ink = [0.0; LEVELS];
    let mut tiles = Vec::with_capacity(LEVELS);
    for (level, &i) in chosen.iter().enumerate() {
        chars[level] = cands[i].0;
        ink[level] = cands[i].1;
        tiles.push(cands[i].2.clone());
    }
    (chars, ink, tiles)
}

/// One character in a 32 × 32 tile. The face is drawn at twice its native size (each font pixel is
/// 2 × 2) and then widened twice by repeating columns, so its cell (7 × 14 font pixels for Departure
/// Mono) fills the square tile the way ASCII art fills its cells. Coverage at the tile edges is left
/// empty, so tiles join without seams.
fn char_tile(text: &mut Text, face: u8, ch: char) -> Vec<f32> {
    let mut tile = vec![0.0f32; TILE * TILE];
    let px = text.native(face).unwrap_or(16.0) * 2.0;
    let Some(g) = text.raster(face, ch, px) else { return tile };
    // The cell: advance wide, ascent + descent tall, centred in the tile after widening.
    let (cw, chh) = (g.advance * 2.0, g.ascent + g.descent);
    let (ox, oy) = ((TILE as f32 - cw) * 0.5, (TILE as f32 - chh) * 0.5);
    for y in 0..g.h as i32 { for x in 0..g.w as i32 {
        let c = g.data[(y as u32 * g.w + x as u32) as usize] as f32 / 255.0;
        if c <= 0.0 { continue; }
        let ty = (oy + g.ascent - g.top as f32 + y as f32).round() as i32;
        for dx in 0..2 {
            let tx = (ox + (g.left + x) as f32 * 2.0).round() as i32 + dx;
            if tx >= 1 && ty >= 1 && tx < TILE as i32 - 1 && ty < TILE as i32 - 1 { tile[ty as usize * TILE + tx as usize] = c; }
        }
    } }
    tile
}

/// Coverage and height of a geometric mark at texel (x, y), level fraction `t` (0 = none … 1 = most).
/// Each texel is averaged over 4 × 4 sub-samples so edges are smooth; positions are in tile units,
/// centre (0.5, 0.5).
fn geometric(s: u8, t: f32, x: usize, y: usize) -> (f32, f32) {
    let n = 4;
    let (mut m, mut h) = (0.0, 0.0);
    for sy in 0..n { for sx in 0..n {
        let u = (x as f32 + (sx as f32 + 0.5) / n as f32) / TILE as f32;
        let v = (y as f32 + (sy as f32 + 0.5) / n as f32) / TILE as f32;
        let (a, b) = sample(s, t, u, v, x, y);
        m += a; h += b;
    } }
    let k = (n * n) as f32;
    (m / k, h / k)
}

fn sample(s: u8, t: f32, u: f32, v: f32, x: usize, y: usize) -> (f32, f32) {
    let (dx, dy) = (u - 0.5, v - 0.5);
    let r = (dx * dx + dy * dy).sqrt();
    let on = |b: bool| if b { 1.0 } else { 0.0 };
    // Distance from the nearest tile edge, 0 at the edge, 0.5 at the centre.
    let edge = dx.abs().max(dy.abs());
    let inset = 0.5 - edge;
    match s {
        // The 8 × 8 Bayer matrix enlarged to the tile (4 texels per entry); level k lights the entries
        // whose threshold is below k / 15, so level 0 is empty and the last is solid.
        set::DITHER => { let b = (bayer8(x as u32 / 4, y as u32 / 4) as f32 + 0.5) / 64.0; let m = on(b < t); (m, m) }
        // Solid, with a very faint line along one edge: neighbouring cells then read as a grid of
        // pixels. The line is on one side only, so tiling gives one line per cell.
        set::PIXEL => (if x == 0 || y == 0 { 0.93 } else { 1.0 }, 1.0),
        // A tile: solid face, a thin dark grout and bevelled edges in the height channel.
        set::MOSAIC => (if inset < 1.0 / 32.0 { 0.55 } else { 1.0 }, (inset / 0.16).min(1.0).sqrt()),
        // A brick: solid with a round stud; the stud is in the height channel, ringed by a faint line.
        set::BRICK => {
            let stud = 0.27;
            let dome = if r < stud { 0.35 + 0.65 * (1.0 - (r / stud).powi(2)).sqrt() } else { 0.35 * (inset / 0.08).min(1.0) };
            (if (r - stud).abs() < 0.02 { 0.82 } else { 1.0 }, dome)
        }
        // Area grows with the level.
        set::DOTS => { let rr = (t / std::f32::consts::PI).sqrt(); let m = on(r < rr); (m, m * (1.0 - r / rr.max(1e-3)).max(0.0).sqrt()) }
        set::CROSS => {
            let w = 0.04 + 0.22 * t; let l = 0.18 + 0.3 * t;
            let m = on(t > 0.0 && ((dx.abs() < w && dy.abs() < l) || (dy.abs() < w && dx.abs() < l)));
            (m, m)
        }
        // One 45° line per tile, measured on the torus so the tile wraps: thickness grows with level.
        set::DIAGONAL => {
            let d = ((u - v).rem_euclid(1.0) - 0.5).abs(); // 0.5 on the line through the corners
            let dist = (0.5 - d) * std::f32::consts::FRAC_1_SQRT_2;
            let m = on(dist < t * 0.36);
            (m, m)
        }
        set::DIAMOND => { let rr = (t * 0.5).sqrt(); let m = on(dx.abs() + dy.abs() < rr); (m, m) }
        set::LINES => { let m = on(dy.abs() < t * 0.5); (m, m) }
        // A soft dot with a glow: brighter and wider with the level, nothing at level 0.
        set::GLOW => { let rr = 0.08 + 0.3 * t; let m = t.sqrt() * (-(r / rr).powi(2) * 1.6).exp(); (m, m) }
        // A horizontal stroke nearly the length of the tile, with round ends; the shader turns it, and
        // turned strokes wrap at the tile edge into their neighbours like hatching. Thicker with the level.
        set::STROKE => {
            let half = 0.44; let w = 0.03 + 0.2 * t;
            let px = (dx.abs() - half).max(0.0);
            let m = on(t > 0.0 && (px * px + dy * dy).sqrt() < w);
            (m, m)
        }
        _ => (0.0, 0.0),
    }
}

/// Palettes: one row of 16 colours each, with how many of them are meant to be used.
pub const PALETTES: [(&str, usize, [[u8; 3]; 16]); 6] = [
    ("黑白", 2, pad(&[[18, 18, 22], [238, 233, 216]])),
    ("孔版三色", 3, pad(&[[30, 34, 78], [255, 76, 126], [247, 238, 214]])),
    ("掌机绿", 4, pad(&[[15, 56, 15], [48, 98, 48], [139, 172, 15], [196, 216, 120]])),
    ("CGA", 4, pad(&[[0, 0, 0], [85, 255, 255], [255, 85, 255], [255, 255, 255]])),
    ("暖色五级", 5, pad(&[[28, 16, 18], [110, 40, 36], [204, 94, 52], [242, 170, 84], [252, 236, 196]])),
    ("PICO-8", 16, [
        [0, 0, 0], [29, 43, 83], [126, 37, 83], [0, 135, 81], [171, 82, 54], [95, 87, 79], [194, 195, 199], [255, 241, 232],
        [255, 0, 77], [255, 163, 0], [255, 236, 39], [0, 228, 54], [41, 173, 255], [131, 118, 156], [255, 119, 168], [255, 204, 170],
    ]),
];

const fn pad(c: &[[u8; 3]]) -> [[u8; 3]; 16] {
    let mut out = [[0u8; 3]; 16];
    let mut i = 0;
    while i < 16 { out[i] = if i < c.len() { c[i] } else { c[c.len() - 1] }; i += 1; }
    out
}

/// RGBA8, 16 × `PALETTES.len()`.
pub fn palette_pixels() -> Vec<u8> {
    PALETTES.iter().flat_map(|(_, _, row)| row.iter().flat_map(|c| [c[0], c[1], c[2], 255])).collect()
}
