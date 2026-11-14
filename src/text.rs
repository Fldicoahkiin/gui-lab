//! Text: shaping and line breaking by parley; every glyph is then kept as its own instance.
//! Each glyph is rasterised once into an atlas that stores two things per texel: coverage, and how
//! close that texel is to the glyph's outline, signed (128 on it, 255 at `SPREAD` pixels inside, 0 at
//! `SPREAD` pixels outside).
//!
//! Several font faces can be registered; each text element picks one. A face can be marked as a pixel
//! face: it is then drawn only at whole multiples of its native size, on whole device pixels, with
//! coverage cut at one half, so its edges are ink or ground and nothing in between.
use crate::scene::{GLYPH, Instance};
use crate::wrap::Around;
use parley::fontique::{Blob, Collection, CollectionOptions};
use parley::layout::PositionedLayoutItem;
use parley::style::{FontFamily, FontFamilyName, StyleProperty};
use parley::{FontContext, Layout, LayoutContext, LineHeight};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format, Vector};

pub const SPREAD: i32 = 12;
pub const ATLAS: u32 = 2048;

/// The faces that ship in `assets/`, in the order `bundled` registers them.
pub const SANS: u8 = 0;
pub const PIXEL: u8 = 1;
pub const MONO: u8 = 2;
pub const SANS_FILE: &str = "NotoSansSC-Regular.otf";
pub const PIXEL_FILE: &str = "FusionPixel-12px-Proportional-zh_hans.otf";
pub const MONO_FILE: &str = "DepartureMono-Regular.otf";

/// A font file to register. `pixel` is the native size (px) of a pixel face.
pub struct Face { pub data: Vec<u8>, pub pixel: Option<f32> }
impl Face {
    pub fn outline(data: Vec<u8>) -> Self { Self { data, pixel: None } }
    pub fn pixel(data: Vec<u8>, size: f32) -> Self { Self { data, pixel: Some(size) } }
}
/// Noto Sans SC, Fusion Pixel 12 px, Departure Mono (11 px), as `SANS`, `PIXEL`, `MONO`.
pub fn bundled(sans: Vec<u8>, pixel: Vec<u8>, mono: Vec<u8>) -> Vec<Face> {
    vec![Face::outline(sans), Face::pixel(pixel, 12.0), Face::pixel(mono, 11.0)]
}
/// Read the bundled faces from a directory (native only; the web page passes the bytes in).
#[cfg(not(target_arch = "wasm32"))]
pub fn load_bundled(dir: &str) -> std::io::Result<Vec<Face>> {
    let r = |f: &str| std::fs::read(format!("{dir}/{f}"));
    Ok(bundled(r(SANS_FILE)?, r(PIXEL_FILE)?, r(MONO_FILE)?))
}

/// A paragraph to draw: text, face, font size, line height and the width it was laid out at
/// (logical px; infinite for text kept on one line).
#[derive(Clone, Copy)]
pub struct Para<'a> { pub s: &'a str, pub face: u8, pub size: f32, pub line: f32, pub width: f32, pub print: f32, pub around: Option<&'a Around> }

#[derive(Clone, Copy)]
struct Slot { x: u32, y: u32, w: u32, h: u32, left: i32, top: i32 }

/// Text, face, size bits, line bits, width bits, what it flows around.
type MeasureKey = (String, u8, u32, u32, u32, u64);

struct Family { name: String, pixel: Option<f32>, data: Arc<Vec<u8>>, blob: u64 }

/// One glyph rasterised on its own (for the mark atlas): coverage rows, placement, and the face's
/// advance and line box at that size, px.
pub struct Raster { pub data: Vec<u8>, pub w: u32, pub h: u32, pub left: i32, pub top: i32, pub advance: f32, pub ascent: f32, pub descent: f32 }

pub struct Text {
    fonts: FontContext,
    layouts: LayoutContext<()>,
    scaler: ScaleContext,
    families: Vec<Family>,
    /// Device px per logical px: pixel faces snap their size in device px.
    scale: f32,
    slots: HashMap<(u64, u32, u32, u8), Option<Slot>>,
    /// Layout asks for the same paragraph at the same width many times; keyed by text, face, size bits,
    /// line bits, width bits.
    measured: HashMap<MeasureKey, (f32, f32)>,
    /// Shaped and broken paragraphs by text, face, size, line height, width and obstacles: this
    /// generation's and the last one's (a paragraph not used for a whole generation is dropped).
    cache: [HashMap<MeasureKey, Rc<Layout<()>>>; 2],
    /// RG8: coverage, nearness-to-ink.
    pub pixels: Vec<u8>,
    pub dirty: bool,
    pen: (u32, u32, u32),
    /// Paragraphs shaped and broken by parley so far (cache misses), for the work counters.
    shaped: u64,
}

