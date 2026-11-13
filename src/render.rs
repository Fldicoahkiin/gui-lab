//! wgpu: draws a `Scene` in two stages. Stage 1 draws every instance into the ink and near images
//! (the same geometry twice, since WebGL2 cannot blend two targets differently); stage 2 is one
//! full-screen material pass that reads them with the field.
use crate::marks::{self, Atlas};
use crate::scene::{Field, Instance, Scene};
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
    /// A commit in progress: where its front started (device px), when, and whether it is on.
    split: [f32; 4],
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
    chars: wgpu::Texture,
    chars_view: wgpu::TextureView,
    /// A 1 × 1 texture for bindings a pass does not read.
    blank: wgpu::TextureView,
    /// The frame as it was before the last commit, and which commit that was.
    before: wgpu::TextureView,
    split_seen: u32,
    split: [f32; 4],
    /// What the last frame was drawn with: a commit shows the old frame in the old look.
    last_look: Option<Look>,
    /// The mark atlas needs the text system (for the character set), so it is made on the first draw.
    marks_ready: bool,
    /// Stage-1 bind groups: atlas plus one picture each. Id `u32::MAX` has a blank picture.
    pictures: HashMap<u32, wgpu::BindGroup>,
    instances: wgpu::Buffer,
    capacity: usize,
    pipes: [wgpu::RenderPipeline; 4], // ink, near, near for the modal layer, material
    ink: wgpu::TextureView,
    near: wgpu::TextureView,
    material: wgpu::BindGroup,
    /// The same without the old frame, for drawing into it.
    material_first: wgpu::BindGroup,
    size: (u32, u32),
}

