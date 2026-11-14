//! wgpu: draws a `Scene` in two stages. Stage 1 draws every instance into the ink and near images
//! (the same geometry twice, since WebGL2 cannot blend two targets differently); stage 2 is one
//! full-screen material pass that reads them with the field.
use crate::marks::{self, Atlas};
use crate::morph::Particle;
use crate::scene::{Field, Instance, Scene};
use crate::style::{Look, MATERIAL_COUNT, Material};
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
    /// The ground at the bottom of the window, and the looseness scale in w.
    ground2: [f32; 4],
    second: [f32; 4],
    third: [f32; 4],
    burst: [f32; 4],
    burst_t: [f32; 4],
    /// Every material's numbers, two vec4s each.
    mats: [[f32; 4]; 2 * MATERIAL_COUNT],
    calm: [[f32; 4]; 4],
    /// The material resolution: 1 / q for the reduced pass (1 otherwise), q, and its size.
    qual: [f32; 4],
    /// Each front's scope box (device px).
    scopes: [[f32; 4]; 8],
    /// `Look::loose_max`, and three spare numbers.
    rip: [f32; 4],
}

const INK: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const NEAR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const ATLAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;

const MARKS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;
/// The reduced material pass runs only when at least this fraction of the window can be loose.
pub const LO_MIN_AREA: f32 = 0.15;

/// The particle pass's own uniforms (see `particles.wgsl`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PartUniforms { res: [f32; 4], m: [f32; 4], bg: [f32; 4], fg: [f32; 4], accent: [f32; 4] }

/// A morph's tiles on the GPU: the stage-1 pictures of its two elements (source ink, target ink,
/// source near, target near), drawn once per morph, and the tile buffer, uploaded once per run.
struct Tiles {
    layout: wgpu::BindGroupLayout,
    pipes: [wgpu::RenderPipeline; 2],
    ubuf: wgpu::Buffer,
    snaps: [wgpu::TextureView; 4],
    group: wgpu::BindGroup,
    buf: wgpu::Buffer,
    capacity: usize,
    count: u32,
    /// Which morph the pictures are of, and which run the buffer holds.
    seen: u32,
    uploaded: u32,
}

pub struct Renderer {
    /// Draw every glyph transparent (for acceptance checks that need to know which pixels are text).
    pub hide_glyphs: bool,
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
    pipes: [wgpu::RenderPipeline; 7], // ink, near, near for the modal layer, material, liquid field, annotations, ink for the modal layer
    /// The upsample after a reduced material pass: made the first time a quality below 1 is used (it holds
    /// the whole material again, and compiling it at start-up is slow on a CPU rasteriser).
    up_pipe: Option<wgpu::RenderPipeline>,
    shader: wgpu::ShaderModule,
    pl: wgpu::PipelineLayout,
    ink: wgpu::TextureView,
    near: wgpu::TextureView,
    soft: wgpu::TextureView,
    /// The annotation layer's instances (stage 3).
    annots: wgpu::Buffer,
    annot_capacity: usize,
    material: wgpu::BindGroup,
    /// The same without the old frame, for drawing into it.
    material_first: wgpu::BindGroup,
    /// The material at reduced resolution (DESIGN.md §1.12), its size and quality, and the bind group
    /// that reads it (with the old frame).
    lo: wgpu::TextureView,
    lo_key: (u32, u32, u32),
    material_up: wgpu::BindGroup,
    /// Automatic quality, and the quality the last frame used.
    pub auto: crate::perf::AutoQuality,
    /// Draw the reduced pass even when nothing is loose (for checks that compare rest pixels).
    pub always_lo: bool,
    size: (u32, u32),
    tiles: Tiles,
    mesh: MeshGpu,
}

/// The mesh pass's uniforms (see `mesh.wgsl`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshUniforms { view: [f32; 16], proj: [f32; 16], light: [f32; 4], misc: [f32; 4], size: [f32; 4], spread: [f32; 4] }

