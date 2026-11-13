//! The slice: one screen of proportional mixed text and a picture, drawn in two stages.
use crate::text::{ATLAS, GlyphInst, SPREAD, Text};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    res: [f32; 4],
    params: [f32; 4],
    mode: [f32; 4],
    bg: [f32; 4],
    fg: [f32; 4],
    pointer: [f32; 4],
    ripples: [[f32; 4]; 8],
}

#[derive(Clone, Copy)]
pub struct Params {
    /// Lattice pitch for text and for pictures, in logical px.
    pub pitch: f32,
    pub pic_pitch: f32,
    pub glitch: f32,
    pub jitter: f32,
    pub square: bool,
    pub light: bool,
}
impl Default for Params {
    fn default() -> Self { Self { pitch: 5.0, pic_pitch: 8.0, glitch: 0.5, jitter: 0.0, square: false, light: false } }
}

const INK: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const NEAR: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;
const PIC: (u32, u32) = (640, 400);

pub struct App {
    device: wgpu::Device,
    queue: wgpu::Queue,
    out_format: wgpu::TextureFormat,
    text: Text,
    glyphs: Vec<GlyphInst>,
    pic_inst: GlyphInst,
    pub params: Params,
    size: (u32, u32),
    scale: f32,
    ripples: [[f32; 4]; 8],
    next_ripple: usize,
    pointer: [f32; 4],
    // GPU
    layout: wgpu::BindGroupLayout,
    ubuf: wgpu::Buffer,
    sampler: wgpu::Sampler,
    atlas: wgpu::Texture,
    pic: wgpu::TextureView,
    glyph_buf: wgpu::Buffer,
    pic_buf: wgpu::Buffer,
    pipes: [wgpu::RenderPipeline; 5], // glyph ink, glyph near, pic ink, pic near, material
    ink: wgpu::TextureView,
    near: wgpu::TextureView,
    bg_atlas: wgpu::BindGroup,
    bg_pic: wgpu::BindGroup,
    bg_mat: wgpu::BindGroup,
}

