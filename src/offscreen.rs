//! Off-screen rendering for the native tools (`shot`, `sheet`, `check`): no window, no surface, the
//! fallback adapter when there is no GPU, and pixels read back to the CPU.
use crate::render::Renderer;
use crate::style::Look;
use crate::ui::Ui;
use crate::error::SurfaceError;

/// The one GPU every `Offscreen` in the process draws with: an instance, adapter and device made once
/// and shared (each `Offscreen` keeps its own renderer and target). Making and dropping Vulkan
/// instances from several threads at once crashes some drivers (lavapipe under parallel tests).
struct Gpu { _instance: wgpu::Instance, device: wgpu::Device, queue: wgpu::Queue, tier: crate::render::ParticleTier }

/// The shared GPU, made on first use; why it could not start (`None`: no adapter) is kept too.
fn gpu() -> Result<&'static Gpu, SurfaceError> {
    static GPU: std::sync::OnceLock<Result<Gpu, Option<String>>> = std::sync::OnceLock::new();
    let made = GPU.get_or_init(|| {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { force_fallback_adapter: true, ..Default::default() })).map_err(|_| None)?;
        if std::env::var_os("AMENUI_QUIET").is_none() { eprintln!("adapter: {:?}", adapter.get_info().name); }
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).map_err(|e| Some(e.to_string()))?;
        Ok(Gpu { tier: crate::render::particle_tier(&adapter), _instance: instance, device, queue })
    });
    made.as_ref().map_err(|e| e.as_ref().map_or(SurfaceError::NoAdapter, |m| SurfaceError::Device(m.clone())))
}

pub struct Offscreen { pub r: Renderer, target: wgpu::Texture, buf: wgpu::Buffer, pub w: u32, pub h: u32, row: u32 }

impl Offscreen {
    /// Device px. Registers every sample picture (`pictures`). Panics with `SurfaceError`'s message when it cannot start;
    /// `try_new` returns it instead.
    pub fn new(w: u32, h: u32) -> Self { Self::try_new(w, h).unwrap_or_else(|e| panic!("{e}")) }
    /// Device px. Registers every sample picture (`pictures`).
    pub fn try_new(w: u32, h: u32) -> Result<Self, SurfaceError> { Self::make(w, h, false) }
    /// Like `try_new`, with a renderer that warms up (`Renderer::begin`): it draws as plain until
    /// `r.warm_step` has made the rest.
    pub fn try_warming(w: u32, h: u32) -> Result<Self, SurfaceError> { Self::make(w, h, true) }
    fn make(w: u32, h: u32, warming: bool) -> Result<Self, SurfaceError> {
        if w == 0 || h == 0 { return Err(SurfaceError::ZeroSize { w, h }); }
        let Gpu { device, queue, tier, .. } = gpu()?;
        let (device, queue) = (device.clone(), queue.clone());
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, view_formats: &[],
        });
        let row = (w * 4).div_ceil(256) * 256;
        let buf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut r = if warming { Renderer::begin(device, queue, format, w, h).into_renderer() } else { Renderer::new(device, queue, format, w, h) };
        r.set_particle_tier(*tier);
        // Frames drawn offscreen are checks and stills: the stages' scale stays where the look puts it.
        r.govern = false;
        // On the CPU rasteriser offscreen draws with, compiling a material's own pass costs more than it saves.
        r.specialise = None;
        for (id, pw, ph, px) in crate::pictures::pictures() { r.set_picture(id, pw, ph, &px); }
        Ok(Self { r, target, buf, w, h, row })
    }

    /// Draw and wait for the GPU, without reading back (for timing).
    pub fn draw<S, M: Clone>(&mut self, ui: &mut Ui<S, M>, look: &Look) {
        let view = self.target.create_view(&Default::default());
        self.r.draw(&view, ui, look);
        self.r.device().poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    }

    /// Draw and read back: RGBA8 rows, `w` × `h`.
    pub fn render<S, M: Clone>(&mut self, ui: &mut Ui<S, M>, look: &Look) -> Vec<u8> {
        let view = self.target.create_view(&Default::default());
        self.r.draw(&view, ui, look);
        let (device, queue) = (self.r.device(), self.r.queue());
        let (w, h, row) = (self.w, self.h, self.row);
        let mut enc = device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &self.target, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &self.buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 });
        queue.submit([enc.finish()]);
        self.buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let data = self.buf.slice(..).get_mapped_range().expect("mapped");
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h { px.extend_from_slice(&data[(y * row) as usize..(y * row + w * 4) as usize]); }
        drop(data);
        self.buf.unmap();
        px
    }
}