/// One copy of a mesh, as the mesh pass reads it (instance rate): its transform's columns, its paint
/// (tone, accent, hue) and its number + 1 as two bytes.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshCopy { m: [[f32; 4]; 4], paint: [f32; 4], id: [f32; 4] }

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const GBUF: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// One `scene3d` element's off-screen images (DESIGN.md §1.18), and what they were last drawn from.
struct MeshTarget {
    size: (u32, u32),
    ubuf: wgpu::Buffer,
    copies: wgpu::Buffer,
    capacity: usize,
    /// colour, normal + depth, id; the depth buffer; the mask, the horizontal distances, the resolve.
    views: [wgpu::TextureView; 7],
    /// mask, h, v, and the mesh pass's own (reads no image).
    groups: [wgpu::BindGroup; 4],
    /// Stage 1 reads the resolve (tex0) and the colour (tex1).
    stage1: wgpu::BindGroup,
    drawn: Option<(u64, [u32; 2], u32)>,
}

/// The mesh pass: its pipelines, every mesh of the library in one vertex buffer, and a target per element.
struct MeshGpu {
    layout: wgpu::BindGroupLayout,
    pipes: [wgpu::RenderPipeline; 4], // mesh, mask, h, v
    verts: wgpu::Buffer,
    ranges: Vec<(u32, u32)>,
    generation: u32,
    targets: Vec<MeshTarget>,
}