impl App {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, out_format: wgpu::TextureFormat, font: Vec<u8>, w: u32, h: u32, scale: f32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()) });
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding, visibility: wgpu::ShaderStages::FRAGMENT, count: None,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[
            wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, count: None, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None } },
            wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, count: None, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering) },
            tex_entry(2), tex_entry(3),
        ] });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let inst_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GlyphInst>() as u64, step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
        };
        let premult = wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING;
        let maxb = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Max };
        let max = wgpu::BlendState { color: maxb, alpha: maxb };
        let inst_bufs = [Some(inst_layout.clone())];
        let pipe = |vs: &str, fs: &str, format, blend: Option<wgpu::BlendState>, inst: bool| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fs), layout: Some(&pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some(vs), compilation_options: Default::default(), buffers: if inst { &inst_bufs } else { &[] } },
            primitive: wgpu::PrimitiveState { topology: if inst { wgpu::PrimitiveTopology::TriangleStrip } else { wgpu::PrimitiveTopology::TriangleList }, ..Default::default() },
            depth_stencil: None, multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some(fs), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })] }),
            multiview_mask: None, cache: None,
        });
        let pipes = [
            pipe("vs_inst", "fs_glyph_ink", INK, Some(premult), true),
            pipe("vs_inst", "fs_glyph_near", NEAR, Some(max), true),
            pipe("vs_inst", "fs_pic_ink", INK, None, true),
            pipe("vs_inst", "fs_pic_near", NEAR, Some(max), true),
            pipe("vs_full", "fs_material", out_format, None, false),
        ];
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: std::mem::size_of::<Uniforms>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let tex = |w, h, format, usage| device.create_texture(&wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format, usage, view_formats: &[],
        });
        let sampled = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let atlas = tex(ATLAS, ATLAS, NEAR, sampled);
        let pic_tex = device.create_texture_with_data(&queue, &wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: PIC.0, height: PIC.1, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Unorm, usage: sampled, view_formats: &[],
        }, wgpu::util::TextureDataOrder::LayerMajor, &picture(PIC.0, PIC.1));
        let pic = pic_tex.create_view(&Default::default());
        let empty = |n: u64| device.create_buffer(&wgpu::BufferDescriptor { label: None, size: n, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let glyph_buf = empty(48 * 8192);
        let pic_buf = empty(48);
        let dummy = tex(1, 1, INK, wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT).create_view(&Default::default());
        let bind = |a: &wgpu::TextureView, b: &wgpu::TextureView| device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &layout, entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(a) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(b) },
        ] });
        let atlas_view = atlas.create_view(&Default::default());
        let bg_atlas = bind(&atlas_view, &dummy);
        let bg_pic = bind(&pic, &dummy);
        let bg_mat = bind(&dummy, &dummy);
        let mut app = Self {
            device, queue, out_format, text: Text::new(font), glyphs: vec![], pic_inst: bytemuck::Zeroable::zeroed(), params: Params::default(),
            size: (0, 0), scale, ripples: [[0.0; 4]; 8], next_ripple: 0, pointer: [-1e5, -1e5, 0.0, 0.0],
            layout, ubuf, sampler, atlas, pic, glyph_buf, pic_buf, pipes, ink: dummy.clone(), near: dummy, bg_atlas, bg_pic, bg_mat,
        };
        app.resize(w, h, scale);
        app
    }

    pub fn resize(&mut self, w: u32, h: u32, scale: f32) {
        let (w, h) = (w.max(1), h.max(1));
        if (w, h) == self.size && scale == self.scale { return; }
        self.size = (w, h); self.scale = scale;
        let usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT;
        let mk = |format| self.device.create_texture(&wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format, usage, view_formats: &[],
        }).create_view(&Default::default());
        self.ink = mk(INK); self.near = mk(NEAR);
        self.bg_mat = self.device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &self.layout, entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: self.ubuf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.ink) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.near) },
        ] });
        self.compose();
    }

    /// Lay the page out. Sizes are logical px; nothing here is monospaced or on a grid.
    fn compose(&mut self) {
        let s = self.scale;
        let lw = self.size.0 as f32 / s;
        let margin = if lw < 560.0 { 20.0 } else { 56.0 };
        let width = (lw - margin * 2.0).min(760.0);
        let x = ((lw - width) / 2.0).floor();
        let mut y = if lw < 560.0 { 28.0 } else { 52.0 };
        let mut g = Vec::new();
        let big = if lw < 560.0 { 30.0 } else { 44.0 };
        y += self.text.paragraph(&mut g, "字和图，是同一种东西做的", big, big * 1.25, width, (x, y), 1.0, s) + 10.0;
        y += self.text.paragraph(&mut g, "Type and pictures are made of the same stuff.", 17.0, 26.0, width, (x, y), 0.5, s) + 26.0;
        let ph = width * 0.625;
        let sp = SPREAD as f32;
        self.pic_inst = GlyphInst { rect: [x * s - sp, y * s - sp, width * s + sp * 2.0, ph * s + sp * 2.0], uv: [-sp / (width * s), -sp / (ph * s), 1.0 + sp / (width * s), 1.0 + sp / (ph * s)], tone: [1.0; 4] };
        y += ph + 26.0;
        for (t, tone) in [
            ("在空白处按一下，会有一圈波传出去。波扫到哪里，哪里的字就松开成一颗一颗的点，再合回来。字不是被别的东西盖住了，是它自己散开的；每个字也会被波推着轻轻动一下。", 0.86),
            ("把指针移到图上，指针周围的图会松成点；点的疏密可以调。指针经过文字时，文字不变。", 0.86),
            ("背景里的小点会避开字，按的是每一笔的形状，不是一个方框。The ground keeps clear of every stroke by its real outline, never by a box around the text.", 0.5),
        ] {
            y += self.text.paragraph(&mut g, t, 17.0, 30.0, width, (x, y), tone, s) + 16.0;
        }
        g.truncate(8192);
        self.glyphs = g;
        self.queue.write_buffer(&self.glyph_buf, 0, bytemuck::cast_slice(&self.glyphs));
        self.queue.write_buffer(&self.pic_buf, 0, bytemuck::bytes_of(&self.pic_inst));
    }

    /// Pointer position in device px. `strength` 0 hides the lens.
    pub fn pointer(&mut self, x: f32, y: f32, strength: f32) { self.pointer = [x, y, strength, 0.0]; }
    /// Start a front at a point (device px) at time `t`.
    pub fn press(&mut self, x: f32, y: f32, t: f32) {
        self.ripples[self.next_ripple] = [x, y, t, 1.0];
        self.next_ripple = (self.next_ripple + 1) % 8;
    }
    pub fn out_format(&self) -> wgpu::TextureFormat { self.out_format }

    pub fn render(&mut self, target: &wgpu::TextureView, t: f32) {
        if self.text.dirty {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &self.atlas, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &self.text.pixels,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(ATLAS * 2), rows_per_image: Some(ATLAS) },
                wgpu::Extent3d { width: ATLAS, height: ATLAS, depth_or_array_layers: 1 },
            );
            self.text.dirty = false;
        }
        let p = self.params;
        let (bg, fg) = if p.light { ([0.914, 0.910, 0.890, 1.0], [0.09, 0.094, 0.10, 1.0]) } else { ([0.047, 0.051, 0.055, 1.0], [0.886, 0.89, 0.878, 1.0]) };
        let u = Uniforms {
            res: [self.size.0 as f32, self.size.1 as f32, self.scale, t],
            params: [p.pitch, p.pic_pitch, p.glitch, p.jitter],
            mode: [if p.square { 1.0 } else { 0.0 }, if self.out_format.is_srgb() { 1.0 } else { 0.0 }, SPREAD as f32, 0.0],
            bg, fg, pointer: self.pointer, ripples: self.ripples,
        };
        self.queue.write_buffer(&self.ubuf, 0, bytemuck::bytes_of(&u));
        let mut enc = self.device.create_command_encoder(&Default::default());
        let n = self.glyphs.len() as u32;
        // Stage 1, drawn twice with the same geometry: once for ink, once for nearness.
        for (view, gp, pp) in [(&self.ink, 0, 2), (&self.near, 1, 3)] {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None, depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } })],
            });
            pass.set_pipeline(&self.pipes[pp]); pass.set_bind_group(0, &self.bg_pic, &[]);
            pass.set_vertex_buffer(0, self.pic_buf.slice(..)); pass.draw(0..4, 0..1);
            pass.set_pipeline(&self.pipes[gp]); pass.set_bind_group(0, &self.bg_atlas, &[]);
            pass.set_vertex_buffer(0, self.glyph_buf.slice(..)); pass.draw(0..4, 0..n);
        }
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None, depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
            });
            pass.set_pipeline(&self.pipes[4]); pass.set_bind_group(0, &self.bg_mat, &[]); pass.draw(0..3, 0..1);
        }
        self.queue.submit([enc.finish()]);
        let _ = &self.pic;
    }
    pub fn device(&self) -> &wgpu::Device { &self.device }
    pub fn queue(&self) -> &wgpu::Queue { &self.queue }
}

/// A picture computed in code: a lit sphere over a horizon.
fn picture(w: u32, h: u32) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    let (cx, cy, r) = (w as f32 * 0.6, h as f32 * 0.46, h as f32 * 0.33);
    let light = { let l = [-0.5f32, -0.62, 0.6]; let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt(); [l[0] / n, l[1] / n, l[2] / n] };
    for y in 0..h { for x in 0..w {
        let (fx, fy) = (x as f32, y as f32);
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
        let o = ((y * w + x) * 4) as usize;
        px[o] = (c[0] * 255.0) as u8; px[o + 1] = (c[1] * 255.0) as u8; px[o + 2] = (c[2] * 255.0) as u8; px[o + 3] = 255;
    } }
    px
}
