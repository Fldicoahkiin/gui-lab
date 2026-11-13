//! Render the demo off-screen to PNG files through a short scripted sequence. No window, no surface.
//! usage: shot <out_dir> <width> <height> <scale>     (device px)
use lab::demo::{self, DemoUi};
use lab::el::Key;
use lab::render::Renderer;

struct Shooter { r: Renderer, target: wgpu::Texture, buf: wgpu::Buffer, w: u32, h: u32, row: u32, out: String }

impl Shooter {
    fn shoot(&mut self, ui: &mut DemoUi, name: &str) {
        let view = self.target.create_view(&Default::default());
        let fx = ui.state.fx;
        self.r.draw(&view, ui, &fx);
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
        let file = std::fs::File::create(format!("{}/{name}.png", self.out)).expect("create png");
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().and_then(|mut wr| wr.write_image_data(&px)).expect("write png");
        eprintln!("{name}");
    }
}

/// Step the clock in frames so that eased things (the corner marks) move as they would on screen.
fn run(ui: &mut DemoUi, to: f32) {
    let mut t = ui.now();
    while t < to { t = (t + 1.0 / 60.0).min(to); ui.tick(t); }
}

/// Scroll the page until `key` sits in the upper part of the window, then return its centre.
fn centre(ui: &mut DemoUi, key: &str) -> (f32, f32) {
    let r = ui.node(key).unwrap_or_else(|| panic!("no node {key}")).rect;
    let (_, lh) = ui.logical_size();
    if r.y < 0.0 || r.bottom() > lh { ui.wheel(4.0, 4.0, r.y - lh * 0.3); }
    ui.node(key).expect("node").rect.center()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = a.get(1).cloned().unwrap_or_else(|| "shots".into());
    let w: u32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(1280);
    let h: u32 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(900);
    let scale: f32 = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(1.0);
    std::fs::create_dir_all(&out).expect("out dir");
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
    let (pw, ph) = (640, 320);
    r.set_picture(0, pw, ph, &demo::picture_pixels(pw, ph));
    let font = std::fs::read("assets/NotoSansSC-Regular.otf").expect("font");
    let mut ui = demo::new_ui(font, w, h, scale);
    let mut s = Shooter { r, target, buf, w, h, row, out };
    let (lw, lh) = (w as f32 / scale, h as f32 / scale);

    run(&mut ui, 0.1);
    s.shoot(&mut ui, "0-condensing");
    run(&mut ui, 10.0);
    s.shoot(&mut ui, "1-rest");

    let (px, py) = (lw * 0.3, lh * 0.55);
    ui.press(px, py);
    ui.release(px, py);
    run(&mut ui, 10.2);
    s.shoot(&mut ui, "2-wave-early");
    run(&mut ui, 10.5);
    s.shoot(&mut ui, "3-wave-mid");

    let (bx, by) = centre(&mut ui, "inc");
    ui.pointer_move(bx, by);
    run(&mut ui, 12.0);
    s.shoot(&mut ui, "4-hover-button");

    ui.press(bx, by);
    run(&mut ui, 12.1);
    s.shoot(&mut ui, "5-held");
    ui.release(bx, by);
    run(&mut ui, 12.25);
    s.shoot(&mut ui, "6-released");
    run(&mut ui, 14.0);
    s.shoot(&mut ui, "7-after-click");

    let (lx, ly) = centre(&mut ui, "list");
    ui.pointer_move(lx, ly);
    ui.wheel(lx, ly, 330.0);
    run(&mut ui, 15.0);
    s.shoot(&mut ui, "8-list-scrolled");

    // Keyboard: focus the first parameter and type a new value without committing it.
    let (fx, fy) = centre(&mut ui, "param/0/value");
    ui.press(fx, fy);
    ui.release(fx, fy);
    for c in "6.5".chars() { ui.key(Key::Char(c), false); }
    run(&mut ui, 17.0);
    s.shoot(&mut ui, "9-typing");
    ui.key(Key::Enter, false);
    ui.key(Key::Tab, false);
    run(&mut ui, 18.0);
    s.shoot(&mut ui, "10-committed-tab");

    let (tx, ty) = centre(&mut ui, "light");
    ui.press(tx, ty);
    ui.release(tx, ty);
    run(&mut ui, 18.35);
    s.shoot(&mut ui, "11-light-wave");
    let (_, picy) = centre(&mut ui, "picture");
    ui.pointer_move(lw * 0.3, picy);
    run(&mut ui, 21.0);
    s.shoot(&mut ui, "12-light-lens");

    // The whole page scrolls when it is taller than the window.
    let (cx, cy) = (8.0, lh * 0.5);
    ui.wheel(cx, cy, 100_000.0);
    ui.pointer_leave();
    run(&mut ui, 22.0);
    s.shoot(&mut ui, "13-page-end");
}