impl Text {
    pub fn new(faces: Vec<Face>) -> Self {
        let mut collection = Collection::new(CollectionOptions { shared: false, system_fonts: false });
        let families = faces.into_iter().map(|f| {
            let data = Arc::new(f.data);
            let blob = Blob::new(data.clone());
            let id = blob.id();
            let reg = collection.register_fonts(blob, None);
            let name = reg.first().and_then(|(id, _)| collection.family_name(*id).map(str::to_owned)).unwrap_or_default();
            Family { name, pixel: f.pixel, data, blob: id }
        }).collect();
        Self {
            fonts: FontContext { collection, source_cache: Default::default() },
            layouts: LayoutContext::new(), scaler: ScaleContext::new(), families, scale: 1.0,
            slots: HashMap::new(), measured: HashMap::new(), cache: [HashMap::new(), HashMap::new()], pixels: vec![0; (ATLAS * ATLAS * 2) as usize], dirty: true, pen: (1, 1, 0), shaped: 0,
        }
    }

    pub fn faces(&self) -> usize { self.families.len() }
    /// How many paragraphs have been shaped (parley layouts built) so far.
    pub fn shaped(&self) -> u64 { self.shaped }
    /// How much of the glyph atlas is used, 0..1 (by rows).
    pub fn atlas_use(&self) -> f32 { ((self.pen.1 + self.pen.2) as f32 / ATLAS as f32).min(1.0) }
    pub fn is_pixel(&self, face: u8) -> bool { self.family(face).pixel.is_some() }
    /// Native size of a pixel face, px.
    pub fn native(&self, face: u8) -> Option<f32> { self.family(face).pixel }
    fn family(&self, face: u8) -> &Family { &self.families[(face as usize).min(self.families.len() - 1)] }

    pub fn set_scale(&mut self, scale: f32) {
        if scale != self.scale { self.scale = scale; self.measured.clear(); self.cache = [HashMap::new(), HashMap::new()]; }
    }

    /// The logical size a face is really drawn at: pixel faces go to the nearest whole multiple (at
    /// least one) of their native size in device px, since anything between would smear their pixels.
    pub fn size_for(&self, face: u8, size: f32) -> f32 {
        match self.family(face).pixel {
            Some(native) => ((size * self.scale / native).round().max(1.0) * native) / self.scale,
            None => size,
        }
    }

    /// The paragraph shaped and broken, from the cache when it was laid out before (same text, face,
    /// size, line height, width and obstacles); only a miss shapes (and counts in `shaped`).
    fn layout(&mut self, s: &str, face: u8, size: f32, line: f32, max_width: Option<f32>, around: Option<&Around>) -> Rc<Layout<()>> {
        let max_width = max_width.filter(|w| w.is_finite());
        let k = (s.to_owned(), face, size.to_bits(), line.to_bits(), max_width.unwrap_or(-1.0).to_bits(), around.map_or(0, Around::id));
        if let Some(l) = self.cache[0].get(&k) { return l.clone(); }
        if let Some(l) = self.cache[1].remove(&k) { self.cache[0].insert(k, l.clone()); return l; }
        if self.cache[0].len() >= 2048 { self.cache.swap(0, 1); self.cache[0].clear(); }
        let l = Rc::new(self.shape(s, face, size, line, max_width, around));
        self.cache[0].insert(k, l.clone());
        l
    }
    /// Shape and break `s` at `size` logical px. Measuring and drawing use this same layout, so the
    /// line breaks on screen are exactly the ones layout planned for.
    fn shape(&mut self, s: &str, face: u8, size: f32, line: f32, max_width: Option<f32>, around: Option<&Around>) -> Layout<()> {
        self.shaped += 1;
        let size = self.size_for(face, size);
        let fi = (face as usize).min(self.families.len() - 1);
        let mut b = self.layouts.ranged_builder(&mut self.fonts, s, 1.0, true);
        // The first registered face (the proportional one, with CJK) backs up any face that lacks a
        // character, so a monospaced face can still show Chinese.
        let stack = [FontFamilyName::named(&self.families[fi].name), FontFamilyName::named(&self.families[0].name)];
        b.push_default(StyleProperty::FontFamily(FontFamily::List(std::borrow::Cow::Borrowed(&stack))));
        b.push_default(StyleProperty::FontSize(size));
        b.push_default(LineHeight::Absolute(line));
        let mut l: Layout<()> = b.build(s);
        match around {
            None => l.break_all_lines(max_width),
            Some(a) => flow_around(&mut l, a, max_width.unwrap_or(1e5), line),
        }
        l
    }

