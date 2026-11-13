//! Off-screen rendering for the native tools (`shot`, `sheet`, `check`): no window, no surface, the
//! fallback adapter when there is no GPU, and pixels read back to the CPU.
use crate::render::Renderer;
use crate::style::Look;
use crate::ui::Ui;

pub struct Offscreen { pub r: Renderer, target: wgpu::Texture, buf: wgpu::Buffer, pub w: u32, pub h: u32, row: u32 }

impl Offscreen {
    /// Device px. Registers every demo picture.
    pub fn new(w: u32, h: u32) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { force_fallback_adapter: true, ..Default::default() })).expect("no adapter");
        eprintln!("adapter: {:?}", adapter.get_info().name);
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).expect("device");
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, view_formats: &[],
        });
        let row = (w * 4).div_ceil(256) * 256;
        let buf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut r = Renderer::new(device, queue, format, w, h);
        for (id, pw, ph, px) in crate::demo::pictures() { r.set_picture(id, pw, ph, &px); }
        Self { r, target, buf, w, h, row }
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

pub fn save_png(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap_or_else(|e| panic!("create {path}: {e}"));
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().and_then(|mut wr| wr.write_image_data(rgba)).expect("write png");
}
