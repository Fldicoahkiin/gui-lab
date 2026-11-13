//! wgpu: draws a `Scene` in two stages. Stage 1 draws every instance into the ink and near images
//! (the same geometry twice, since WebGL2 cannot blend two targets differently); stage 2 is one
//! full-screen material pass that reads them with the field.
use crate::scene::{Field, Fx, Instance};
use crate::text::{ATLAS, SPREAD};
use crate::ui::Ui;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    res: [f32; 4],
    fa: [f32; 4],
    fb: [f32; 4],
    fc: [f32; 4],
    fd: [f32; 4],
    bg: [f32; 4],
    fg: [f32; 4],
    accent: [f32; 4],
    pointer: [f32; 4],
    fronts: [[f32; 4]; 8],
}

const INK: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const NEAR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const ATLAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;

/// Ground, ink and accent colours for a theme.
pub fn theme(light: bool) -> ([f32; 4], [f32; 4], [f32; 4]) {
    if light { ([0.914, 0.910, 0.890, 1.0], [0.09, 0.094, 0.10, 1.0], [0.86, 0.30, 0.08, 1.0]) }
    else { ([0.047, 0.051, 0.055, 1.0], [0.886, 0.89, 0.878, 1.0], [1.0, 0.42, 0.17, 1.0]) }
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    out_format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    ubuf: wgpu::Buffer,
    sampler: wgpu::Sampler,
    atlas: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    /// Stage-1 bind groups: atlas plus one picture each. Id `u32::MAX` has a blank picture.
    pictures: HashMap<u32, wgpu::BindGroup>,
    instances: wgpu::Buffer,
    capacity: usize,
    pipes: [wgpu::RenderPipeline; 3], // ink, near, material
    ink: wgpu::TextureView,
    near: wgpu::TextureView,
    material: wgpu::BindGroup,
    size: (u32, u32),
}

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, out_format: wgpu::TextureFormat, w: u32, h: u32) -> Self {
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
            array_stride: std::mem::size_of::<Instance>() as u64, step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4],
        };
        let maxb = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Max };
        let inst_bufs = [Some(inst_layout)];
        let pipe = |vs: &str, fs: &str, format, blend: Option<wgpu::BlendState>, inst: bool| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fs), layout: Some(&pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some(vs), compilation_options: Default::default(), buffers: if inst { &inst_bufs } else { &[] } },
            primitive: wgpu::PrimitiveState { topology: if inst { wgpu::PrimitiveTopology::TriangleStrip } else { wgpu::PrimitiveTopology::TriangleList }, ..Default::default() },
            depth_stencil: None, multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some(fs), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })] }),
            multiview_mask: None, cache: None,
        });
        let pipes = [
            pipe("vs_inst", "fs_ink", INK, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), true),
            pipe("vs_inst", "fs_near", NEAR, Some(wgpu::BlendState { color: maxb, alpha: maxb }), true),
            pipe("vs_full", "fs_material", out_format, None, false),
        ];
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: std::mem::size_of::<Uniforms>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: ATLAS, height: ATLAS, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format: ATLAS_FORMAT, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
        });
        let atlas_view = atlas.create_view(&Default::default());
        let capacity = 4096;
        let instances = Self::instance_buffer(&device, capacity);
        let blank = Self::target(&device, 1, 1, INK);
        let material = Self::bind(&device, &layout, &ubuf, &sampler, &blank, &blank);
        let mut r = Self {
            device, queue, out_format, layout, ubuf, sampler, atlas, atlas_view, pictures: HashMap::new(), instances, capacity, pipes,
            ink: blank.clone(), near: blank, material, size: (0, 0),
        };
        r.set_picture(u32::MAX, 1, 1, &[0, 0, 0, 255]);
        r.resize(w, h);
        r
    }

    fn instance_buffer(device: &wgpu::Device, n: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (n * std::mem::size_of::<Instance>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
    }
    fn target(device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat) -> wgpu::TextureView {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT, view_formats: &[],
        }).create_view(&Default::default())
    }
    fn bind(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, ubuf: &wgpu::Buffer, sampler: &wgpu::Sampler, a: &wgpu::TextureView, b: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(a) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(b) },
        ] })
    }

    /// Register picture `id` (RGBA8, `w` × `h`).
    pub fn set_picture(&mut self, id: u32, w: u32, h: u32, rgba: &[u8]) {
        let tex = self.device.create_texture_with_data(&self.queue, &wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Unorm, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
        }, wgpu::util::TextureDataOrder::LayerMajor, rgba);
        let view = tex.create_view(&Default::default());
        let group = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, &self.atlas_view, &view);
        self.pictures.insert(id, group);
    }

    /// Device px.
    pub fn resize(&mut self, w: u32, h: u32) {
        let (w, h) = (w.max(1), h.max(1));
        if (w, h) == self.size { return; }
        self.size = (w, h);
        self.ink = Self::target(&self.device, w, h, INK);
        self.near = Self::target(&self.device, w, h, NEAR);
        self.material = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, &self.ink, &self.near);
    }

    pub fn device(&self) -> &wgpu::Device { &self.device }
    pub fn queue(&self) -> &wgpu::Queue { &self.queue }

    pub fn draw<S, M: Clone>(&mut self, target: &wgpu::TextureView, ui: &mut Ui<S, M>, fx: &Fx) {
        self.resize(ui.device_size().0, ui.device_size().1);
        let text = ui.text_mut();
        if text.dirty {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &self.atlas, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &text.pixels,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(ATLAS * 2), rows_per_image: Some(ATLAS) },
                wgpu::Extent3d { width: ATLAS, height: ATLAS, depth_or_array_layers: 1 },
            );
            text.dirty = false;
        }
        self.uniforms(ui.field(), fx, ui.scale(), ui.now());
        let scene = ui.scene();
        if scene.instances.len() > self.capacity {
            self.capacity = scene.instances.len().next_power_of_two();
            self.instances = Self::instance_buffer(&self.device, self.capacity);
        }
        self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&scene.instances));

        let mut enc = self.device.create_command_encoder(&Default::default());
        let clear = |enc: &mut wgpu::CommandEncoder, view, color| enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None, depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(color), store: wgpu::StoreOp::Store } })],
        });
        for (view, pipe) in [(&self.ink, &self.pipes[0]), (&self.near, &self.pipes[1])] {
            let mut pass = clear(&mut enc, view, wgpu::Color::TRANSPARENT);
            pass.set_pipeline(pipe);
            pass.set_vertex_buffer(0, self.instances.slice(..));
            for b in &scene.batches {
                let group = self.pictures.get(&b.picture).or_else(|| self.pictures.get(&u32::MAX)).expect("blank picture");
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..4, b.start..b.end);
            }
        }
        {
            let mut pass = clear(&mut enc, target, wgpu::Color::BLACK);
            pass.set_pipeline(&self.pipes[2]);
            pass.set_bind_group(0, &self.material, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([enc.finish()]);
    }

    fn uniforms(&self, field: &Field, fx: &Fx, scale: f32, t: f32) {
        let (bg, fg, accent) = theme(fx.light);
        let flag = |b: bool| if b { 1.0 } else { 0.0 };
        let u = Uniforms {
            res: [self.size.0 as f32, self.size.1 as f32, scale, t],
            fa: [fx.pitch, fx.pic_pitch, fx.jitter, fx.glitch],
            fb: [fx.wave_speed, fx.wave_life, fx.wave_width, fx.wave_tail],
            fc: [fx.push, fx.lens, fx.enter, flag(fx.square)],
            fd: [flag(fx.light), flag(self.out_format.is_srgb()), SPREAD as f32, 0.0],
            bg, fg, accent, pointer: field.pointer, fronts: field.fronts,
        };
        self.queue.write_buffer(&self.ubuf, 0, bytemuck::bytes_of(&u));
    }
}