    /// Width and height of a paragraph in logical px when limited to `max_width`.
    pub fn measure(&mut self, s: &str, face: u8, size: f32, line: f32, max_width: Option<f32>) -> (f32, f32) { self.measure_around(s, face, size, line, max_width, None) }
    /// The same, flowing around obstacles: the width is the whole column's, the height reaches the last line.
    pub fn measure_around(&mut self, s: &str, face: u8, size: f32, line: f32, max_width: Option<f32>, around: Option<&Around>) -> (f32, f32) {
        let k = (s.to_owned(), face, size.to_bits(), line.to_bits(), max_width.unwrap_or(-1.0).to_bits(), around.map_or(0, Around::id));
        if let Some(v) = self.measured.get(&k) { return *v; }
        if self.measured.len() > 4096 { self.measured.clear(); }
        let l = self.layout(s, face, size, line, max_width, around);
        let v = match around {
            None => (l.width().ceil(), l.height().ceil()),
            Some(_) => (max_width.unwrap_or(l.width()).ceil(), l.lines().map(|ln| ln.metrics().block_max_coord).fold(0.0f32, f32::max).ceil()),
        };
        self.measured.insert(k, v);
        v
    }

    /// Where a caret at byte `index` of a paragraph goes: x, top and height, logical px. `origin` is
    /// where the paragraph is drawn.
    pub fn caret(&mut self, p: Para<'_>, origin: (f32, f32), index: usize) -> Option<crate::ui::Rect> {
        let l = self.layout(p.s, p.face, p.size, p.line, Some(p.width), p.around);
        let at = index.min(p.s.len());
        let b = parley::Cursor::from_byte_index(&l, at, parley::Affinity::Downstream).geometry(&l, 1.0);
        let h = (b.y1 - b.y0) as f32;
        (h > 0.0).then(|| crate::ui::Rect::new(origin.0 + b.x0 as f32, origin.1 + b.y0 as f32, 1.0, h))
    }
    /// The byte index (on a cluster boundary) nearest to the point (x, y), logical px; `origin` is where
    /// the paragraph is drawn. A point beyond the text goes to the nearest line's end.
    pub fn hit(&mut self, p: Para<'_>, origin: (f32, f32), x: f32, y: f32) -> usize {
        let l = self.layout(p.s, p.face, p.size, p.line, Some(p.width), p.around);
        parley::Cursor::from_point(&l, x - origin.0, y - origin.1).index().min(p.s.len())
    }
    /// The boxes (one per line, logical px) that bytes `a..b` cover: where a selection is.
    pub fn boxes(&mut self, p: Para<'_>, origin: (f32, f32), a: usize, b: usize) -> Vec<crate::ui::Rect> {
        let l = self.layout(p.s, p.face, p.size, p.line, Some(p.width), p.around);
        let (a, b) = (a.min(p.s.len()), b.min(p.s.len()));
        if a >= b { return vec![]; }
        let sel = parley::Selection::new(parley::Cursor::from_byte_index(&l, a, parley::Affinity::Downstream), parley::Cursor::from_byte_index(&l, b, parley::Affinity::Upstream));
        sel.geometry(&l).into_iter().map(|(bb, _)| crate::ui::Rect::new(origin.0 + bb.x0 as f32, origin.1 + bb.y0 as f32, (bb.x1 - bb.x0) as f32, (bb.y1 - bb.y0) as f32)).collect()
    }
    /// Where the caret goes from byte `at` by the layout: a line up or down (keeping its x), or the start
    /// or end of its visual line.
    pub fn nav(&mut self, p: Para<'_>, at: usize, to: Nav) -> usize {
        let l = self.layout(p.s, p.face, p.size, p.line, Some(p.width), p.around);
        let s = parley::Selection::from_byte_index(&l, at.min(p.s.len()), parley::Affinity::Downstream);
        let s = match to { Nav::Up => s.previous_line(&l, false), Nav::Down => s.next_line(&l, false), Nav::LineStart => s.line_start(&l, false), Nav::LineEnd => s.line_end(&l, false) };
        s.focus().index().min(p.s.len())
    }

