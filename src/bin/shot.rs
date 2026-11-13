//! Render frames off-screen to PNG files. No window, no surface.
//! usage: shot <out_dir> <width> <height> <scale>
use lab::app::App;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = a.get(1).cloned().unwrap_or_else(|| "shots".into());
    let w: u32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(1200);
    let h: u32 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(900);
    let scale: f32 = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(1.0);
    std::fs::create_dir_all(&out).unwrap();
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { force_fallback_adapter: true, ..Default::default() })).expect("no adapter");
    eprintln!("adapter: {:?}", adapter.get_info().name);
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2, format, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let font = std::fs::read("assets/NotoSansSC-Regular.otf").unwrap();
    let mut app = App::new(device.clone(), queue.clone(), format, font, w, h, scale);
    let row = (w * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut shoot = |app: &mut App, name: &str, t: f32| {
        app.render(&view, t);
        let mut enc = device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &target, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 });
        queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let data = buf.slice(..).get_mapped_range().unwrap();
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h { px.extend_from_slice(&data[(y * row) as usize..(y * row + w * 4) as usize]); }
        drop(data); buf.unmap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(format!("{out}/{name}.png")).unwrap()), w, h);
        e.set_color(png::ColorType::Rgba); e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&px).unwrap();
    };
    let s = scale;
    shoot(&mut app, "1-rest", 10.0);
    app.press(w as f32 * 0.42, h as f32 * 0.62, 10.0);
    shoot(&mut app, "2-wave-early", 10.22);
    shoot(&mut app, "3-wave-mid", 10.5);
    shoot(&mut app, "4-wave-late", 11.6);
    app.pointer(w as f32 * 0.55, 330.0 * s, 1.0);
    shoot(&mut app, "5-lens", 20.0);
    app.params.pic_pitch = 14.0; app.params.square = true; app.params.light = true; app.params.jitter = 0.6;
    shoot(&mut app, "6-lens-coarse-light", 20.0);
}
