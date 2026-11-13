//! wgpu: draws a `Scene` in two stages. Stage 1 draws every instance into the ink and near images
//! (the same geometry twice, since WebGL2 cannot blend two targets differently); stage 2 is one
//! full-screen material pass that reads them with the field.
use crate::marks::{self, Atlas};
use crate::scene::{Field, Instance};
use crate::style::{Look, Material};
use crate::text::{ATLAS, MONO, SPREAD};
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
    wipe: [f32; 4],
    wipe_rect: [f32; 4],
    fronts: [[f32; 4]; 8],
    /// Eight `PicFx` slots, seven vec4s each.
    pics: [[f32; 4]; 56],
}

const INK: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const NEAR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const ATLAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;

const MARKS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    out_format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    ubuf: wgpu::Buffer,
    sampler: wgpu::Sampler,
    atlas: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    marks: wgpu::Texture,
    marks_view: wgpu::TextureView,
    palette_view: wgpu::TextureView,
    /// The mark atlas needs the text system (for the character set), so it is made on the first draw.
    marks_ready: bool,
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
            tex_entry(2), tex_entry(3), tex_entry(4), tex_entry(5),
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
        // Only the mark atlas has mip levels; for every other texture the mipmap filter does nothing.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, mipmap_filter: wgpu::MipmapFilterMode::Linear, ..Default::default() });
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: ATLAS, height: ATLAS, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format: ATLAS_FORMAT, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
        });
        let atlas_view = atlas.create_view(&Default::default());
        let marks = device.create_texture(&wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: marks::WIDTH as u32, height: marks::HEIGHT as u32, depth_or_array_layers: 1 }, mip_level_count: marks::MIPS as u32, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format: MARKS_FORMAT, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
        });
        let marks_view = marks.create_view(&Default::default());
        let palette_view = device.create_texture_with_data(&queue, &wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: 16, height: marks::PALETTES.len() as u32, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Unorm, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
        }, wgpu::util::TextureDataOrder::LayerMajor, &marks::palette_pixels()).create_view(&Default::default());
        let capacity = 4096;
        let instances = Self::instance_buffer(&device, capacity);
        let blank = Self::target(&device, 1, 1, INK);
        let material = Self::bind(&device, &layout, &ubuf, &sampler, [&blank, &blank, &marks_view, &palette_view]);
        let mut r = Self {
            device, queue, out_format, layout, ubuf, sampler, atlas, atlas_view, marks, marks_view, palette_view, marks_ready: false,
            pictures: HashMap::new(), instances, capacity, pipes,
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
    /// Every pass uses one layout: uniforms, sampler, then four textures (stage 1: glyph atlas and a
    /// picture; stage 2: ink and near), the mark atlas and the palettes.
    fn bind(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, ubuf: &wgpu::Buffer, sampler: &wgpu::Sampler, t: [&wgpu::TextureView; 4]) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(t[0]) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(t[1]) },
            wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(t[2]) },
            wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(t[3]) },
        ] })
    }

    /// Register picture `id` (RGBA8, `w` × `h`).
    pub fn set_picture(&mut self, id: u32, w: u32, h: u32, rgba: &[u8]) {
        let tex = self.device.create_texture_with_data(&self.queue, &wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Unorm, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
        }, wgpu::util::TextureDataOrder::LayerMajor, rgba);
        let view = tex.create_view(&Default::default());
        let group = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.atlas_view, &view, &self.marks_view, &self.palette_view]);
        self.pictures.insert(id, group);
    }

    /// Device px.
    pub fn resize(&mut self, w: u32, h: u32) {
        let (w, h) = (w.max(1), h.max(1));
        if (w, h) == self.size { return; }
        self.size = (w, h);
        self.ink = Self::target(&self.device, w, h, INK);
        self.near = Self::target(&self.device, w, h, NEAR);
        self.material = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.ink, &self.near, &self.marks_view, &self.palette_view]);
    }

    pub fn device(&self) -> &wgpu::Device { &self.device }
    pub fn queue(&self) -> &wgpu::Queue { &self.queue }

    fn upload_marks(&mut self, atlas: &Atlas) {
        for (level, (w, h, px)) in atlas.mips().iter().enumerate() {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &self.marks, mip_level: level as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                px,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(*w as u32 * 2), rows_per_image: Some(*h as u32) },
                wgpu::Extent3d { width: *w as u32, height: *h as u32, depth_or_array_layers: 1 },
            );
        }
        self.marks_ready = true;
    }

    pub fn draw<S, M: Clone>(&mut self, target: &wgpu::TextureView, ui: &mut Ui<S, M>, look: &Look) {
        self.resize(ui.device_size().0, ui.device_size().1);
        if !self.marks_ready {
            let face = if ui.text_mut().faces() > MONO as usize { MONO } else { 0 };
            let atlas = Atlas::build(ui.text_mut(), face);
            self.upload_marks(&atlas);
        }
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
        self.uniforms(ui.field(), look, ui.scale(), ui.now());
        let scene = ui.scene();
        if scene.instances.len() > self.capacity {
            self.capacity = scene.instances.len().next_power_of_two();
            self.instances = Self::instance_buffer(&self.device, self.capacity);
        }
        self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&scene.instances));

        let mut enc = self.device.create_command_encoder(&Default::default());
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

    fn uniforms(&self, field: &Field, look: &Look, scale: f32, t: f32) {
        let (fx, th) = (&look.fx, &look.theme);
        let c = |v: [f32; 3]| [v[0], v[1], v[2], 1.0];
        let (bg, fg, accent) = (c(th.ground), c(th.ink), c(th.accent));
        let flag = |b: bool| if b { 1.0 } else { 0.0 };
        let mut pics = [[0.0; 4]; 56];
        for (i, p) in look.pics.iter().enumerate() { pics[i * 7..i * 7 + 7].copy_from_slice(&p.pack()); }
        let u = Uniforms {
            res: [self.size.0 as f32, self.size.1 as f32, scale, t],
            fa: [fx.pitch, 0.0, fx.jitter, fx.glitch],
            fb: [fx.wave_speed, fx.wave_life, fx.wave_width, fx.wave_tail],
            fc: [fx.push, fx.lens, fx.enter, flag(fx.square)],
            fd: [flag(look.material == Material::Dot), flag(self.out_format.is_srgb()), SPREAD as f32, marks::PALETTES.len() as f32],
            bg, fg, accent, pointer: field.pointer, wipe: field.wipe, wipe_rect: field.wipe_rect, fronts: field.fronts, pics,
        };
        self.queue.write_buffer(&self.ubuf, 0, bytemuck::bytes_of(&u));
    }
}

fn clear<'a>(enc: &'a mut wgpu::CommandEncoder, view: &'a wgpu::TextureView, color: wgpu::Color) -> wgpu::RenderPass<'a> {
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None, depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(color), store: wgpu::StoreOp::Store } })],
    })
}