    /// Rasterise one character of a face at `px` device px, outside any layout (for the mark atlas).
    /// Pixel faces are cut at half coverage as they are on the page.
    pub fn raster(&mut self, face: u8, ch: char, px: f32) -> Option<Raster> {
        let fam = self.family(face);
        let (data, pixel) = (fam.data.clone(), fam.pixel.is_some());
        let fref = swash::FontRef::from_index(&data, 0)?;
        let gid = fref.charmap().map(ch);
        let m = fref.metrics(&[]).scale(px);
        let advance = fref.glyph_metrics(&[]).scale(px).advance_width(gid);
        let mut scaler = self.scaler.builder(fref).size(px).hint(false).build();
        let img = Render::new(&[Source::Outline]).format(Format::Alpha).render(&mut scaler, gid)?;
        let mut data = img.data;
        if pixel { for c in &mut data { *c = if *c >= 128 { 255 } else { 0 }; } }
        Some(Raster { data, w: img.placement.width, h: img.placement.height, left: img.placement.left, top: img.placement.top, advance, ascent: m.ascent, descent: m.descent })
    }

    /// Call `out` once per visible glyph with its instance (device px) and glyph id. `origin` is
    /// logical px.
    pub fn emit(&mut self, p: Para<'_>, origin: (f32, f32), proto: Instance, scale: f32, out: impl FnMut(Instance, u32)) {
        self.emit_in(p, origin, proto, scale, None, out)
    }
    /// The same, leaving out lines entirely outside `rows` (device px, top and bottom): what a clip
    /// hides is not even made into instances.
    pub fn emit_in(&mut self, p: Para<'_>, origin: (f32, f32), proto: Instance, scale: f32, rows: Option<(f32, f32)>, mut out: impl FnMut(Instance, u32)) {
        let l = self.layout(p.s, p.face, p.size, p.line, Some(p.width), p.around);
        let para_pixel = self.is_pixel(p.face);
        // A pixel face starts on a whole device pixel so its pixels land on the screen's.
        let (ox, oy) = if para_pixel { ((origin.0 * scale).round(), (origin.1 * scale).round()) } else { (origin.0 * scale, origin.1 * scale) };
        for ln in l.lines() {
            if let Some((top, bottom)) = rows {
                let m = ln.metrics();
                let (y0, y1) = (oy + m.block_min_coord * scale, oy + m.block_max_coord * scale);
                // A glyph's ink can reach a little past its line box, and the spread pads it further.
                let pad = (SPREAD as f32 + p.size) * scale;
                if y1 + pad < top || y0 - pad > bottom { continue; }
            }
            for item in ln.items() {
                let PositionedLayoutItem::GlyphRun(gr) = item else { continue };
                let run = gr.run();
                let font = run.font();
                let Some(fref) = swash::FontRef::from_index(font.data.as_ref(), font.index as usize) else { continue };
                // Pixel treatment follows the face a run is really set in: a fallback outline face inside
                // pixel text is drawn as an outline.
                let pixel = self.families.iter().any(|f| f.pixel.is_some() && f.blob == font.data.id());
                let fsize = run.font_size() * scale;
                let coords: Vec<i16> = run.normalized_coords().iter().map(|c| c.to_bits()).collect();
                let mut scaler = self.scaler.builder(fref).size(fsize).hint(false).normalized_coords(coords.iter().copied()).build();
                for g in gr.positioned_glyphs() {
                    let (gx, gy) = (ox + g.x * scale, (oy + g.y * scale).round());
                    let gx = if pixel { gx.round() } else { gx };
                    let sub = if pixel { 0 } else { ((gx.fract() * 4.0) as u8).min(3) };
                    let key = (font.data.id(), g.id, fsize.to_bits(), sub);
                    let slot = match self.slots.get(&key) {
                        Some(s) => *s,
                        None => {
                            let mut img = Render::new(&[Source::Outline]).format(Format::Alpha).offset(Vector::new(sub as f32 / 4.0, 0.0)).render(&mut scaler, g.id as _);
                            if pixel && let Some(i) = img.as_mut() { for c in &mut i.data { *c = if *c >= 128 { 255 } else { 0 }; } }
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
                    inst.extra[1] = p.print;
                    out(inst, g.id);
                }
            }
        }
    }
}

/// Where `Text::nav` moves the caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav { Up, Down, LineStart, LineEnd }

/// The grapheme boundary before byte `at` (0 at the start).
pub fn prev_grapheme(s: &str, at: usize) -> usize { s.grapheme_indices(true).map(|(i, _)| i).take_while(|&i| i < at).last().unwrap_or(0) }
/// The grapheme boundary after byte `at` (the end at the end).
pub fn next_grapheme(s: &str, at: usize) -> usize { s.grapheme_indices(true).map(|(i, g)| i + g.len()).find(|&e| e > at).unwrap_or(s.len()) }
/// Is this piece a word (letters, digits, or an ideograph) rather than spaces or punctuation?
fn wordy(w: &str) -> bool { w.chars().any(char::is_alphanumeric) }
/// The start of the word before byte `at` (skipping spaces and punctuation first). Each CJK ideograph
/// is a word of its own (UAX #29).
pub fn prev_word(s: &str, at: usize) -> usize {
    s.split_word_bound_indices().filter(|(i, w)| *i < at && wordy(w)).map(|(i, _)| i).last().unwrap_or(0)
}
/// The end of the word after byte `at`.
pub fn next_word(s: &str, at: usize) -> usize {
    s.split_word_bound_indices().map(|(i, w)| (i + w.len(), w)).find(|(e, w)| *e > at && wordy(w)).map_or(s.len(), |(e, _)| e)
}
/// The word (or run of spaces, or punctuation) that byte `at` is in: what a double click selects.
pub fn word_range(s: &str, at: usize) -> (usize, usize) {
    let at = at.min(s.len());
    let mut last = (s.len(), s.len());
    for (i, w) in s.split_word_bound_indices() {
        if at >= i && at < i + w.len() { return (i, i + w.len()); }
        last = (i, i + w.len());
    }
    last
}

/// Break `l` into lines that keep out of the obstacles in `a`: down the column a line height at a
/// time, each band's free pieces (left to right) take a line each; a band with no free piece is
/// skipped a few px at a time.
fn flow_around(l: &mut Layout<()>, a: &Around, width: f32, line: f32) {
    let mut br = l.break_lines();
    br.state_mut().set_layout_max_advance(width);
    let mut y = 0.0f32;
    let mut pieces: std::collections::VecDeque<(f32, f32)> = Default::default();
    let mut guard = 0;
    while !br.is_done() && guard < 100_000 {
        guard += 1;
        if pieces.is_empty() {
            pieces = a.free(width, y, y + line).into();
            if pieces.is_empty() { y += 4.0; continue; }
        }
        let (x0, x1) = pieces.pop_front().expect("a free piece");
        let st = br.state_mut();
        st.set_line_x(x0);
        st.set_line_max_advance((x1 - x0).min(width));
        st.set_line_y(y as f64);
        if br.break_next().is_none() { break; }
        if pieces.is_empty() { y += line; }
    }
    br.finish();
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

/// Signed nearness to the glyph's outline: 128 on the outline, rising to 255 at `SPREAD` pixels inside
/// the ink and falling to 0 at `SPREAD` pixels outside (exact Euclidean distances, Felzenszwalb &
/// Huttenlocher, half a texel from the edge of the inked texels).
fn nearness(cov: &[u8], w: usize, h: usize) -> Vec<u8> {
    let outside = edt(cov, w, h, |c| c >= 128);
    let inside = edt(cov, w, h, |c| c < 128);
    cov.iter().enumerate().map(|(i, &c)| {
        let sd = if c >= 128 { inside[i].sqrt() - 0.5 } else { 0.5 - outside[i].sqrt() };
        ((0.5 + 0.5 * sd / SPREAD as f32).clamp(0.0, 1.0) * 255.0).round() as u8
    }).collect()
}

/// Squared distance from every texel to the nearest texel where `seed` holds.
fn edt(cov: &[u8], w: usize, h: usize, seed: impl Fn(u8) -> bool) -> Vec<f32> {
    const INF: f32 = 1e20;
    let mut d: Vec<f32> = cov.iter().map(|&c| if seed(c) { 0.0 } else { INF }).collect();
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
    d
}
