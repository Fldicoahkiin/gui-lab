//! Text: shaping and line breaking by cosmic-text, then every glyph is kept as its own instance.
//! Each glyph is rasterised once into an atlas that stores two things per texel: coverage, and how
//! close that texel is to the nearest ink (1 at ink, falling to 0 at `SPREAD` pixels away).
use cosmic_text::{Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent, fontdb};
use std::collections::HashMap;

pub const SPREAD: i32 = 12;
pub const ATLAS: u32 = 2048;

/// One glyph on screen, in device pixels. `rect` already includes the spread padding.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlyphInst {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    /// rgb: tone (text is drawn as a mix of ground and ink by this amount). a: unused.
    pub tone: [f32; 4],
}

#[derive(Clone, Copy)]
struct Slot { x: u32, y: u32, w: u32, h: u32, left: i32, top: i32 }

pub struct Text {
    fonts: FontSystem,
    cache: SwashCache,
    family: String,
    slots: HashMap<CacheKey, Option<Slot>>,
    /// RG8: coverage, nearness-to-ink.
    pub pixels: Vec<u8>,
    pub dirty: bool,
    pen: (u32, u32, u32),
}

impl Text {
    pub fn new(font: Vec<u8>) -> Self {
        let mut db = fontdb::Database::new();
        db.load_font_data(font);
        let family = db.faces().next().map(|f| f.families[0].0.clone()).unwrap_or_default();
        Self {
            fonts: FontSystem::new_with_locale_and_db("zh-CN".into(), db),
            cache: SwashCache::new(),
            family,
            slots: HashMap::new(),
            pixels: vec![0; (ATLAS * ATLAS * 2) as usize],
            dirty: true,
            pen: (1, 1, 0),
        }
    }

    /// Lay out a paragraph `width` wide (logical px) with its top-left at `origin`, append its glyphs,
    /// and return its height in logical px.
    pub fn paragraph(&mut self, out: &mut Vec<GlyphInst>, s: &str, size: f32, line: f32, width: f32, origin: (f32, f32), tone: f32, scale: f32) -> f32 {
        let mut buf = Buffer::new(&mut self.fonts, Metrics::new(size, line));
        buf.set_size(Some(width), None);
        let family = self.family.clone();
        buf.set_text(s, &Attrs::new().family(Family::Name(&family)), Shaping::Advanced, None);
        buf.shape_until_scroll(&mut self.fonts, false);
        let mut height = 0.0f32;
        let runs: Vec<(f32, Vec<cosmic_text::LayoutGlyph>)> = buf.layout_runs().map(|r| { height = height.max(r.line_top + r.line_height); (r.line_y, r.glyphs.to_vec()) }).collect();
        for (line_y, glyphs) in runs {
            for g in glyphs {
                let pg = g.physical((0.0, 0.0), scale);
                let Some(slot) = self.slot(pg.cache_key) else { continue };
                let x = (origin.0 * scale).round() + pg.x as f32 + slot.left as f32 - SPREAD as f32;
                let y = ((origin.1 + line_y) * scale).round() + pg.y as f32 - slot.top as f32 - SPREAD as f32;
                let a = ATLAS as f32;
                out.push(GlyphInst {
                    rect: [x, y, slot.w as f32, slot.h as f32],
                    uv: [slot.x as f32 / a, slot.y as f32 / a, (slot.x + slot.w) as f32 / a, (slot.y + slot.h) as f32 / a],
                    tone: [tone, tone, tone, 1.0],
                });
            }
        }
        height
    }

    fn slot(&mut self, key: CacheKey) -> Option<Slot> {
        if let Some(s) = self.slots.get(&key) { return *s; }
        let made = self.rasterise(key);
        self.slots.insert(key, made);
        made
    }

    fn rasterise(&mut self, key: CacheKey) -> Option<Slot> {
        let img = self.cache.get_image_uncached(&mut self.fonts, key)?;
        if img.placement.width == 0 || img.placement.height == 0 || !matches!(img.content, SwashContent::Mask) { return None; }
        let (gw, gh) = (img.placement.width as i32, img.placement.height as i32);
        let (w, h) = ((gw + SPREAD * 2) as u32, (gh + SPREAD * 2) as u32);
        if self.pen.0 + w + 1 > ATLAS { self.pen = (1, self.pen.1 + self.pen.2 + 1, 0); }
        if self.pen.1 + h + 1 > ATLAS { return None; } // atlas full
        let (ax, ay) = (self.pen.0, self.pen.1);
        self.pen.0 += w + 1; self.pen.2 = self.pen.2.max(h);

        // Coverage, padded.
        let n = (w * h) as usize;
        let mut cov = vec![0u8; n];
        for y in 0..gh { for x in 0..gw {
            cov[((y + SPREAD) as u32 * w + (x + SPREAD) as u32) as usize] = img.data[(y * gw + x) as usize];
        } }
        let near = nearness(&cov, w as usize, h as usize);
        for y in 0..h { for x in 0..w {
            let i = (y * w + x) as usize; let o = (((ay + y) * ATLAS + ax + x) * 2) as usize;
            self.pixels[o] = cov[i]; self.pixels[o + 1] = near[i];
        } }
        self.dirty = true;
        Some(Slot { x: ax, y: ay, w, h, left: img.placement.left, top: img.placement.top })
    }
}

/// Exact Euclidean distance to the nearest inked texel (Felzenszwalb & Huttenlocher), mapped to 255 at
/// ink and 0 at `SPREAD` or further.
fn nearness(cov: &[u8], w: usize, h: usize) -> Vec<u8> {
    const INF: f32 = 1e20;
    let mut d: Vec<f32> = cov.iter().map(|&c| if c >= 128 { 0.0 } else { INF }).collect();
    let m = w.max(h);
    let (mut f, mut out, mut v, mut z) = (vec![0.0f32; m], vec![0.0f32; m], vec![0usize; m], vec![0.0f32; m + 1]);
    let pass = |f: &[f32], n: usize, out: &mut [f32], v: &mut [usize], z: &mut [f32]| {
        let mut k = 0usize; v[0] = 0; z[0] = -INF; z[1] = INF;
        for q in 1..n {
            loop {
                let p = v[k];
                let s = ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32)) / (2.0 * q as f32 - 2.0 * p as f32);
                if s <= z[k] && k > 0 { k -= 1; } else { k += 1; v[k] = q; z[k] = s; z[k + 1] = INF; break; }
            }
        }
        k = 0;
        for q in 0..n {
            while z[k + 1] < q as f32 { k += 1; }
            let p = v[k]; out[q] = (q as f32 - p as f32).powi(2) + f[p];
        }
    };
    for x in 0..w {
        for y in 0..h { f[y] = d[y * w + x]; }
        pass(&f, h, &mut out, &mut v, &mut z);
        for y in 0..h { d[y * w + x] = out[y]; }
    }
    for y in 0..h {
        f[..w].copy_from_slice(&d[y * w..y * w + w]);
        pass(&f, w, &mut out, &mut v, &mut z);
        d[y * w..y * w + w].copy_from_slice(&out[..w]);
    }
    d.iter().map(|&q| ((1.0 - q.sqrt() / SPREAD as f32).clamp(0.0, 1.0) * 255.0) as u8).collect()
}