/// Draw one frame of `ui` into a new texture of `format` (a window's surface format) and read it back as
/// RGBA8 rows, `w` × `h` (a window's `--shot`).
pub fn read_back<S, M: Clone>(r: &mut Renderer, ui: &mut Ui<S, M>, look: &Look, format: wgpu::TextureFormat, w: u32, h: u32) -> Result<Vec<u8>, String> {
    let device = r.device().clone();
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2, format, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    r.draw(&view, ui, look);
    let row = (w * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 });
    r.queue().submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
    let data = buf.slice(..).get_mapped_range().map_err(|e| format!("{e:?}"))?;
    let bgra = matches!(format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for p in data[(y * row) as usize..(y * row + w * 4) as usize].chunks(4) {
            if bgra { px.extend_from_slice(&[p[2], p[1], p[0], 255]); } else { px.extend_from_slice(&[p[0], p[1], p[2], 255]); }
        }
    }
    Ok(px)
}

/// The part of an RGBA8 frame `w` px wide inside device rect (x0, y0, x1, y1), clamped to the frame.
pub fn crop(px: &[u8], w: u32, h: u32, r: [u32; 4]) -> (u32, u32, Vec<u8>) {
    let (x0, y0, x1, y1) = (r[0].min(w), r[1].min(h), r[2].min(w), r[3].min(h));
    let (x1, y1) = (x1.max(x0), y1.max(y0));
    let mut out = Vec::with_capacity(((x1 - x0) * (y1 - y0) * 4) as usize);
    for y in y0..y1 { out.extend_from_slice(&px[((y * w + x0) * 4) as usize..((y * w + x1) * 4) as usize]); }
    (x1 - x0, y1 - y0, out)
}
/// Frames of one size side by side in rows of `cols`, on a mid-grey with 8 px gaps.
pub fn sheet(frames: &[(u32, u32, Vec<u8>)], cols: usize) -> (u32, u32, Vec<u8>) {
    let (fw, fh) = frames.first().map_or((1, 1), |f| (f.0, f.1));
    let rows = frames.len().div_ceil(cols.max(1));
    let (w, h) = (cols as u32 * (fw + 8) + 8, rows as u32 * (fh + 8) + 8);
    let mut px = vec![96u8; (w * h * 4) as usize];
    for p in px.chunks_mut(4) { p[3] = 255; }
    for (n, (_, _, f)) in frames.iter().enumerate() {
        let (ox, oy) = (8 + (n % cols) as u32 * (fw + 8), 8 + (n / cols) as u32 * (fh + 8));
        for y in 0..fh { let d = (((oy + y) * w + ox) * 4) as usize; px[d..d + (fw * 4) as usize].copy_from_slice(&f[(y * fw * 4) as usize..((y + 1) * fw * 4) as usize]); }
    }
    (w, h, px)
}

/// Encode RGBA8 rows as a PNG in memory (deterministic: same pixels, same bytes).
pub fn png_bytes(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        if let Ok(mut wr) = e.write_header() { let _ = wr.write_image_data(rgba); }
    }
    out
}

/// The same with text chunks (`tEXt`, Latin-1 keywords): the render tool puts the look's recipe in
/// `amenui-recipe`, so any image it wrote reproduces itself.
pub fn png_bytes_with(w: u32, h: u32, rgba: &[u8], texts: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        for (k, v) in texts { let _ = e.add_text_chunk((*k).to_owned(), (*v).to_owned()); }
        if let Ok(mut wr) = e.write_header() { let _ = wr.write_image_data(rgba); }
    }
    out
}
/// The `tEXt` chunk `key` of a PNG, if it has one.
pub fn png_text(png: &[u8], key: &str) -> Option<String> {
    let d = png::Decoder::new(std::io::Cursor::new(png));
    let r = d.read_info().ok()?;
    r.info().uncompressed_latin1_text.iter().find(|c| c.keyword == key).map(|c| c.text.clone())
}

pub fn save_png(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap_or_else(|e| panic!("create {path}: {e}"));
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().and_then(|mut wr| wr.write_image_data(rgba)).expect("write png");
}