/// What one frame is drawn from. `tiles`: a morph's tiles are flying, this many seconds in, with this
/// curl (device px), ease, the picture flag of the morph's picture (0 if none) and the flight's length.
struct Frame<'a> { scene: &'a Scene, field: &'a Field, look: &'a Look, lib: &'a crate::mesh::Library, scale: f32, t: f32, mt: f32, split: [f32; 4], tiles: Option<[f32; 5]> }

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
            tex_entry(2), tex_entry(3), tex_entry(4), tex_entry(5), tex_entry(6), tex_entry(7), tex_entry(8), tex_entry(9),
        ] });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let inst_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64, step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4],
        };
        let maxb = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Max };
        let add = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add };
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
            // The liquid's field adds up, so outlines that come close join.
            pipe("vs_soft", "fs_soft", INK, Some(wgpu::BlendState { color: add, alpha: add }), true),
            pipe("vs_annot", "fs_annot", out_format, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), true),
            pipe("vs_inst", "fs_ink_top", INK, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), true),
        ];
        let tiles = Self::tiles(&device);
        let mesh = Self::mesh_gpu(&device);
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
        let material = Self::bind(&device, &layout, &ubuf, &sampler, [&blank, &blank, &marks_view, &palette_view, &chars_view, &blank, &blank, &blank]);
        let annots = Self::instance_buffer(&device, 256);
        let mut r = Self {
            device, queue, out_format, layout, ubuf, sampler, atlas, atlas_view, marks, marks_view, palette_view, chars, chars_view,
            before: blank.clone(), blank: blank.clone(), split_seen: 0, split: [0.0; 4], last_look: None, marks_ready: false, hide_glyphs: false,
            pictures: HashMap::new(), instances, capacity, pipes, annots, annot_capacity: 256, up_pipe: None, shader, pl,
            ink: blank.clone(), soft: blank.clone(), lo: blank.clone(), lo_key: (0, 0, 0), material_up: material.clone(), auto: Default::default(), always_lo: false,
            near: blank, material_first: material.clone(), material, size: (0, 0), tiles, mesh,
        };
        r.set_picture(u32::MAX, 1, 1, &[0, 0, 0, 255]);
        r.resize(w, h);
        r
    }

    fn tiles(device: &wgpu::Device) -> Tiles {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(include_str!("particles.wgsl").into()) });
        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding, visibility: wgpu::ShaderStages::FRAGMENT, count: None,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[
            wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, count: None, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None } },
            tex(1), tex(2), tex(3), tex(4),
        ] });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let bufs = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Particle>() as u64, step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
        })];
        let maxb = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Max };
        let pipe = |fs: &str, format, blend| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fs), layout: Some(&pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_part"), compilation_options: Default::default(), buffers: &bufs },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None, multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some(fs), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })] }),
            multiview_mask: None, cache: None,
        });
        let pipes = [pipe("fs_part_ink", INK, wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), pipe("fs_part_near", NEAR, wgpu::BlendState { color: maxb, alpha: maxb })];
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: std::mem::size_of::<PartUniforms>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let snaps = std::array::from_fn(|i| Self::target(device, 1, 1, if i < 2 { INK } else { NEAR }));
        let group = Self::tile_group(device, &layout, &ubuf, &snaps);
        let capacity = 1024;
        let buf = Self::particle_buffer(device, capacity);
        Tiles { layout, pipes, ubuf, snaps, group, buf, capacity, count: 0, seen: 0, uploaded: 0 }
    }
    fn mesh_gpu(device: &wgpu::Device) -> MeshGpu {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(include_str!("mesh.wgsl").into()) });
        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding, visibility: wgpu::ShaderStages::FRAGMENT, count: None,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[
            wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, count: None, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None } },
            tex(1), tex(2), tex(3), tex(4),
        ] });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let bufs = [
            Some(wgpu::VertexBufferLayout { array_stride: 24, step_mode: wgpu::VertexStepMode::Vertex, attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3] }),
            Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<MeshCopy>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4] }),
        ];
        let target = |format| Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL });
        let mesh = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fs_mesh"), layout: Some(&pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_mesh"), compilation_options: Default::default(), buffers: &bufs },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: Some(true), depth_compare: Some(wgpu::CompareFunction::Less), stencil: Default::default(), bias: Default::default() }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_mesh"), compilation_options: Default::default(), targets: &[target(GBUF), target(GBUF), target(GBUF)] }),
            multiview_mask: None, cache: None,
        });
        let full = |fs: &str| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fs), layout: Some(&pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some(fs), compilation_options: Default::default(), targets: &[target(GBUF)] }),
            multiview_mask: None, cache: None,
        });
        let pipes = [mesh, full("fs_mask"), full("fs_h"), full("fs_v")];
        let verts = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: 24, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        MeshGpu { layout, pipes, verts, ranges: vec![], generation: 0, targets: vec![] }
    }

    /// Draw every `scene3d` element of `scene` off-screen and resolve it (DESIGN.md §1.18), unless it
    /// is exactly what its target already holds.
    fn prepare_meshes(&mut self, enc: &mut wgpu::CommandEncoder, scene: &Scene, lib: &crate::mesh::Library) {
        if scene.meshes.is_empty() { return; }
        if self.mesh.generation != lib.generation {
            let mut all: Vec<[f32; 6]> = vec![];
            self.mesh.ranges = lib.all().iter().map(|m| { let first = all.len() as u32; all.extend(m.vertices()); (first, all.len() as u32 - first) }).collect();
            self.mesh.verts = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&all), usage: wgpu::BufferUsages::VERTEX });
            self.mesh.generation = lib.generation;
            for t in &mut self.mesh.targets { t.drawn = None; }
        }
        for (k, md) in scene.meshes.iter().enumerate() {
            let sc = &md.scene;
            let rs = sc.spec.resolution_scale.clamp(0.25, 1.0);
            let (rw, rh) = (md.rect[2].max(1.0), md.rect[3].max(1.0));
            let size = (((rw * rs).ceil() as u32).max(1), ((rh * rs).ceil() as u32).max(1));
            if self.mesh.targets.len() <= k || self.mesh.targets[k].size != size {
                let t = self.mesh_target(size);
                if self.mesh.targets.len() <= k { self.mesh.targets.push(t); } else { self.mesh.targets[k] = t; }
            }
            let key = (sc.fingerprint(), [rw as u32, rh as u32], lib.generation);
            if self.mesh.targets[k].drawn == Some(key) { continue; }
            self.mesh.targets[k].drawn = Some(key);
            let px = rw / size.0 as f32;
            let radius = ((SPREAD as f32 / px).ceil() + 1.0).clamp(2.0, 40.0);
            let cam = &sc.camera;
            let u = MeshUniforms {
                view: cam.view().0, proj: cam.proj(rw / rh).0,
                light: [sc.light.dir[0], sc.light.dir[1], sc.light.dir[2], sc.light.ambient.clamp(0.0, 1.0)],
                misc: [cam.near.max(1e-3), cam.far.max(cam.near + 1e-3), sc.spec.edge_depth.max(0.0), sc.spec.edge_normal.clamp(0.0, 180.0).to_radians().cos()],
                size: [size.0 as f32, size.1 as f32, px, radius], spread: [SPREAD as f32, 0.0, 0.0, 0.0],
            };
            let mut copies: Vec<MeshCopy> = Vec::with_capacity(sc.count());
            let mut draws: Vec<(u32, u32, u32, u32)> = vec![];
            for it in &sc.items {
                let Some(&(first, count)) = self.mesh.ranges.get(it.mesh.0 as usize) else { continue };
                let start = copies.len() as u32;
                for m in &it.transforms {
                    let n = copies.len() as u32 + 1;
                    copies.push(MeshCopy { m: m.cols(), paint: [it.paint.tone, it.paint.accent, it.paint.hue as f32, 0.0], id: [(n & 255) as f32, ((n >> 8) & 255) as f32, 0.0, 0.0] });
                }
                draws.push((first, count, start, copies.len() as u32));
            }
            let t = &mut self.mesh.targets[k];
            self.queue.write_buffer(&t.ubuf, 0, bytemuck::bytes_of(&u));
            if copies.len() > t.capacity {
                t.capacity = copies.len().next_power_of_two();
                t.copies = self.device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (t.capacity * std::mem::size_of::<MeshCopy>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            }
            if !copies.is_empty() { self.queue.write_buffer(&t.copies, 0, bytemuck::cast_slice(&copies)); }
            let t = &self.mesh.targets[k];
            {
                let att = |v| Some(wgpu::RenderPassColorAttachment { view: v, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } });
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mesh"), timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
                    color_attachments: &[att(&t.views[0]), att(&t.views[1]), att(&t.views[2])],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &t.views[3], depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }), stencil_ops: None }),
                });
                pass.set_pipeline(&self.mesh.pipes[0]);
                pass.set_bind_group(0, &t.groups[3], &[]);
                pass.set_vertex_buffer(0, self.mesh.verts.slice(..));
                pass.set_vertex_buffer(1, t.copies.slice(..));
                // Every group of copies in one call.
                for (first, count, a, b) in draws { if b > a { pass.draw(first..first + count, a..b); } }
            }
            for (pipe, (view, group)) in [(1, (4, 0)), (2, (5, 1)), (3, (6, 2))] {
                let mut pass = clear(enc, &t.views[view], wgpu::Color::TRANSPARENT);
                pass.set_pipeline(&self.mesh.pipes[pipe]);
                pass.set_bind_group(0, &t.groups[group], &[]);
                pass.draw(0..3, 0..1);
            }
        }
    }

    fn mesh_target(&self, size: (u32, u32)) -> MeshTarget {
        let d = &self.device;
        let tex = |format| Self::target(d, size.0, size.1, format);
        let views = [tex(GBUF), tex(GBUF), tex(GBUF), tex(DEPTH), tex(GBUF), tex(GBUF), tex(GBUF)];
        let ubuf = d.create_buffer(&wgpu::BufferDescriptor { label: None, size: std::mem::size_of::<MeshUniforms>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let group = |t: [&wgpu::TextureView; 4]| d.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &self.mesh.layout, entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(t[0]) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(t[1]) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(t[2]) },
            wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(t[3]) },
        ] });
        let b = &self.blank;
        // The mesh pass reads nothing; mask reads normal + depth and id; h the mask; v the mask, h, normal and id.
        let groups = [group([b, b, &views[1], &views[2]]), group([&views[4], b, b, b]), group([&views[4], &views[5], &views[1], &views[2]]), group([b, b, b, b])];
        let stage1 = Self::bind(d, &self.layout, &self.ubuf, &self.sampler, [&views[6], &views[0], &self.marks_view, &self.palette_view, &self.chars_view, b, b, b]);
        let capacity = 64;
        let copies = d.create_buffer(&wgpu::BufferDescriptor { label: None, size: (capacity * std::mem::size_of::<MeshCopy>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        MeshTarget { size, ubuf, copies, capacity, views, groups, stage1, drawn: None }
    }

    /// The bind group a batch of stage 1 draws with: its picture's, or its 3D element's resolve.
    fn batch_group(&self, picture: u32) -> Option<&wgpu::BindGroup> {
        if picture >= crate::scene::MESH_BATCH { return self.mesh.targets.get((picture - crate::scene::MESH_BATCH) as usize).map(|t| &t.stage1); }
        self.pictures.get(&picture).or_else(|| self.pictures.get(&u32::MAX))
    }

    fn tile_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, ubuf: &wgpu::Buffer, snaps: &[wgpu::TextureView; 4]) -> wgpu::BindGroup {
        let mut entries = vec![wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() }];
        // Bindings 1–4: source ink, target ink, source near, target near.
        for (k, v) in [&snaps[0], &snaps[1], &snaps[2], &snaps[3]].into_iter().enumerate() { entries.push(wgpu::BindGroupEntry { binding: k as u32 + 1, resource: wgpu::BindingResource::TextureView(v) }); }
        device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &entries })
    }
    fn particle_buffer(device: &wgpu::Device, n: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (n * std::mem::size_of::<Particle>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
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
    fn bind(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, ubuf: &wgpu::Buffer, sampler: &wgpu::Sampler, t: [&wgpu::TextureView; 8]) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(t[0]) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(t[1]) },
            wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(t[2]) },
            wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(t[3]) },
            wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(t[4]) },
            wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(t[5]) },
            wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(t[6]) },
            wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::TextureView(t[7]) },
        ] })
    }

    /// Register picture `id` (RGBA8, `w` × `h`).
    pub fn set_picture(&mut self, id: u32, w: u32, h: u32, rgba: &[u8]) {
        let tex = self.device.create_texture_with_data(&self.queue, &wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Unorm, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
        }, wgpu::util::TextureDataOrder::LayerMajor, rgba);
        let view = tex.create_view(&Default::default());
        let group = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.atlas_view, &view, &self.marks_view, &self.palette_view, &self.chars_view, &self.blank, &self.blank, &self.blank]);
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
        self.soft = Self::target(&self.device, w, h, INK);
        self.material = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.ink, &self.near, &self.marks_view, &self.palette_view, &self.chars_view, &self.before, &self.soft, &self.blank]);
        self.material_first = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.ink, &self.near, &self.marks_view, &self.palette_view, &self.chars_view, &self.blank, &self.soft, &self.blank]);
        self.lo_key = (0, 0, 0);
        // The old frame is gone with the old size.
        self.split = [0.0; 4];
        let t = &mut self.tiles;
        t.snaps = [Self::target(&self.device, w, h, INK), Self::target(&self.device, w, h, INK), Self::target(&self.device, w, h, NEAR), Self::target(&self.device, w, h, NEAR)];
        t.group = Self::tile_group(&self.device, &t.layout, &t.ubuf, &t.snaps);
        t.seen = 0;
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

    /// The reduced target for quality `q` (its size), made when the size or quality changes.
    fn lo_for(&mut self, q: f32) -> (u32, u32) {
        let (w, h) = ((self.size.0 as f32 * q).ceil() as u32, (self.size.1 as f32 * q).ceil() as u32);
        let key = (w, h, (q * 1000.0) as u32);
        if self.up_pipe.is_none() {
            // After a reduced material pass: rest pixels at full resolution, loose ones from the reduced pass.
            self.up_pipe = Some(self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("fs_material_up"), layout: Some(&self.pl),
                vertex: wgpu::VertexState { module: &self.shader, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
                depth_stencil: None, multisample: Default::default(),
                fragment: Some(wgpu::FragmentState { module: &self.shader, entry_point: Some("fs_material_up"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: self.out_format, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
                multiview_mask: None, cache: None,
            }));
        }
        if key != self.lo_key {
            self.lo_key = key;
            self.lo = Self::target(&self.device, w, h, self.out_format);
            self.material_up = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.ink, &self.near, &self.marks_view, &self.palette_view, &self.chars_view, &self.before, &self.soft, &self.lo]);
        }
        (w, h)
    }

    pub fn draw<S, M: Clone>(&mut self, target: &wgpu::TextureView, ui: &mut Ui<S, M>, look: &Look) {
        let started = crate::perf::now_ms();
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
        let (scale, now, mt) = (ui.scale(), ui.now(), ui.material_clock());
        // A new commit: draw the frame as it was, in the look it had, into `before`, once.
        if let Some(sp) = ui.split() && sp.id != self.split_seen {
            self.split_seen = sp.id;
            if let Some(old) = self.last_look {
                let before = self.before.clone();
                self.frame(&before, Frame { scene: &sp.scene, field: &sp.field, look: &old, lib: ui.meshes(), scale, t: sp.t, mt, split: [0.0; 4], tiles: None }, false);
                self.split = [sp.at[0], sp.at[1], sp.t, 1.0];
            }
        }
        // The front has left the screen: the old frame is not needed any more.
        let (w, h) = (self.size.0 as f32, self.size.1 as f32);
        if self.split[3] > 0.0 && (now - self.split[2]) * look.fx.wave_speed * scale > (w * w + h * h).sqrt() { self.split = [0.0; 4]; }
        self.last_look = Some(*look);
        let split = self.split;
        // A morph: picture its two elements once, upload its tiles once per run.
        let mut tiles = None;
        if let Some(run) = ui.morph_run() {
            if run.id != self.tiles.seen {
                self.tiles.seen = run.id;
                for (insts, ink, near) in [(&run.src, 0, 2), (&run.dst, 1, 3)] {
                    let mut sc = Scene::default();
                    for i in insts { sc.push(*i); }
                    // A 3D element in the morph: its meshes as the page has them.
                    sc.meshes = ui.scene().meshes.clone();
                    let (iv, nv) = (self.tiles.snaps[ink].clone(), self.tiles.snaps[near].clone());
                    self.uniforms(&Field::default(), look, scale, now, mt, [0.0; 4], [1.0, 1.0, w, h]);
                    self.upload(&sc);
                    let mut enc = self.device.create_command_encoder(&Default::default());
                    self.prepare_meshes(&mut enc, &sc, ui.meshes());
                    self.stage1(&mut enc, &sc, &iv, &nv, None, 0);
                    self.queue.submit([enc.finish()]);
                }
            }
            if run.upload != self.tiles.uploaded {
                self.tiles.uploaded = run.upload;
                let t = &mut self.tiles;
                if run.particles.len() > t.capacity {
                    t.capacity = run.particles.len().next_power_of_two();
                    t.buf = Self::particle_buffer(&self.device, t.capacity);
                }
                self.queue.write_buffer(&t.buf, 0, bytemuck::cast_slice(&run.particles));
                t.count = run.particles.len() as u32;
            }
            // A picture on either side: its PicFx slot, as near.g stores it.
            let pic = run.src.iter().chain(&run.dst).find(|i| i.paint[3] == crate::scene::PICTURE).map_or(0.0, |i| (i.paint[1] + 1.0) / 8.0);
            let len = run.flight.map_or(0.0, |f| f[1] - f[0]);
            tiles = run.clock(now).map(|c| [c, run.spec.curl * scale, run.spec.ease.index() as f32, pic, len]);
        }
        // The material's resolution: the look's level, or the automatic one. The reduced pass runs only
        // when something can be loose (at rest every pixel is shaded at full resolution anyway).
        let auto = look.quality <= 0.0;
        let q = if auto { self.auto.quality() } else { crate::perf::LEVELS.iter().copied().min_by(|a, b| (a - look.quality).abs().total_cmp(&(b - look.quality).abs())).unwrap_or(1.0) };
        // A small loose area (a lens, one held control) costs less at full resolution than a second pass.
        let loose = tiles.is_some() || ui.loose_estimate(now, look.fx.lens) >= LO_MIN_AREA;
        let lo = (q < 1.0 && (loose || self.always_lo)).then(|| { let (w, h) = self.lo_for(q); [1.0 / q, q, w as f32, h as f32] });
        let pending = ui.perf().pending;
        self.frame_q(target, Frame { scene: ui.scene(), field: ui.field(), look, lib: ui.meshes(), scale, t: now, mt, split, tiles }, true, lo);
        let submit = (crate::perf::now_ms() - started) as f32;
        let parts = ui.morph_run().map_or(0, |r| if r.clock(now).is_some() { r.particles.len() as u32 } else { 0 });
        let p = ui.perf_mut();
        p.quality = q; p.auto = auto; p.particles = parts;
        p.frame(submit);
        if auto { self.auto.feed(pending.update_layout + pending.scene + submit); }
        ui.frame_drawn();
    }

    fn upload(&mut self, scene: &Scene) {
        if scene.instances.len() > self.capacity {
            self.capacity = scene.instances.len().next_power_of_two();
            self.instances = Self::instance_buffer(&self.device, self.capacity);
        }
        if self.hide_glyphs {
            // For acceptance checks: the same scene with every glyph transparent (nothing else moves).
            let v: Vec<_> = scene.instances.iter().map(|i| { let mut i = *i; if i.paint[3] < 0.5 { i.paint[2] = 0.0; } i }).collect();
            self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&v));
        } else {
            self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&scene.instances));
        }
    }

    /// Stage 1: every instance into ink and near. The page first, then `tiles` morph tiles (if any),
    /// then the modal layer.
    fn stage1(&self, enc: &mut wgpu::CommandEncoder, scene: &Scene, ink: &wgpu::TextureView, near: &wgpu::TextureView, soft: Option<&wgpu::TextureView>, tiles: u32) {
        let layer = scene.layer.unwrap_or(u32::MAX);
        for (k, (view, pipe, top)) in [(ink, &self.pipes[0], &self.pipes[6]), (near, &self.pipes[1], &self.pipes[2])].into_iter().enumerate() {
            let mut pass = clear(enc, view, wgpu::Color::TRANSPARENT);
            let mut pending = tiles > 0;
            let draw_tiles = |pass: &mut wgpu::RenderPass<'_>| {
                pass.set_pipeline(&self.tiles.pipes[k]);
                pass.set_bind_group(0, &self.tiles.group, &[]);
                pass.set_vertex_buffer(0, self.tiles.buf.slice(..));
                pass.draw(0..4, 0..tiles);
                pass.set_vertex_buffer(0, self.instances.slice(..));
            };
            pass.set_vertex_buffer(0, self.instances.slice(..));
            for b in &scene.batches {
                if pending && b.start >= layer { draw_tiles(&mut pass); pending = false; }
                pass.set_pipeline(if b.start >= layer { top } else { pipe });
                let Some(group) = self.batch_group(b.picture) else { continue };
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..4, b.start..b.end);
            }
            if pending { draw_tiles(&mut pass); }
        }
        // The liquid's field, a third time, only when liquid is in use.
        if let Some(view) = soft {
            let mut pass = clear(enc, view, wgpu::Color::TRANSPARENT);
            pass.set_vertex_buffer(0, self.instances.slice(..));
            pass.set_pipeline(&self.pipes[4]);
            // The modal layer is not liquid (see `fs_ink_top`).
            for b in scene.batches.iter().filter(|b| b.start < layer) {
                let Some(group) = self.batch_group(b.picture) else { continue };
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..4, b.start..b.end);
            }
        }
    }

    /// Both stages of one frame into `target`, submitted on its own so that each frame gets its own
    /// uniforms and instances. `with_before` binds the old frame (not when drawing into it).
    fn frame(&mut self, target: &wgpu::TextureView, f: Frame<'_>, with_before: bool) { self.frame_q(target, f, with_before, None); }
    /// `lo`: draw the material at reduced resolution first (`qual` for the uniforms), then upsample.
    fn frame_q(&mut self, target: &wgpu::TextureView, f: Frame<'_>, with_before: bool, lo: Option<[f32; 4]>) {
        let scene = f.scene;
        self.uniforms(f.field, f.look, f.scale, f.t, f.mt, f.split, lo.unwrap_or([1.0, 1.0, self.size.0 as f32, self.size.1 as f32]));
        self.upload(scene);
        let tiles = match f.tiles {
            Some([clock, curl, ease, pic, len]) if self.tiles.count > 0 => {
                let th = &f.look.theme;
                let c = |v: [f32; 3]| [v[0], v[1], v[2], 1.0];
                let u = PartUniforms { res: [self.size.0 as f32, self.size.1 as f32, f.scale, clock], m: [curl, ease, pic, len], bg: c(th.ground), fg: c(th.ink), accent: c(th.accent) };
                self.queue.write_buffer(&self.tiles.ubuf, 0, bytemuck::bytes_of(&u));
                self.tiles.count
            }
            _ => 0,
        };
        // Liquid needs its field: as the global material, or as any instance's override.
        let liquid = Material::Liquid.id() as u32;
        let wants_soft = f.look.material == Material::Liquid || scene.instances.iter().any(|i| i.look[0] >= 127.5 && (i.look[0] as u32) % 16 == liquid);
        let annots = scene.annot.len() as u32;
        if annots > 0 {
            if scene.annot.len() > self.annot_capacity {
                self.annot_capacity = scene.annot.len().next_power_of_two();
                self.annots = Self::instance_buffer(&self.device, self.annot_capacity);
            }
            self.queue.write_buffer(&self.annots, 0, bytemuck::cast_slice(&scene.annot));
        }
        let mut enc = self.device.create_command_encoder(&Default::default());
        self.prepare_meshes(&mut enc, scene, f.lib);
        let (ink, near, soft) = (self.ink.clone(), self.near.clone(), self.soft.clone());
        self.stage1(&mut enc, scene, &ink, &near, wants_soft.then_some(&soft), tiles);
        if lo.is_some() {
            let lo_view = self.lo.clone();
            let mut pass = clear(&mut enc, &lo_view, wgpu::Color::BLACK);
            pass.set_pipeline(&self.pipes[3]);
            pass.set_bind_group(0, if with_before { &self.material } else { &self.material_first }, &[]);
            pass.draw(0..3, 0..1);
        }
        {
            let mut pass = clear(&mut enc, target, wgpu::Color::BLACK);
            if lo.is_some() {
                pass.set_pipeline(self.up_pipe.as_ref().expect("made with the reduced target"));
                pass.set_bind_group(0, &self.material_up, &[]);
            } else {
                pass.set_pipeline(&self.pipes[3]);
                pass.set_bind_group(0, if with_before { &self.material } else { &self.material_first }, &[]);
            }
            pass.draw(0..3, 0..1);
            // Stage 3: the annotations, exact, over the material.
            if annots > 0 {
                pass.set_pipeline(&self.pipes[5]);
                pass.set_bind_group(0, self.pictures.get(&u32::MAX).expect("blank picture"), &[]);
                pass.set_vertex_buffer(0, self.annots.slice(..));
                pass.draw(0..4, 0..annots);
            }
        }
        self.queue.submit([enc.finish()]);
    }

    #[allow(clippy::too_many_arguments)]
    fn uniforms(&self, field: &Field, look: &Look, scale: f32, t: f32, mt: f32, split: [f32; 4], qual: [f32; 4]) {
        let (fx, th) = (&look.fx, &look.theme);
        let c = |v: [f32; 3]| [v[0], v[1], v[2], 1.0];
        let (bg, fg, accent) = (c(th.ground), c(th.ink), c(th.accent));
        let flag = |b: bool| if b { 1.0 } else { 0.0 };
        let mut pics = [[0.0; 4]; 56];
        for (i, p) in look.pics.iter().enumerate() { pics[i * 7..i * 7 + 7].copy_from_slice(&p.pack()); }
        let mut mats = [[0.0; 4]; 2 * MATERIAL_COUNT];
        for (i, m) in look.params.iter().enumerate() {
            mats[i * 2] = [m.0[0], m.0[1], m.0[2], m.0[3]];
            mats[i * 2 + 1] = [m.0[4], m.0[5], m.0[6], m.0[7]];
        }
        let u = Uniforms {
            res: [self.size.0 as f32, self.size.1 as f32, scale, t],
            fa: [fx.pitch, mt, fx.jitter, fx.glitch],
            fb: [fx.wave_speed, fx.wave_life, fx.wave_width, fx.wave_tail],
            fc: [fx.push, fx.lens, fx.enter, flag(fx.square)],
            fd: [look.material.id() as f32, flag(self.out_format.is_srgb()), SPREAD as f32, marks::PALETTES.len() as f32],
            bg, fg, accent, pointer: [field.pointer[0], field.pointer[1], field.pointer[2], fx.lens_hard], wipe: field.wipe, wipe_rect: field.wipe_rect, split, fronts: field.fronts, pics,
            ground2: [th.ground2[0], th.ground2[1], th.ground2[2], look.loose_scale], second: c(th.second), third: c(th.third),
            burst: field.burst, burst_t: field.burst_t, mats, calm: field.calm, qual, scopes: field.scopes, rip: [look.loose_max, 0.0, 0.0, 0.0],
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