/// What one frame is drawn from.
struct Frame<'a> { scene: &'a Scene, field: &'a Field, look: &'a Look, scale: f32, t: f32, split: [f32; 4] }

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
            tex_entry(2), tex_entry(3), tex_entry(4), tex_entry(5), tex_entry(6), tex_entry(7),
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
            // The modal layer replaces what is beneath it where it has ink, so its card is not loose and
            // pictures under it do not show through as pictures.
            pipe("vs_inst", "fs_near_top", NEAR, None, true),
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
        let chars = device.create_texture(&wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: marks::CHAR_W as u32, height: marks::CHAR_H as u32, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::R8Unorm, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
        });
        let chars_view = chars.create_view(&Default::default());
        let capacity = 4096;
        let instances = Self::instance_buffer(&device, capacity);
        let blank = Self::target(&device, 1, 1, INK);
        let material = Self::bind(&device, &layout, &ubuf, &sampler, [&blank, &blank, &marks_view, &palette_view, &chars_view, &blank]);
        let mut r = Self {
            device, queue, out_format, layout, ubuf, sampler, atlas, atlas_view, marks, marks_view, palette_view, chars, chars_view,
            before: blank.clone(), blank: blank.clone(), split_seen: 0, split: [0.0; 4], last_look: None, marks_ready: false,
            pictures: HashMap::new(), instances, capacity, pipes,
            ink: blank.clone(), near: blank, material_first: material.clone(), material, size: (0, 0),
        };
        r.set_picture(u32::MAX, 1, 1, &[0, 0, 0, 255]);
        r.resize(w, h);
        r
    }

    fn instance_buffer(device: &wgpu::Device, n: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (n * std::mem::size_of::<Instance>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
    }
    fn target(device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat) -> wgpu::TextureView {
        Self::target_with(device, w, h, format, wgpu::TextureUsages::empty())
    }
    fn target_with(device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat, extra: wgpu::TextureUsages) -> wgpu::TextureView {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT | extra, view_formats: &[],
        }).create_view(&Default::default())
    }
    /// Every pass uses one layout: uniforms, sampler, then six textures (stage 1: glyph atlas and a
    /// picture; stage 2: ink and near), the mark atlas, the palettes, the character tiles and the frame
    /// before the last commit.
    fn bind(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, ubuf: &wgpu::Buffer, sampler: &wgpu::Sampler, t: [&wgpu::TextureView; 6]) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(t[0]) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(t[1]) },
            wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(t[2]) },
            wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(t[3]) },
            wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(t[4]) },
            wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(t[5]) },
        ] })
    }

    /// Register picture `id` (RGBA8, `w` × `h`).
    pub fn set_picture(&mut self, id: u32, w: u32, h: u32, rgba: &[u8]) {
        let tex = self.device.create_texture_with_data(&self.queue, &wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Unorm, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
        }, wgpu::util::TextureDataOrder::LayerMajor, rgba);
        let view = tex.create_view(&Default::default());
        let group = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.atlas_view, &view, &self.marks_view, &self.palette_view, &self.chars_view, &self.blank]);
        self.pictures.insert(id, group);
    }

    /// Device px.
    pub fn resize(&mut self, w: u32, h: u32) {
        let (w, h) = (w.max(1), h.max(1));
        if (w, h) == self.size { return; }
        self.size = (w, h);
        self.ink = Self::target(&self.device, w, h, INK);
        self.near = Self::target(&self.device, w, h, NEAR);
        self.before = Self::target(&self.device, w, h, self.out_format);
        self.material = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.ink, &self.near, &self.marks_view, &self.palette_view, &self.chars_view, &self.before]);
        self.material_first = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.ink, &self.near, &self.marks_view, &self.palette_view, &self.chars_view, &self.blank]);
        // The old frame is gone with the old size.
        self.split = [0.0; 4];
    }

    pub fn device(&self) -> &wgpu::Device { &self.device }
    pub fn queue(&self) -> &wgpu::Queue { &self.queue }

    fn upload_marks(&mut self, atlas: &Atlas, chars: &marks::CharTiles) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &self.chars, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &chars.pixels,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(marks::CHAR_W as u32), rows_per_image: Some(marks::CHAR_H as u32) },
            wgpu::Extent3d { width: marks::CHAR_W as u32, height: marks::CHAR_H as u32, depth_or_array_layers: 1 },
        );
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
            let chars = marks::char_tiles(ui.text_mut(), face);
            self.upload_marks(&atlas, &chars);
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
        let (scale, now) = (ui.scale(), ui.now());
        // A new commit: draw the frame as it was, in the look it had, into `before`, once.
        if let Some(sp) = ui.split() && sp.id != self.split_seen {
            self.split_seen = sp.id;
            if let Some(old) = self.last_look {
                let before = self.before.clone();
                self.frame(&before, Frame { scene: &sp.scene, field: &sp.field, look: &old, scale, t: sp.t, split: [0.0; 4] }, false);
                self.split = [sp.at[0], sp.at[1], sp.t, 1.0];
            }
        }
        // The front has left the screen: the old frame is not needed any more.
        let (w, h) = (self.size.0 as f32, self.size.1 as f32);
        if self.split[3] > 0.0 && (now - self.split[2]) * look.fx.wave_speed * scale > (w * w + h * h).sqrt() { self.split = [0.0; 4]; }
        self.last_look = Some(*look);
        let split = self.split;
        self.frame(target, Frame { scene: ui.scene(), field: ui.field(), look, scale, t: now, split }, true);
    }

    /// Both stages of one frame into `target`, submitted on its own so that each frame gets its own
    /// uniforms and instances. `with_before` binds the old frame (not when drawing into it).
    fn frame(&mut self, target: &wgpu::TextureView, f: Frame<'_>, with_before: bool) {
        let scene = f.scene;
        self.uniforms(f.field, f.look, f.scale, f.t, f.split);
        if scene.instances.len() > self.capacity {
            self.capacity = scene.instances.len().next_power_of_two();
            self.instances = Self::instance_buffer(&self.device, self.capacity);
        }
        self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&scene.instances));

        let mut enc = self.device.create_command_encoder(&Default::default());
        let layer = scene.layer.unwrap_or(u32::MAX);
        for (view, pipe, top) in [(&self.ink, &self.pipes[0], &self.pipes[0]), (&self.near, &self.pipes[1], &self.pipes[2])] {
            let mut pass = clear(&mut enc, view, wgpu::Color::TRANSPARENT);
            pass.set_vertex_buffer(0, self.instances.slice(..));
            for b in &scene.batches {
                pass.set_pipeline(if b.start >= layer { top } else { pipe });
                let group = self.pictures.get(&b.picture).or_else(|| self.pictures.get(&u32::MAX)).expect("blank picture");
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..4, b.start..b.end);
            }
        }
        {
            let mut pass = clear(&mut enc, target, wgpu::Color::BLACK);
            pass.set_pipeline(&self.pipes[3]);
            pass.set_bind_group(0, if with_before { &self.material } else { &self.material_first }, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([enc.finish()]);
    }

    fn uniforms(&self, field: &Field, look: &Look, scale: f32, t: f32, split: [f32; 4]) {
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
            bg, fg, accent, pointer: field.pointer, wipe: field.wipe, wipe_rect: field.wipe_rect, split, fronts: field.fronts, pics,
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
