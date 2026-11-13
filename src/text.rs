//! Text: shaping and line breaking by parley; every glyph is then kept as its own instance.
//! Each glyph is rasterised once into an atlas that stores two things per texel: coverage, and how
//! close that texel is to the nearest ink (255 at ink, falling to 0 at `SPREAD` pixels away).
use crate::scene::{GLYPH, Instance};
use parley::fontique::{Blob, Collection, CollectionOptions};
use parley::layout::PositionedLayoutItem;
use parley::style::{FontFamily, StyleProperty};
use parley::{FontContext, Layout, LayoutContext, LineHeight};
use std::collections::HashMap;
use std::sync::Arc;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format, Vector};

pub const SPREAD: i32 = 12;
pub const ATLAS: u32 = 2048;

/// A paragraph to draw: text, font size, line height and the width it was laid out at (logical px).
#[derive(Clone, Copy)]
pub struct Para<'a> { pub s: &'a str, pub size: f32, pub line: f32, pub width: f32 }

#[derive(Clone, Copy)]
struct Slot { x: u32, y: u32, w: u32, h: u32, left: i32, top: i32 }

pub struct Text {
    fonts: FontContext,
    layouts: LayoutContext<()>,
    scaler: ScaleContext,
    family: String,
    slots: HashMap<(u64, u32, u32, u8), Option<Slot>>,
    /// Layout asks for the same paragraph at the same width many times; keyed by text, size bits, width bits.
    measured: HashMap<(String, u32, u32, u32), (f32, f32)>,
    /// RG8: coverage, nearness-to-ink.
    pub pixels: Vec<u8>,
    pub dirty: bool,
    pen: (u32, u32, u32),
}

impl Text {
    pub fn new(font: Vec<u8>) -> Self {
        let mut collection = Collection::new(CollectionOptions { shared: false, system_fonts: false });
        let reg = collection.register_fonts(Blob::new(Arc::new(font)), None);
        let family = reg.first().and_then(|(id, _)| collection.family_name(*id).map(str::to_owned)).unwrap_or_default();
        Self {
            fonts: FontContext { collection, source_cache: Default::default() },
            layouts: LayoutContext::new(), scaler: ScaleContext::new(), family,
            slots: HashMap::new(), measured: HashMap::new(), pixels: vec![0; (ATLAS * ATLAS * 2) as usize], dirty: true, pen: (1, 1, 0),
        }
    }

    /// Shape and break `s` at `size` logical px. Measuring and drawing use this same call, so the
    /// line breaks on screen are exactly the ones layout planned for.
    fn layout(&mut self, s: &str, size: f32, line: f32, max_width: Option<f32>) -> Layout<()> {
        let mut b = self.layouts.ranged_builder(&mut self.fonts, s, 1.0, true);
        b.push_default(StyleProperty::FontFamily(FontFamily::named(&self.family)));
        b.push_default(StyleProperty::FontSize(size));
        b.push_default(LineHeight::Absolute(line));
        let mut l: Layout<()> = b.build(s);
        l.break_all_lines(max_width);
        l
    }

    /// Width and height of a paragraph in logical px when limited to `max_width`.
    pub fn measure(&mut self, s: &str, size: f32, line: f32, max_width: Option<f32>) -> (f32, f32) {
        let k = (s.to_owned(), size.to_bits(), line.to_bits(), max_width.unwrap_or(-1.0).to_bits());
        if let Some(v) = self.measured.get(&k) { return *v; }
        if self.measured.len() > 4096 { self.measured.clear(); }
        let l = self.layout(s, size, line, max_width);
        let v = (l.width().ceil(), l.height().ceil());
        self.measured.insert(k, v);
        v
    }

    /// Call `out` once per visible glyph with its instance (device px) and glyph id. `origin` is
    /// logical px.
    pub fn emit(&mut self, p: Para<'_>, origin: (f32, f32), proto: Instance, scale: f32, mut out: impl FnMut(Instance, u32)) {
        let l = self.layout(p.s, p.size, p.line, Some(p.width));
        let (ox, oy) = (origin.0 * scale, origin.1 * scale);
        for ln in l.lines() {
            for item in ln.items() {
                let PositionedLayoutItem::GlyphRun(gr) = item else { continue };
                let run = gr.run();
                let font = run.font();
                let Some(fref) = swash::FontRef::from_index(font.data.as_ref(), font.index as usize) else { continue };
                let fsize = run.font_size() * scale;
                let coords: Vec<i16> = run.normalized_coords().iter().map(|c| c.to_bits()).collect();
                let mut scaler = self.scaler.builder(fref).size(fsize).hint(false).normalized_coords(coords.iter().copied()).build();
                for g in gr.positioned_glyphs() {
                    let (gx, gy) = (ox + g.x * scale, (oy + g.y * scale).round());
                    let sub = ((gx.fract() * 4.0) as u8).min(3);
                    let key = (font.data.id(), g.id, fsize.to_bits(), sub);
                    let slot = match self.slots.get(&key) {
                        Some(s) => *s,
                        None => {
                            let img = Render::new(&[Source::Outline]).format(Format::Alpha).offset(Vector::new(sub as f32 / 4.0, 0.0)).render(&mut scaler, g.id as _);
                            let made = img.and_then(|i| store(&mut self.pixels, &mut self.pen, &i.data, i.placement.width, i.placement.height, i.placement.left, i.placement.top));
                            self.dirty |= made.is_some();
                            self.slots.insert(key, made);
                            made
                        }
                    };
                    let Some(sl) = slot else { continue };
                    let a = ATLAS as f32;
                    let mut inst = proto;
                    inst.rect = [gx.floor() + sl.left as f32 - SPREAD as f32, gy - sl.top as f32 - SPREAD as f32, sl.w as f32, sl.h as f32];
                    inst.uv = [sl.x as f32 / a, sl.y as f32 / a, (sl.x + sl.w) as f32 / a, (sl.y + sl.h) as f32 / a];
                    inst.paint[3] = GLYPH;
                    out(inst, g.id);
                }
            }
        }
    }
}

/// Put one glyph image, padded by the spread, into the atlas. `None` when it is empty or the atlas is
/// full (the glyph is then left out rather than drawn wrong).
fn store(pixels: &mut [u8], pen: &mut (u32, u32, u32), data: &[u8], gw: u32, gh: u32, left: i32, top: i32) -> Option<Slot> {
    if gw == 0 || gh == 0 { return None; }
    let sp = SPREAD as u32;
    let (w, h) = (gw + sp * 2, gh + sp * 2);
    if pen.0 + w + 1 > ATLAS { *pen = (1, pen.1 + pen.2 + 1, 0); }
    if pen.1 + h + 1 > ATLAS { return None; }
    let (ax, ay) = (pen.0, pen.1);
    pen.0 += w + 1; pen.2 = pen.2.max(h);
    let mut cov = vec![0u8; (w * h) as usize];
    for y in 0..gh { for x in 0..gw { cov[((y + sp) * w + x + sp) as usize] = data[(y * gw + x) as usize]; } }
    let near = nearness(&cov, w as usize, h as usize);
    for y in 0..h { for x in 0..w {
        let i = (y * w + x) as usize; let o = (((ay + y) * ATLAS + ax + x) * 2) as usize;
        pixels[o] = cov[i]; pixels[o + 1] = near[i];
    } }
    Some(Slot { x: ax, y: ay, w, h, left, top })
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
        for (q, o) in out.iter_mut().enumerate().take(n) {
            while z[k + 1] < q as f32 { k += 1; }
            let p = v[k]; *o = (q as f32 - p as f32).powi(2) + f[p];
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
