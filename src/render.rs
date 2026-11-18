//! wgpu: draws a `Scene` in two stages. Stage 1 draws every instance into the ink and near images
//! (the same geometry twice, since WebGL2 cannot blend two targets differently); stage 2 is one
//! full-screen material pass that reads them with the field.
use crate::marks::{self, Atlas};
use crate::morph::Particle;
use crate::scene::{Field, Instance, Scene};
use crate::style::{Look, MATERIAL_COUNT};
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
    /// Eight `Markr` slots, seven vec4s each.
    pics: [[f32; 4]; 56],
    /// The ground at the bottom of the window, and the looseness scale in w.
    ground2: [f32; 4],
    second: [f32; 4],
    third: [f32; 4],
    burst: [f32; 4],
    burst_t: [f32; 4],
    /// Every material's numbers, two vec4s each.
    mats: [[f32; 4]; 2 * crate::materials::MAX],
    calm: [[f32; 4]; 4],
    /// The material resolution: 1 / q for the reduced pass (1 otherwise), q, and its size.
    qual: [f32; 4],
    /// Each front's scope box (device px).
    scopes: [[f32; 4]; 8],
    /// `Look::loose_max`, what a transition loosens everything by, and two spare numbers.
    rip: [f32; 4],
    /// A preset's transition (`preset::Transition::pack`) and the elements its boxes find.
    tr: [[f32; 4]; 9],
    boxes: [[f32; 4]; 16],
    scrolls: [[f32; 4]; 8],
    scrollo: [[f32; 4]; 8],
    scrp: [f32; 4],
    scrq: [f32; 4],
    /// The finishing pass (`post::Post::pack`) and its palette.
    post: [[f32; 4]; 6],
    pal: [[f32; 4]; crate::post::PALETTE_MAX],
    /// The field map: cell and drift (device px), nodes across and down (0: all zero, not read).
    fmap: [f32; 4],
    /// A commit's scope (`El::commit_scope`), device px x0, y0, x1, y1 (empty: the whole window).
    sbox: [f32; 4],
    plate: [f32; 4],
    plates: [[f32; 4]; 4],
}

const INK: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const NEAR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// r, g: what text to read holds (`fs_hold`); b: where a picture of the page is, by its flag in the
/// near image; a: where the modal layer has a shape or a picture of its own.
const HOLD: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// When a plate's pixel was drawn last, in `PLATE_TICK`s of the plate clock, as 24 bits in RGB (A: drawn).
const PLATE_AGE: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// The plate clock's step, s (24 bits of it last 38 hours).
pub const PLATE_TICK: f32 = 1.0 / 120.0;

/// One plate's kept images and the group that lays them on (binding 2: ink, 3: near, 10: the ink's age,
/// 9: the nearness's age).
struct PlateTex { ink: wgpu::TextureView, near: wgpu::TextureView, age: wgpu::TextureView, near_age: wgpu::TextureView, bind: wgpu::BindGroup }
const ATLAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;

const MARKS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;
/// The reduced material pass runs only when at least this fraction of the window can be loose.
pub const LO_MIN_AREA: f32 = 0.15;

/// The particle pass's own uniforms (see `particles.wgsl`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PartUniforms {
    res: [f32; 4], m: [f32; 4], bg: [f32; 4], fg: [f32; 4], accent: [f32; 4],
    ga: [f32; 4], gb: [f32; 4], gd: [f32; 4], ge: [f32; 4], gp: [f32; 4], gf: [f32; 4], gw: [f32; 4], gt: [f32; 4], fronts: [[f32; 4]; 8],
}

/// How particles can be simulated here (DESIGN.md §1.10): a compute shader (WebGPU, native), a fragment
/// shader into float targets (WebGL2 with 32- or 16-bit float render targets), or not at all (the CPU
/// path, capped at `MorphSpec::count_max`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleTier { Compute, Float32, Float16, Cpu }
impl ParticleTier {
    pub fn name(self) -> &'static str { match self { ParticleTier::Compute => "gpu compute", ParticleTier::Float32 => "gpu rgba32f", ParticleTier::Float16 => "gpu rgba16f", ParticleTier::Cpu => "cpu (no float targets)" } }
}
/// What this adapter can do for particles.
pub fn particle_tier(adapter: &wgpu::Adapter) -> ParticleTier {
    let f = |fmt| adapter.get_texture_format_features(fmt).allowed_usages;
    let compute = adapter.get_downlevel_capabilities().flags.contains(wgpu::DownlevelFlags::COMPUTE_SHADERS) && adapter.limits().max_storage_textures_per_shader_stage > 0
        && f(wgpu::TextureFormat::Rgba32Float).contains(wgpu::TextureUsages::STORAGE_BINDING);
    if compute { ParticleTier::Compute }
    else if f(wgpu::TextureFormat::Rgba32Float).contains(wgpu::TextureUsages::RENDER_ATTACHMENT) { ParticleTier::Float32 }
    else if f(wgpu::TextureFormat::Rgba16Float).contains(wgpu::TextureUsages::RENDER_ATTACHMENT) { ParticleTier::Float16 }
    else { ParticleTier::Cpu }
}

/// The GPU path's state: two textures of per-tile offset and velocity, one step after the other.
struct Sim {
    tier: ParticleTier,
    states: [wgpu::TextureView; 2],
    width: u32,
    cur: usize,
    steps: u32,
    /// The morph and the run (a reversal is a new run) the state belongs to.
    run: (u32, u32),
    frag: Option<wgpu::RenderPipeline>,
    compute: Option<(wgpu::ComputePipeline, wgpu::BindGroupLayout)>,
}
/// One step of the simulation: 1/60 s, so a fixed clock gives the same state.
pub const SIM_DT: f32 = 1.0 / 60.0;

/// A morph's tiles on the GPU: the stage-1 pictures of its two elements (source ink, target ink,
/// source near, target near), drawn once per morph, and the tile buffer, uploaded once per run.
struct Tiles {
    layout: wgpu::BindGroupLayout,
    /// Their pipelines (`Tiles::pipes` and the grid's `gpipes`), made the first time a morph flies or
    /// by the warm-up, and the layout they are made with.
    pipes: Option<[wgpu::RenderPipeline; 2]>,
    pl: wgpu::PipelineLayout,
    ubuf: wgpu::Buffer,
    snaps: [wgpu::TextureView; 4],
    group: wgpu::BindGroup,
    buf: wgpu::Buffer,
    capacity: usize,
    count: u32,
    /// Which morph the pictures are of, and which run the buffer holds.
    seen: u32,
    uploaded: u32,
    /// Grid tiles (the GPU path): their pipelines, the state texture a stage-1 pass reads (when a grid
    /// is drawn this frame: its bind group and count), an empty state for the CPU path.
    gpipes: Option<[wgpu::RenderPipeline; 2]>,
    grid_now: Option<(wgpu::BindGroup, u32)>,
    no_state: wgpu::TextureView,
    sim: Option<Sim>,
    tier: ParticleTier,
}

pub struct Renderer {
    /// Draw every glyph transparent (for acceptance checks that need to know which pixels are text).
    pub hide_glyphs: bool,
    /// How many plates the last frame laid on (two passes each: ink and nearness), for tests.
    pub plates_laid: std::cell::Cell<u32>,
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
    /// Where text to read holds the looseness under and around it to the cap (R8, max-blended; scene.rs
    /// `Scene::holds`, DESIGN.md §1.15), its pipeline, and the instances drawn into it this frame.
    hold: wgpu::TextureView,
    hold_pipe: wgpu::RenderPipeline,
    holds: wgpu::Buffer,
    hold_cap: usize,
    hold_n: u32,
    /// The field map's texture (`fieldmap::MAX_NODES` square; the map's nodes in its top left), the
    /// bytes last uploaded, and what the uniforms say about it this frame.
    fmap: wgpu::Texture,
    fmap_view: wgpu::TextureView,
    fmap_seen: Option<std::rc::Rc<Vec<u8>>>,
    fmap_u: [f32; 4],
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
    /// The pictures' textures, so their groups can be made again when the hold image is (`resize`).
    picture_views: HashMap<u32, wgpu::TextureView>,
    /// The group the hold image is drawn with (the glyph atlas; it does not read the hold image).
    hold_group: Option<wgpu::BindGroup>,
    instances: wgpu::Buffer,
    capacity: usize,
    pipes: [wgpu::RenderPipeline; 6], // ink, near, near for the modal layer, material, annotations, ink for the modal layer
    /// The upsample after a reduced material pass: made the first time a quality below 1 is used (it holds
    /// the whole material again, and compiling it at start-up is slow on a CPU rasteriser).
    up_pipe: Option<wgpu::RenderPipeline>,
    shader: wgpu::ShaderModule,
    pl: wgpu::PipelineLayout,
    ink: wgpu::TextureView,
    near: wgpu::TextureView,
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
    /// Never read the field map (the check's control for DESIGN.md §1.29: with the map all zero, a
    /// frame is the same with or without it).
    pub no_map: bool,
    size: (u32, u32),
    tiles: Tiles,
    mesh: MeshGpu,
    /// Rows of the particle state textures.
    sim_rows: u32,
    /// The boxes the frame being drawn is limited to.
    region: crate::dirty::Region,
    /// Draw only what changed (DESIGN.md §1.19): the frame is kept here between draws and copied to the
    /// target; what the last frame was drawn from; zeroing a box of an image; the copy.
    pub partial: bool,
    frame_view: wgpu::TextureView,
    prev: Option<crate::dirty::Inputs>,
    /// The lists of the frame before `prev`, kept to be filled again (no allocation per frame).
    spare: (Vec<Instance>, Vec<Instance>),
    zero: [wgpu::RenderPipeline; 2],
    blit: wgpu::RenderPipeline,
    /// A see-through group (`El::opacity`): its own ink image, and the pass that lays it on the ink.
    group: Option<(wgpu::TextureView, wgpu::BindGroup)>,
    /// Each plate's kept images (`El::plate`, DESIGN.md §1.33): ink, near, and when each pixel was drawn
    /// last; made when a slot is first used, emptied by `Cmd::ClearPlate` and a new window size.
    plates: [Option<PlateTex>; crate::scene::PLATES],
    /// Drawing into a plate, its age and laying it on the ink and the near image.
    plate_pipes: [wgpu::RenderPipeline; 4],
    /// The plate clock in `PLATE_TICK`s, and each slot's fade (`Plate::fade_s`, in ticks; 0: none).
    plate_now: f32,
    plate_fade: [[f32; 4]; 4],
    /// How many plates the last frame had (a plate that comes or goes redraws everything).
    had_plates: usize,
    /// Each plate slot has had ink drawn into it since it was made or cleared: an empty plate is not laid on.
    plate_inked: [bool; 4],
    /// The last frame had ink ageing on a plate (the frame after it shows where the ageing ended).
    was_ageing: bool,
    /// The last plate clearing applied (`Scene::plate_serial`).
    plate_serial: u64,
    group_pipe: wgpu::RenderPipeline,
    /// The last frame had see-through groups (the next is drawn whole).
    had_groups: bool,
    /// The viewer asked for less motion (`Ui::reduced_motion`): new instances fade in, not condense.
    reduced: bool,
    /// How long that fade is, seconds (`Ui::reduced_fade`).
    fade_s: f32,
    /// The running commit's scope (`El::commit_scope`), device px; empty for the whole window.
    sbox: [f32; 4],
    /// The finishing pass in place of the blit when the look has one (DESIGN.md §1.21).
    /// Made by the warm-up (or the first frame that needs it).
    post: Option<wgpu::RenderPipeline>,
    blit_group: wgpu::BindGroup,
    /// The last frame's two boxes (for the readout and the bench); `None`: all of it.
    pub last_region: Option<([u32; 4], [u32; 4])>,
    /// What is still to prepare (`Renderer::begin`, `Warmup`): while there is any, frames are drawn as
    /// plain (DESIGN.md §1.23). `None`: ready.
    warm: Option<warm::Pending>,
    /// The warm-up has just finished: the next frame condenses into the look.
    condense: bool,
    /// The transition of the commit in progress and the boxes it finds (uniforms), and its looseness.
    tr: [[f32; 4]; 9],
    boxes: [[f32; 4]; 16],
    tr_loose: f32,
    /// The beat clock off the grid this frame (`Ui::beat_time`): what condenses goes on between steps.
    t_cont: f32,
}

/// The mesh pass's uniforms (see `mesh.wgsl`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshUniforms { view: [f32; 16], proj: [f32; 16], light: [f32; 4], misc: [f32; 4], size: [f32; 4], spread: [f32; 4], depth: [f32; 4] }

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

/// The largest vertex buffer the mesh pass asks for, bytes: the library's vertices go in as many as
/// they need (some adapters refuse a buffer under a megabyte; SwiftShader refused 760 176 bytes).
pub const MESH_CHUNK: usize = 256 * 1024;

/// The mesh pass: its pipelines, the library's meshes in vertex buffers of at most `MESH_CHUNK`, and a
/// target per element.
struct MeshGpu {
    layout: wgpu::BindGroupLayout,
    /// mesh, mask, h, v: made the first time a 3D element is drawn or by the warm-up, with `pl`.
    pipes: Option<[wgpu::RenderPipeline; 4]>,
    pl: wgpu::PipelineLayout,
    verts: Vec<wgpu::Buffer>,
    /// Each mesh's pieces: the buffer, its first vertex and how many.
    ranges: Vec<MeshPieces>,
    generation: u32,
    targets: Vec<MeshTarget>,
}

/// What one frame is drawn from. `tiles`: a morph's tiles are flying, this many seconds in, with this
/// curl (device px), ease, the picture flag of the morph's picture (0 if none) and the flight's length.
struct Frame<'a> { scene: &'a Scene, field: &'a Field, look: &'a Look, lib: &'a crate::mesh::Library, scale: f32, t: f32, mt: [f32; 2], split: [f32; 4], tiles: Option<[f32; 5]>, grid: Option<(crate::morph::Grid, f32, f32)> }

#[path = "render_warm.rs"]
mod warm;
pub use warm::{Progress, Stage, WARM_MS, Warmup, plain};

impl Renderer {
    /// A renderer with everything made at once (offline rendering, tests): the same frames, byte for
    /// byte, as one warmed up by `Renderer::begin` and `Warmup::step` to the end. The mark atlas is
    /// made on the first draw (it needs the text system).
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, out_format: wgpu::TextureFormat, w: u32, h: u32) -> Self {
        let mut r = Self::core(device, queue, out_format, w, h);
        r.ensure_post();
        r.ensure_tiles();
        r.ensure_mesh();
        r
    }
    /// The pipelines that plain needs (ink, nearness, the material pass, annotations, the copy out),
    /// the textures and buffers: enough to draw a frame.
    fn core(device: wgpu::Device, queue: wgpu::Queue, out_format: wgpu::TextureFormat, w: u32, h: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(crate::materials::shader().into()) });
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding, visibility: wgpu::ShaderStages::FRAGMENT, count: None,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[
            wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, count: None, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None } },
            wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, count: None, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering) },
            tex_entry(2), tex_entry(3), tex_entry(4), tex_entry(5), tex_entry(6), tex_entry(7), tex_entry(9), tex_entry(10),
            // The field map is read in the vertex stage too (its flow pushes instances), by textureLoad.
            wgpu::BindGroupLayoutEntry { binding: 8, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, count: None, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false } },
        ] });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let inst_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64, step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4],
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
        #[cfg(not(target_arch = "wasm32"))]
        let timing = std::env::var_os("AMENUI_TIMING").is_some();
        #[cfg(not(target_arch = "wasm32"))]
        let pipe = |vs: &str, fs: &str, format, blend: Option<wgpu::BlendState>, inst: bool| {
            let t0 = std::time::Instant::now();
            let p = pipe(vs, fs, format, blend, inst);
            if timing { eprintln!("pipeline {fs}: {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0); }
            p
        };
        let pipes = [
            pipe("vs_inst", "fs_ink", INK, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), true),
            pipe("vs_inst", "fs_near", NEAR, Some(wgpu::BlendState { color: maxb, alpha: maxb }), true),
            // The modal layer replaces what is beneath it where it has ink, so its card is not loose and
            // pictures under it do not show through as pictures.
            pipe("vs_inst", "fs_near_top", NEAR, None, true),
            pipe("vs_full", "fs_material", out_format, None, false),
            pipe("vs_annot", "fs_annot", out_format, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), true),
            pipe("vs_inst", "fs_ink_top", INK, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), true),
        ];
        let hold_pipe = pipe("vs_hold", "fs_hold", HOLD, Some(wgpu::BlendState { color: maxb, alpha: maxb }), true);
        let tiles = Self::tiles(&device);
        let mesh = Self::mesh_gpu(&device);
        let zero = [pipe("vs_full", "fs_zero", INK, None, false), pipe("vs_full", "fs_zero", out_format, None, false)];
        let blit = pipe("vs_full", "fs_blit", out_format, None, false);
        let group_pipe = pipe("vs_group", "fs_group", INK, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), false);
        let plate_pipes = [
            pipe("vs_inst", "fs_plate_age", PLATE_AGE, None, true),
            pipe("vs_group", "fs_plate_ink", INK, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), false),
            pipe("vs_group", "fs_plate_near", NEAR, Some(wgpu::BlendState { color: maxb, alpha: maxb }), false),
            pipe("vs_inst", "fs_plate_age_near", PLATE_AGE, None, true),
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
        let n = crate::fieldmap::MAX_NODES as u32;
        let fmap = device.create_texture_with_data(&queue, &wgpu::TextureDescriptor {
            label: Some("field map"), size: wgpu::Extent3d { width: n, height: n, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Unorm, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
        }, wgpu::util::TextureDataOrder::LayerMajor, &[0u8, 128, 128, 0].repeat((n * n) as usize));
        let fmap_view = fmap.create_view(&Default::default());
        let material = Self::bind(&device, &layout, &ubuf, &sampler, [&blank, &blank, &marks_view, &palette_view, &chars_view, &blank, &blank, &fmap_view, &blank]);
        let holds = Self::instance_buffer(&device, 64);
        let annots = Self::instance_buffer(&device, 256);
        let (blank_view, blank_group) = (blank.clone(), material.clone());
        let mut r = Self {
            device, queue, out_format, layout, ubuf, sampler, atlas, atlas_view, marks, marks_view, palette_view, chars, chars_view,
            before: blank.clone(), blank: blank.clone(), hold: blank.clone(), hold_pipe, holds, hold_cap: 64, hold_n: 0, fmap, fmap_view: fmap_view.clone(), fmap_seen: None, fmap_u: [0.0; 4], split_seen: 0, split: [0.0; 4], last_look: None, marks_ready: false, hide_glyphs: false, plates_laid: std::cell::Cell::new(0),
            pictures: HashMap::new(), picture_views: HashMap::new(), hold_group: None, instances, capacity, pipes, annots, annot_capacity: 256, up_pipe: None, shader, pl,
            ink: blank.clone(), lo: blank.clone(), lo_key: (0, 0, 0), material_up: material.clone(), auto: Default::default(), always_lo: false, no_map: false,
            near: blank, material_first: material.clone(), material, size: (0, 0), tiles, mesh, sim_rows: 0,
            region: crate::dirty::Region::All, partial: true, frame_view: blank_view.clone(), prev: None, spare: (vec![], vec![]), zero, blit, group: None, group_pipe, had_groups: false, plates: Default::default(), plate_pipes, plate_now: 0.0, plate_fade: [[0.0; 4]; 4], had_plates: 0, plate_inked: [false; 4], was_ageing: false, plate_serial: 0, reduced: false, fade_s: 0.15, sbox: [0.0; 4], post: None, blit_group: blank_group, warm: None, condense: false, last_region: None, tr: [[0.0; 4]; 9], boxes: [[0.0; 4]; 16], tr_loose: 0.0, t_cont: 0.0,
        };
        r.set_picture(u32::MAX, 1, 1, &[0, 0, 0, 255]);
        r.resize(w, h);
        r
    }

    fn tiles(device: &wgpu::Device) -> Tiles {
        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding, visibility: wgpu::ShaderStages::FRAGMENT, count: None,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[
            wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, count: None, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None } },
            tex(1), tex(2), tex(3), tex(4),
            wgpu::BindGroupLayoutEntry { binding: 5, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE, count: None,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false } },
        ] });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: std::mem::size_of::<PartUniforms>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let snaps = std::array::from_fn(|i| Self::target(device, 1, 1, if i < 2 { INK } else { NEAR }));
        let no_state = device.create_texture(&wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba32Float, usage: wgpu::TextureUsages::TEXTURE_BINDING, view_formats: &[],
        }).create_view(&Default::default());
        let group = Self::tile_group(device, &layout, &ubuf, &snaps, &no_state);
        let capacity = 1024;
        let buf = Self::particle_buffer(device, capacity);
        Tiles { layout, pipes: None, pl, ubuf, snaps, group, buf, capacity, count: 0, seen: 0, uploaded: 0, gpipes: None, grid_now: None, no_state, sim: None, tier: ParticleTier::Cpu }
    }
    /// The morph tiles' pipelines (CPU tiles and the GPU grid's), from their own shader.
    fn tile_pipes(device: &wgpu::Device, pl: &wgpu::PipelineLayout) -> ([wgpu::RenderPipeline; 2], [wgpu::RenderPipeline; 2]) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(include_str!("particles.wgsl").into()) });
        let bufs = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Particle>() as u64, step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
        })];
        let maxb = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Max };
        let pipe = |fs: &str, format, blend| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fs), layout: Some(pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_part"), compilation_options: Default::default(), buffers: &bufs },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None, multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some(fs), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })] }),
            multiview_mask: None, cache: None,
        });
        let pipes = [pipe("fs_part_ink", INK, wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), pipe("fs_part_near", NEAR, wgpu::BlendState { color: maxb, alpha: maxb })];
        let gpipe = |fs: &str, format, blend| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fs), layout: Some(pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_grid"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None, multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some(fs), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })] }),
            multiview_mask: None, cache: None,
        });
        let gpipes = [gpipe("fs_part_ink", INK, wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), gpipe("fs_part_near", NEAR, wgpu::BlendState { color: maxb, alpha: maxb })];
        (pipes, gpipes)
    }
    fn mesh_gpu(device: &wgpu::Device) -> MeshGpu {
        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding, visibility: wgpu::ShaderStages::FRAGMENT, count: None,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[
            wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, count: None, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None } },
            tex(1), tex(2), tex(3), tex(4),
        ] });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        MeshGpu { layout, pipes: None, pl, verts: vec![], ranges: vec![], generation: 0, targets: vec![] }
    }

    /// The mesh pass's pipelines, from its own shader.
    fn mesh_pipes(device: &wgpu::Device, pl: &wgpu::PipelineLayout) -> [wgpu::RenderPipeline; 4] {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(include_str!("mesh.wgsl").into()) });
        let bufs = [
            Some(wgpu::VertexBufferLayout { array_stride: 24, step_mode: wgpu::VertexStepMode::Vertex, attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3] }),
            Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<MeshCopy>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4] }),
        ];
        let target = |format| Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL });
        let mesh = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fs_mesh"), layout: Some(pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_mesh"), compilation_options: Default::default(), buffers: &bufs },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: Some(true), depth_compare: Some(wgpu::CompareFunction::Less), stencil: Default::default(), bias: Default::default() }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_mesh"), compilation_options: Default::default(), targets: &[target(GBUF), target(GBUF), target(GBUF)] }),
            multiview_mask: None, cache: None,
        });
        let full = |fs: &str| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fs), layout: Some(pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some(fs), compilation_options: Default::default(), targets: &[target(GBUF)] }),
            multiview_mask: None, cache: None,
        });
        [mesh, full("fs_mask"), full("fs_h"), full("fs_v")]
    }
    /// Draw every `scene3d` element of `scene` off-screen and resolve it (DESIGN.md §1.18), unless it
    /// is exactly what its target already holds.
    fn prepare_meshes(&mut self, enc: &mut wgpu::CommandEncoder, scene: &Scene, lib: &crate::mesh::Library, marks: crate::style::Marks) {
        if scene.meshes.is_empty() { return; }
        self.ensure_mesh();
        if self.mesh.generation != lib.generation {
            let cap = MESH_CHUNK.min(self.device.limits().max_buffer_size as usize);
            let (chunks, ranges) = mesh_chunks(lib.all().iter().map(|m| m.vertices()), cap / std::mem::size_of::<[f32; 6]>());
            self.mesh.ranges = ranges;
            self.mesh.verts = chunks.iter().map(|c| self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(c), usage: wgpu::BufferUsages::VERTEX })).collect();
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
            let key = (sc.fingerprint() ^ (marks.depth_scale.to_bits() as u64) << 7 ^ (marks.depth_dim.to_bits() as u64) << 29, [rw as u32, rh as u32], lib.generation);
            if self.mesh.targets[k].drawn == Some(key) { continue; }
            self.mesh.targets[k].drawn = Some(key);
            let px = rw / size.0 as f32;
            let radius = ((SPREAD as f32 / px).ceil() + 1.0).clamp(2.0, 40.0);
            let cam = &sc.camera;
            let u = MeshUniforms {
                view: cam.view().0, proj: cam.proj(rw / rh).0,
                light: [sc.light.dir[0], sc.light.dir[1], sc.light.dir[2], sc.light.ambient.clamp(0.0, 1.0)],
                misc: [cam.near.max(1e-3), cam.far.max(cam.near + 1e-3), sc.spec.edge_depth.max(0.0), sc.spec.edge_normal.clamp(0.0, 180.0).to_radians().cos()],
                size: [size.0 as f32, size.1 as f32, px, radius], spread: [SPREAD as f32, sc.spec.tones, sc.spec.terminator.clamp(0.0, 1.0), 0.0],
                depth: { let (z0, z1) = if marks.depth_scale != 0.0 || marks.depth_dim != 0.0 { sc.depth_range(lib) } else { (0.0, 1.0) }; [marks.depth_scale.clamp(0.0, 1.0), marks.depth_dim.clamp(0.0, 1.0), z0, z1] },
            };
            let mut copies: Vec<MeshCopy> = Vec::with_capacity(sc.count());
            let mut draws: Vec<(u32, u32, u32, u32, u32)> = vec![];
            for it in &sc.items {
                let Some(parts) = self.mesh.ranges.get(it.mesh.0 as usize) else { continue };
                let start = copies.len() as u32;
                for m in &it.transforms {
                    let n = copies.len() as u32 + 1;
                    copies.push(MeshCopy { m: m.cols(), paint: [it.paint.tone, it.paint.accent, it.paint.hue as f32, 0.0], id: [(n & 255) as f32, ((n >> 8) & 255) as f32, 0.0, 0.0] });
                }
                for &(chunk, first, count) in parts { draws.push((chunk, first, count, start, copies.len() as u32)); }
            }
            let t = &mut self.mesh.targets[k];
            self.queue.write_buffer(&t.ubuf, 0, bytemuck::bytes_of(&u));
            if copies.len() > t.capacity {
                t.capacity = copies.len().next_power_of_two();
                t.copies = self.device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (t.capacity * std::mem::size_of::<MeshCopy>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            }
            if !copies.is_empty() { self.queue.write_buffer(&t.copies, 0, bytemuck::cast_slice(&copies)); }
            let t = &self.mesh.targets[k];
            let mesh_pipes = self.mesh.pipes.as_ref().expect("made at the start of this pass");
            {
                let att = |v| Some(wgpu::RenderPassColorAttachment { view: v, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } });
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mesh"), timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
                    color_attachments: &[att(&t.views[0]), att(&t.views[1]), att(&t.views[2])],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &t.views[3], depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }), stencil_ops: None }),
                });
                pass.set_pipeline(&mesh_pipes[0]);
                pass.set_bind_group(0, &t.groups[3], &[]);
                pass.set_vertex_buffer(1, t.copies.slice(..));
                // Every group of copies in one call per piece of its mesh.
                let mut bound = None;
                for (chunk, first, count, a, b) in draws {
                    if b <= a { continue; }
                    if bound != Some(chunk) { pass.set_vertex_buffer(0, self.mesh.verts[chunk as usize].slice(..)); bound = Some(chunk); }
                    pass.draw(first..first + count, a..b);
                }
            }
            for (pipe, (view, group)) in [(1, (4, 0)), (2, (5, 1)), (3, (6, 2))] {
                let mut pass = clear(enc, &t.views[view], wgpu::Color::TRANSPARENT);
                pass.set_pipeline(&mesh_pipes[pipe]);
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
        let stage1 = Self::bind(d, &self.layout, &self.ubuf, &self.sampler, [&views[6], &views[0], &self.marks_view, &self.palette_view, &self.chars_view, b, b, &self.fmap_view, b]);
        let capacity = 64;
        let copies = d.create_buffer(&wgpu::BufferDescriptor { label: None, size: (capacity * std::mem::size_of::<MeshCopy>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        MeshTarget { size, ubuf, copies, capacity, views, groups, stage1, drawn: None }
    }

    /// The bind group a batch of stage 1 draws with: its picture's, or its 3D element's resolve.
    fn batch_group(&self, picture: u32) -> Option<&wgpu::BindGroup> {
        if picture >= crate::scene::MESH_BATCH { return self.mesh.targets.get((picture - crate::scene::MESH_BATCH) as usize).map(|t| &t.stage1); }
        self.pictures.get(&picture).or_else(|| self.pictures.get(&u32::MAX))
    }

    fn tile_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, ubuf: &wgpu::Buffer, snaps: &[wgpu::TextureView; 4], state: &wgpu::TextureView) -> wgpu::BindGroup {
        let mut entries = vec![wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() }];
        // Bindings 1–4: source ink, target ink, source near, target near; 5 the simulation's state.
        for (k, v) in [&snaps[0], &snaps[1], &snaps[2], &snaps[3], state].into_iter().enumerate() { entries.push(wgpu::BindGroupEntry { binding: k as u32 + 1, resource: wgpu::BindingResource::TextureView(v) }); }
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
    fn bind(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, ubuf: &wgpu::Buffer, sampler: &wgpu::Sampler, t: [&wgpu::TextureView; 9]) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(t[0]) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(t[1]) },
            wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(t[2]) },
            wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(t[3]) },
            wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(t[4]) },
            wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(t[5]) },
            wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::TextureView(t[6]) },
            wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(t[7]) },
            wgpu::BindGroupEntry { binding: 10, resource: wgpu::BindingResource::TextureView(t[8]) },
        ] })
    }

    /// Register picture `id` (RGBA8, `w` × `h`).
    pub fn set_picture(&mut self, id: u32, w: u32, h: u32, rgba: &[u8]) {
        let tex = self.device.create_texture_with_data(&self.queue, &wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Unorm, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
        }, wgpu::util::TextureDataOrder::LayerMajor, rgba);
        let view = tex.create_view(&Default::default());
        let group = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.atlas_view, &view, &self.marks_view, &self.palette_view, &self.chars_view, &self.blank, &self.blank, &self.fmap_view, &self.hold]);
        self.pictures.insert(id, group);
        self.picture_views.insert(id, view);
    }

    /// Device px.
    pub fn resize(&mut self, w: u32, h: u32) {
        let (w, h) = (w.max(1), h.max(1));
        if (w, h) == self.size { return; }
        self.size = (w, h);
        self.ink = Self::target(&self.device, w, h, INK);
        self.group = None;
        // A new window size empties every plate.
        self.plates = Default::default();
        self.near = Self::target_with(&self.device, w, h, NEAR, wgpu::TextureUsages::COPY_SRC);
        self.hold = Self::target(&self.device, w, h, HOLD);
        // Stage 1 reads the hold image (a shape scribblr fades keeps its fill where text holds it).
        for (id, view) in &self.picture_views {
            let g = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.atlas_view, view, &self.marks_view, &self.palette_view, &self.chars_view, &self.blank, &self.blank, &self.fmap_view, &self.hold]);
            self.pictures.insert(*id, g);
        }
        self.hold_group = self.picture_views.get(&u32::MAX).map(|v| Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.atlas_view, v, &self.marks_view, &self.palette_view, &self.chars_view, &self.blank, &self.blank, &self.fmap_view, &self.blank]));
        self.before = Self::target(&self.device, w, h, self.out_format);
        self.material = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.ink, &self.near, &self.marks_view, &self.palette_view, &self.chars_view, &self.before, &self.blank, &self.fmap_view, &self.hold]);
        self.material_first = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.ink, &self.near, &self.marks_view, &self.palette_view, &self.chars_view, &self.blank, &self.blank, &self.fmap_view, &self.hold]);
        self.lo_key = (0, 0, 0);
        let frame = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("frame"), size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format: self.out_format, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, view_formats: &[],
        });
        self.frame_view = frame.create_view(&Default::default());
        self.blit_group = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.frame_view, &self.blank, &self.marks_view, &self.palette_view, &self.chars_view, &self.blank, &self.blank, &self.fmap_view, &self.blank]);
        self.prev = None;
        // The old frame is gone with the old size.
        self.split = [0.0; 4];
        let t = &mut self.tiles;
        t.snaps = [Self::target(&self.device, w, h, INK), Self::target(&self.device, w, h, INK), Self::target(&self.device, w, h, NEAR), Self::target(&self.device, w, h, NEAR)];
        t.group = Self::tile_group(&self.device, &t.layout, &t.ubuf, &t.snaps, &t.no_state);
        t.seen = 0;
    }

    /// Say what the adapter can do for particles (`particle_tier`); until then morphs stay on the CPU.
    pub fn set_particle_tier(&mut self, tier: ParticleTier) { self.tiles.tier = tier; }
    pub fn particle_tier(&self) -> ParticleTier { self.tiles.tier }

    /// The simulation's state for `count` tiles, made (or grown) when needed.
    fn sim_for(&mut self, count: u32) {
        let tier = self.tiles.tier;
        let width = count.clamp(1, 1024);
        let rows = count.div_ceil(width).max(1);
        // The state is read by index (row by row, `width` wide), so a big enough one serves any smaller count.
        if self.tiles.sim.as_ref().is_some_and(|s| s.width * self.sim_rows() >= count) { return; }
        let format = if tier == ParticleTier::Float16 { wgpu::TextureFormat::Rgba16Float } else { wgpu::TextureFormat::Rgba32Float };
        let usage = wgpu::TextureUsages::TEXTURE_BINDING | if tier == ParticleTier::Compute { wgpu::TextureUsages::STORAGE_BINDING } else { wgpu::TextureUsages::RENDER_ATTACHMENT };
        let tex = || self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("particle state"), size: wgpu::Extent3d { width, height: rows, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format, usage: usage | wgpu::TextureUsages::COPY_DST, view_formats: &[],
        }).create_view(&Default::default());
        let states = [tex(), tex()];
        let (frag, compute) = match self.tiles.sim.take() {
            Some(s) => (s.frag, s.compute),
            None => (None, None),
        };
        let pl = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&self.tiles.layout)], immediate_size: 0 });
        let frag = frag.or_else(|| (tier != ParticleTier::Compute).then(|| {
            let shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(include_str!("particles.wgsl").into()) });
            self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("fs_sim"), layout: Some(&pl),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_sim"), compilation_options: Default::default(), buffers: &[] },
                primitive: Default::default(), depth_stencil: None, multisample: Default::default(),
                fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_sim"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
                multiview_mask: None, cache: None,
            })
        }));
        let compute = compute.or_else(|| (tier == ParticleTier::Compute).then(|| {
            let src = format!("{}\n{}", include_str!("particles.wgsl"), include_str!("sim_cs.wgsl"));
            let shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(src.into()) });
            let tex = |binding| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, count: None,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false } };
            let layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::COMPUTE, count: None, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None } },
                tex(1), tex(2), tex(3), tex(4),
                wgpu::BindGroupLayoutEntry { binding: 5, visibility: wgpu::ShaderStages::COMPUTE, count: None, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false } },
                wgpu::BindGroupLayoutEntry { binding: 6, visibility: wgpu::ShaderStages::COMPUTE, count: None, ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: wgpu::TextureFormat::Rgba32Float, view_dimension: wgpu::TextureViewDimension::D2 } },
            ] });
            let cpl = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
            (self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("cs_sim"), layout: Some(&cpl), module: &shader, entry_point: Some("cs_sim"), compilation_options: Default::default(), cache: None }), layout)
        }));
        self.tiles.sim = Some(Sim { tier, states, width, cur: 0, steps: 0, run: (0, 0), frag, compute });
        self.sim_rows_set(rows);
    }
    fn sim_rows(&self) -> u32 { self.sim_rows }
    fn sim_rows_set(&mut self, r: u32) { self.sim_rows = r; }

    /// Zero both state textures (a new morph: every tile on its path).
    fn sim_clear(&mut self) {
        let Some(sim) = &self.tiles.sim else { return };
        let mut enc = self.device.create_command_encoder(&Default::default());
        if sim.tier == ParticleTier::Compute {
            // Storage textures cannot be render targets here: zeros are written instead.
            let bytes = (sim.width * self.sim_rows * 16) as usize;
            let zeros = vec![0u8; bytes];
            for v in &sim.states {
                self.queue.write_texture(wgpu::TexelCopyTextureInfo { texture: v.texture(), mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All }, &zeros,
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(sim.width * 16), rows_per_image: Some(self.sim_rows) },
                    wgpu::Extent3d { width: sim.width, height: self.sim_rows, depth_or_array_layers: 1 });
            }
        } else {
            for v in &sim.states { drop(clear(&mut enc, v, wgpu::Color::TRANSPARENT)); }
        }
        self.queue.submit([enc.finish()]);
    }

    /// The particle uniforms: the frame's, and for the GPU path its grid, forces, pointer and fronts.
    #[allow(clippy::too_many_arguments)]
    fn part_uniforms(&self, look: &Look, scale: f32, clock: f32, m: [f32; 4], grid: Option<(crate::morph::Grid, f32, f32)>, field: &Field, now: f32) -> PartUniforms {
        let th = &look.theme;
        let c = |v: [f32; 3]| [v[0], v[1], v[2], 1.0];
        let pa = &look.particles;
        let mut u = PartUniforms { res: [self.size.0 as f32, self.size.1 as f32, scale, clock], m, bg: c(th.ground), fg: c(th.ink), accent: c(th.accent),
            ga: [0.0; 4], gb: [0.0; 4], gd: [0.0; 4], ge: [SIM_DT, 0.0, -1.0, 0.0], gp: [0.0; 4], gf: [0.0; 4], gw: [0.0; 4], gt: [1.0, 0.0, 0.0, 0.0], fronts: [[0.0; 4]; 8] };
        if let Some((g, stagger, start)) = grid {
            u.ga = g.a; u.gb = g.b;
            u.gd = [g.cols as f32, g.rows as f32, self.tiles.sim.as_ref().map_or(1.0, |s| s.width as f32), stagger];
            let (tc, td) = g.turned.map_or((-1.0, 0.0), |t| (t.first, t.dur));
            u.ge = [SIM_DT, start, tc, td];
            if let Some(t) = g.turned { u.gt = [t.alpha, t.beta, t.to, 0.0]; }
            u.gp = [field.pointer[0], field.pointer[1], pa.pointer * scale * field.pointer[2], pa.pointer_radius * scale];
            u.gf = [pa.curl * scale, pa.curl_scale * scale, pa.damping, pa.spring];
            u.gw = [look.fx.wave_speed * scale, look.fx.wave_width * scale, pa.front * scale, now];
            u.fronts = field.fronts;
        }
        u
    }

    /// Bring the GPU path's state up to `clock` (seconds into the flight) in fixed steps.
    #[allow(clippy::too_many_arguments)]
    fn sim_run(&mut self, look: &Look, scale: f32, m: [f32; 4], grid: crate::morph::Grid, stagger: f32, start: f32, field: &Field, ids: (u32, u32), clock: f32) {
        self.sim_for(grid.count());
        let fresh = self.tiles.sim.as_ref().is_some_and(|s| s.run.0 != ids.0);
        if fresh { self.sim_clear(); }
        let Some(sim) = &mut self.tiles.sim else { return };
        if sim.run != ids { if fresh { sim.cur = 0; } sim.steps = 0; sim.run = ids; }
        let target = (clock / SIM_DT + 1e-3).floor().max(0.0) as u32;
        // A scrubbed run going back: the state is only the swirl's offset from each tile's place on its
        // path (the place itself is computed from the clock), so it carries on from there.
        if target < sim.steps { sim.steps = target; }
        // A host that fell far behind skips ahead rather than stalling.
        if target > sim.steps + 8 { sim.steps = target - 8; }
        while self.tiles.sim.as_ref().is_some_and(|s| s.steps < target) {
            let steps = self.tiles.sim.as_ref().map_or(0, |s| s.steps);
            let t = (steps + 1) as f32 * SIM_DT;
            let u = self.part_uniforms(look, scale, t, m, Some((grid, stagger, start)), field, start + t);
            self.queue.write_buffer(&self.tiles.ubuf, 0, bytemuck::bytes_of(&u));
            let Some(sim) = &self.tiles.sim else { return };
            let (from, to) = (sim.cur, 1 - sim.cur);
            let read = Self::tile_group(&self.device, &self.tiles.layout, &self.tiles.ubuf, &self.tiles.snaps, &sim.states[from]);
            let mut enc = self.device.create_command_encoder(&Default::default());
            if let Some((pipe, layout)) = &sim.compute {
                let t = &self.tiles;
                let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: t.ubuf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&t.snaps[0]) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&t.snaps[1]) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&t.snaps[2]) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&t.snaps[3]) },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&sim.states[from]) },
                    wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&sim.states[to]) },
                ] });
                let mut pass = enc.begin_compute_pass(&Default::default());
                pass.set_pipeline(pipe);
                pass.set_bind_group(0, &group, &[]);
                pass.dispatch_workgroups(sim.width.div_ceil(8), self.sim_rows.div_ceil(8), 1);
            } else if let Some(pipe) = &sim.frag {
                let mut pass = clear(&mut enc, &sim.states[to], wgpu::Color::TRANSPARENT);
                pass.set_pipeline(pipe);
                pass.set_bind_group(0, &read, &[]);
                pass.draw(0..3, 0..1);
            }
            self.queue.submit([enc.finish()]);
            if let Some(sim) = &mut self.tiles.sim { sim.cur = to; sim.steps += 1; }
        }
    }

    pub fn device(&self) -> &wgpu::Device { &self.device }
    /// The last frame's near image read back (RGBA8 rows; `b` is the looseness stage 1 wrote), for checks.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn read_near(&self) -> Vec<u8> {
        let (w, h) = self.size;
        let row = (w * 4).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: self.near.texture(), mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 });
        self.queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let data = buf.slice(..).get_mapped_range().expect("mapped");
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h { px.extend_from_slice(&data[(y * row) as usize..(y * row + w * 4) as usize]); }
        px
    }
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

    /// The upsample after a reduced material pass (made when first needed: it holds the whole material
    /// again, and compiling it is slow on a CPU rasteriser).
    fn ensure_up(&mut self) {
        if self.up_pipe.is_some() { return; }
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
    /// The finishing pass.
    fn ensure_post(&mut self) {
        if self.post.is_some() { return; }
        self.post = Some(self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fs_post"), layout: Some(&self.pl),
            vertex: wgpu::VertexState { module: &self.shader, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
            depth_stencil: None, multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &self.shader, entry_point: Some("fs_post"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: self.out_format, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
            multiview_mask: None, cache: None,
        }));
    }
    /// The morph tiles' pipelines.
    fn ensure_tiles(&mut self) {
        if self.tiles.pipes.is_some() { return; }
        let (p, g) = Self::tile_pipes(&self.device, &self.tiles.pl);
        (self.tiles.pipes, self.tiles.gpipes) = (Some(p), Some(g));
    }
    /// The mesh pass's pipelines.
    fn ensure_mesh(&mut self) {
        if self.mesh.pipes.is_none() { self.mesh.pipes = Some(Self::mesh_pipes(&self.device, &self.mesh.pl)); }
    }

    /// The reduced target for quality `q` (its size), made when the size or quality changes.
    fn lo_for(&mut self, q: f32) -> (u32, u32) {
        let (w, h) = ((self.size.0 as f32 * q).ceil() as u32, (self.size.1 as f32 * q).ceil() as u32);
        let key = (w, h, (q * 1000.0) as u32);
        self.ensure_up();
        if key != self.lo_key {
            self.lo_key = key;
            self.lo = Self::target(&self.device, w, h, self.out_format);
            self.material_up = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&self.ink, &self.near, &self.marks_view, &self.palette_view, &self.chars_view, &self.before, &self.lo, &self.fmap_view, &self.hold]);
        }
        (w, h)
    }

    pub fn draw<S, M: Clone>(&mut self, target: &wgpu::TextureView, ui: &mut Ui<S, M>, look: &Look) {
        // What the frame's input left waiting is laid out first (`Ui::settle`).
        ui.settle();
        let started = crate::perf::now_ms();
        self.reduced = ui.reduced_motion();
        self.fade_s = ui.reduced_fade();
        self.resize(ui.device_size().0, ui.device_size().1);
        // Still warming up (`Renderer::begin`): drawn as plain, which is the same page at rest (§1.23);
        // the frame after the last step condenses into the look.
        let cold = self.warming();
        ui.set_render_ready(self.warm.as_ref().map_or(1.0, |w| w.fraction()));
        let plain;
        let look = if cold { plain = warm::plain(look); &plain } else { look };
        if std::mem::take(&mut self.condense) && warm::plain(look) != *look { ui.condense_all(); }
        if !self.marks_ready && !cold {
            let face = if ui.text_mut().faces() > MONO as usize { MONO } else { 0 };
            let atlas = Atlas::build(ui.text_mut(), face);
            let chars = marks::char_tiles(ui.text_mut(), face);
            self.upload_marks(&atlas, &chars);
        }
        if ui.morph_run().is_some() { self.ensure_tiles(); }
        if look.post.active() { self.ensure_post(); }
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
        // What falls on the beat is drawn at the start of its step; condensing goes on between (`t_cont`).
        let (scale, now, mt) = (ui.scale(), ui.beat_grid(), [ui.material_clock(), ui.ground_clock()]);
        self.t_cont = ui.beat_time();
        // The old frame of a commit and a morph's pictures are drawn without the field map.
        self.fmap_u = [0.0; 4];
        // A new commit: the frame as it is on the screen (mid-transition too, so a second switch goes on
        // from there) into `before`, once; before any frame was kept, the old scene in its old look.
        if let Some(sp) = ui.split() && sp.id != self.split_seen {
            self.split_seen = sp.id;
            self.sbox = sp.scope.unwrap_or([0.0; 4]);
            if self.prev.is_some() {
                let mut enc = self.device.create_command_encoder(&Default::default());
                {
                    let mut pass = clear(&mut enc, &self.before, wgpu::Color::BLACK);
                    pass.set_pipeline(&self.blit);
                    pass.set_bind_group(0, &self.blit_group, &[]);
                    pass.draw(0..3, 0..1);
                }
                self.queue.submit([enc.finish()]);
                self.split = [sp.at[0], sp.at[1], sp.t, 1.0];
            } else if let Some(old) = self.last_look.or(Some(sp.look)) {
                let before = self.before.clone();
                // This draws the old scene into the shared images: the next frame is drawn whole.
                self.prev = None;
                self.region = crate::dirty::Region::All;
                self.frame(&before, Frame { scene: &sp.scene, field: &sp.field, look: &old, lib: ui.meshes(), scale, t: sp.t, mt, split: [0.0; 4], tiles: None, grid: None }, false);
                self.split = [sp.at[0], sp.at[1], sp.t, 1.0];
            }
        }
        // The front has left the screen: the old frame is not needed any more.
        let (w, h) = (self.size.0 as f32, self.size.1 as f32);
        // Within a scope the transition takes the scope's share of the time (`Split::span`).
        let span = if self.sbox[2] > self.sbox[0] { ((self.sbox[2] - self.sbox[0]).hypot(self.sbox[3] - self.sbox[1]) / w.hypot(h).max(1.0)).clamp(0.5, 1.0) } else { 1.0 };
        let dur = look.transition.duration_ms / 1000.0 * span;
        let over = if dur > 0.0 { now - self.split[2] >= dur } else { (now - self.split[2]) * look.fx.wave_speed * scale > (w * w + h * h).sqrt() };
        if self.split[3] > 0.0 && over { self.split = [0.0; 4]; }
        // The transition's program and, for its boxes, the largest elements on the page (device px).
        (self.tr, self.tr_loose) = if self.split[3] > 0.0 && dur > 0.0 {
            ({ let mut tr = look.transition.pack([self.split[0], self.split[1]], self.split[2]); tr[0][0] = dur; tr }, look.transition.loosen(((now - self.split[2]) / dur).clamp(0.0, 1.0)))
        } else { ([[0.0; 4]; 9], 0.0) };
        if self.split[3] > 0.0 && dur > 0.0 {
            let mut found: Vec<[f32; 4]> = ui.nodes().iter().skip(1).filter(|n| n.kind() != "box" && n.rect.w >= 12.0 && n.rect.h >= 12.0)
                .map(|n| { let r = n.rect.intersect(&n.clip); [r.x * scale, r.y * scale, r.right() * scale, r.bottom() * scale] })
                .filter(|r| r[2] > r[0] + 8.0 && r[3] > r[1] + 8.0 && r[2] - r[0] < w * 0.9).collect();
            found.sort_by(|a, b| ((b[2] - b[0]) * (b[3] - b[1])).total_cmp(&((a[2] - a[0]) * (a[3] - a[1]))));
            found.truncate(16);
            // In reading order.
            found.sort_by(|a, b| (a[1], a[0]).partial_cmp(&(b[1], b[0])).unwrap_or(std::cmp::Ordering::Equal));
            self.boxes = [[0.0; 4]; 16];
            for (i, r) in found.into_iter().enumerate() { self.boxes[i] = r; }
        }
        self.last_look = Some(*look);
        let split = self.split;
        // A morph: picture its two elements once, upload its tiles once per run.
        let mut tiles = None;
        let mut grid = None;
        ui.set_particle_gpu(self.tiles.tier != ParticleTier::Cpu);
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
                    self.prepare_meshes(&mut enc, &sc, ui.meshes(), look.marks);
                    self.stage1(&mut enc, &sc, &iv, &nv, 0, None);
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
            // A picture on either side: its Markr slot, as near.g stores it.
            let pic = run.src.iter().chain(&run.dst).find(|i| i.paint[3] == crate::scene::PICTURE).map_or(0.0, |i| (i.paint[1] + 1.0) / 8.0);
            let len = run.flight.map_or(0.0, |f| f[1] - f[0]);
            tiles = run.clock(now).map(|c| [c, run.spec.curl * scale, run.spec.ease.index() as f32, pic, len]);
            // The GPU path: step the simulation up to now (fixed steps), then draw from its state.
            if let (Some(g), Some(c), Some(f)) = (run.grid, run.clock(now), run.flight) {
                // A run turned around flies its way back over `turned.1`; its flight length is the original's.
                let flen = if g.turned.is_some() { run.spec.duration_ms / 1000.0 } else { len };
                let m = [run.spec.curl * scale, run.spec.ease.index() as f32, pic, flen];
                let ids = (run.id, run.upload);
                let stagger = run.spec.stagger;
                let field = *ui.field();
                self.sim_run(look, scale, m, g, stagger, f[0], &field, ids, c);
                grid = Some((g, stagger, f[0]));
                tiles = Some([c, m[0], m[1], pic, flen]);
            }
        }
        // The material's resolution: the look's level, or the automatic one. The reduced pass runs only
        // when something can be loose (at rest every pixel is shaded at full resolution anyway).
        let auto = look.quality <= 0.0;
        let q = if auto { self.auto.quality() } else { crate::perf::LEVELS.iter().copied().min_by(|a, b| (a - look.quality).abs().total_cmp(&(b - look.quality).abs())).unwrap_or(1.0) };
        // A small loose area (a lens, one held control) costs less at full resolution than a second pass.
        let loose = tiles.is_some() || ui.loose_estimate(now, look.fx.lens) >= LO_MIN_AREA;
        let lo = (q < 1.0 && (loose || self.always_lo)).then(|| { let (w, h) = self.lo_for(q); [1.0 / q, q, w as f32, h as f32] });
        let pending = ui.perf().pending;
        // The field map (DESIGN.md §1.29): its bytes when they changed (a few KB), and the numbers.
        let snap = ui.fieldmap().snapshot().clone();
        if self.fmap_seen.as_ref().is_none_or(|b| !std::rc::Rc::ptr_eq(b, &snap.bytes)) && snap.cols > 0 {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &self.fmap, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &snap.bytes,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(snap.cols as u32 * 4), rows_per_image: Some(snap.rows as u32) },
                wgpu::Extent3d { width: snap.cols as u32, height: snap.rows as u32, depth_or_array_layers: 1 },
            );
            self.fmap_seen = Some(snap.bytes.clone());
        }
        self.fmap_u = if snap.nonzero && !self.no_map { [snap.cell * scale, look.map.drift * scale, snap.cols as f32, snap.rows as f32] } else { [0.0; 4] };
        // What changed since the last frame (DESIGN.md §1.19).
        let (mut instances, mut annot) = std::mem::take(&mut self.spare);
        instances.clear();
        instances.extend_from_slice(&ui.scene().instances);
        annot.clear();
        annot.extend_from_slice(&ui.scene().annot);
        let inputs = crate::dirty::Inputs {
            instances, annot, meshes: ui.scene().meshes.iter().map(|m| m.scene.fingerprint() ^ (m.rect[2].to_bits() as u64) << 32 ^ m.rect[3].to_bits() as u64).collect(),
            field: *ui.field(), look: *look, now, mclock: mt[0], gclock: mt[1], size: self.size, map: snap,
            global: split[3] > 0.0 || tiles.is_some() || lo.is_some() || self.hide_glyphs || !ui.scene().groups.is_empty() || self.had_groups || ui.scene().plate_ageing || self.was_ageing || ui.scene().plate_serial != self.plate_serial || ui.scene().plates.len() != self.had_plates,
        };
        self.had_groups = !ui.scene().groups.is_empty();
        self.had_plates = ui.scene().plates.len();
        self.was_ageing = ui.scene().plate_ageing;
        let region = match (&self.prev, self.partial) { (Some(p), true) => crate::dirty::region(p, &inputs, scale), _ => crate::dirty::Region::All };
        if let Some(old) = self.prev.replace(inputs) { self.spare = (old.instances, old.annot); }
        self.region = region;
        self.frame_q(target, Frame { scene: ui.scene(), field: ui.field(), look, lib: ui.meshes(), scale, t: now, mt, split, tiles, grid }, true, lo);
        let submit = (crate::perf::now_ms() - started) as f32;
        let parts = ui.morph_run().map_or(0, |r| if r.clock(now).is_some() { r.grid.map_or(r.particles.len() as u32, |g| g.count()) } else { 0 });
        let path = ui.morph_run().map_or(self.tiles.tier.name(), |r| if r.grid.is_some() { self.tiles.tier.name() } else { "cpu" });
        // The readout's share of the screen redrawn and the passes recorded (stage 1, the material, the
        // annotation layer, plus the reduced pass and the particles when they run).
        let (sw, sh) = (self.size.0.max(1) as f32, self.size.1.max(1) as f32);
        let dirty = match self.region {
            crate::dirty::Region::All => 1.0,
            crate::dirty::Region::Boxes { stage1, stage2 } => {
                let b = crate::dirty::union(stage1, stage2);
                if crate::dirty::is_empty(b) { 0.0 } else { ((b[2].min(sw) - b[0].max(0.0)).max(0.0) * (b[3].min(sh) - b[1].max(0.0)).max(0.0) / (sw * sh)).min(1.0) }
            }
        };
        let passes = 3 + lo.is_some() as u32 + tiles.is_some() as u32 + grid.is_some() as u32;
        // Full-size targets: ink, near, before (8 bytes a pixel or less) and the glyph atlas.
        let tex_mb = (sw * sh * 4.0 * 6.0 + (crate::text::ATLAS * crate::text::ATLAS * 2) as f32) / 1_048_576.0;
        let p = ui.perf_mut();
        p.dirty = dirty; p.passes = passes; p.tex_mb = tex_mb;
        p.quality = q; p.auto = auto; p.particles = parts; p.particle_path = path;
        p.frame(submit);
        if auto { self.auto.feed(pending.update_layout + pending.scene + submit); }
        ui.frame_drawn();
    }

    fn upload(&mut self, scene: &Scene) {
        if scene.instances.len() > self.capacity {
            self.capacity = scene.instances.len().next_power_of_two();
            self.instances = Self::instance_buffer(&self.device, self.capacity);
        }
        if self.warming() {
            // Warming up: every element in the global material, which is plain.
            let v: Vec<_> = scene.instances.iter().map(|i| { let mut i = *i; i.look[0] = 0.0; if self.hide_glyphs && i.paint[3] < 0.5 { i.paint[2] = 0.0; } i }).collect();
            self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&v));
        } else if self.hide_glyphs {
            // For acceptance checks: the same scene with every glyph transparent (nothing else moves).
            let v: Vec<_> = scene.instances.iter().map(|i| { let mut i = *i; if i.paint[3] < 0.5 { i.paint[2] = 0.0; } i }).collect();
            self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&v));
        } else {
            self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&scene.instances));
        }
        // What text to read holds: its instances as they are now, with the hold's reach in paint.xy.
        let mut holds: Vec<Instance> = scene.holds.iter().filter_map(|&(k, r)| scene.instances.get(k as usize).map(|i| { let mut i = *i; i.paint[0] = r[0]; i.paint[1] = r[1]; i })).collect();
        // Where pictures are, so ink drawn over one is written in the picture's terms (its colour), and
        // where the modal layer covers them with its own shapes and pictures (`fs_hold`).
        let layer = scene.layer.unwrap_or(u32::MAX) as usize;
        for (k, i) in scene.instances.iter().enumerate() {
            if i.paint[2] < 0.01 { continue; }
            let pic = i.paint[3] == crate::scene::PICTURE;
            // Character marks over a picture (`el::Cells`) are ink over it, not a picture of their own.
            if k < layer && pic && i.extra[0] <= 0.5 {
                let code = (i.paint[1] + 1.0) / 8.0 - if i.resp[1] > 1.5 { 1.0 / 32.0 } else { 0.0 };
                holds.push(Instance { paint: [-1.0, code, 1.0, i.paint[3]], ..*i });
            } else if k >= layer && ((pic && i.extra[0] <= 0.5) || i.paint[3] == crate::scene::SHAPE) {
                holds.push(Instance { paint: [-2.0, 0.0, 1.0, i.paint[3]], ..*i });
            }
        }
        if holds.len() > self.hold_cap {
            self.hold_cap = holds.len().next_power_of_two();
            self.holds = Self::instance_buffer(&self.device, self.hold_cap);
        }
        if !holds.is_empty() { self.queue.write_buffer(&self.holds, 0, bytemuck::cast_slice(&holds)); }
        self.hold_n = holds.len() as u32;
    }
    /// The hold image for this frame: cleared, then each instance of text to read that sits on others'
    /// ink drawn into it (always whole: it is small, and a partial frame reads it anywhere).
    fn hold_pass(&self, enc: &mut wgpu::CommandEncoder) {
        let mut pass = clear(enc, &self.hold, wgpu::Color::TRANSPARENT);
        if self.hold_n == 0 { return; }
        let Some(group) = &self.hold_group else { return };
        pass.set_pipeline(&self.hold_pipe);
        pass.set_bind_group(0, group, &[]);
        pass.set_vertex_buffer(0, self.holds.slice(..));
        pass.draw(0..4, 0..self.hold_n);
    }

    /// Stage 1: every instance into ink and near. The page first, then `tiles` morph tiles (if any),
    /// then the modal layer.
    /// A stage-1 pass: cleared, or with `scissor` only that box zeroed and drawn (the rest is kept).
    fn open<'a>(&self, enc: &'a mut wgpu::CommandEncoder, view: &'a wgpu::TextureView, scissor: Option<[u32; 4]>) -> wgpu::RenderPass<'a> {
        match scissor {
            None => clear(enc, view, wgpu::Color::TRANSPARENT),
            Some([x, y, w, h]) => {
                let mut pass = load(enc, view);
                pass.set_scissor_rect(x, y, w, h);
                pass.set_pipeline(&self.zero[0]);
                pass.set_bind_group(0, self.pictures.get(&u32::MAX).expect("blank picture"), &[]);
                pass.draw(0..3, 0..1);
                pass
            }
        }
    }
    /// `scissor`: only this box (x, y, w, h device px) is drawn again; the rest of the images is kept.
    #[allow(clippy::too_many_arguments)]
    /// Make the plates this frame uses (`El::plate`) and empty the ones cleared.
    fn plates_ready(&mut self, enc: &mut wgpu::CommandEncoder, scene: &crate::scene::Scene) {
        let (w, h) = self.size;
        for p in &scene.plates {
            let slot = p.slot as usize;
            if self.plates[slot].is_none() {
                let (ink, near) = (Self::target(&self.device, w, h, INK), Self::target(&self.device, w, h, NEAR));
                let (age, near_age) = (Self::target(&self.device, w, h, PLATE_AGE), Self::target(&self.device, w, h, PLATE_AGE));
                let bind = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&ink, &near, &self.marks_view, &self.palette_view, &self.chars_view, &self.blank, &near_age, &self.fmap_view, &age]);
                for v in [&ink, &near, &age, &near_age] { let _ = clear(enc, v, wgpu::Color::TRANSPARENT); }
                self.plates[slot] = Some(PlateTex { ink, near, age, near_age, bind });
                self.plate_inked[slot] = false;
            }
        }
        if scene.plate_serial != self.plate_serial {
            self.plate_serial = scene.plate_serial;
            for &slot in &scene.plate_clear {
                if let Some(pt) = &self.plates[slot as usize] { for v in [&pt.ink, &pt.near, &pt.age, &pt.near_age] { let _ = clear(enc, v, wgpu::Color::TRANSPARENT); } }
                self.plate_inked[slot as usize] = false;
            }
        }
        for p in scene.plates.iter().filter(|p| p.end > p.start) { self.plate_inked[p.slot as usize] = true; }
    }
    fn stage1(&self, enc: &mut wgpu::CommandEncoder, scene: &Scene, ink: &wgpu::TextureView, near: &wgpu::TextureView, tiles: u32, scissor: Option<[u32; 4]>) {
        let layer = scene.layer.unwrap_or(u32::MAX);
        for (k, (view, pipe, top)) in [(ink, &self.pipes[0], &self.pipes[5]), (near, &self.pipes[1], &self.pipes[2])].into_iter().enumerate() {
            let mut pass = self.open(enc, view, scissor);
            let mut pending = tiles > 0;
            let draw_tiles = |pass: &mut wgpu::RenderPass<'_>| {
                if let Some((group, n)) = &self.tiles.grid_now {
                    // The GPU path: tile `i` is instance `i`, its place read from the state texture.
                    pass.set_pipeline(&self.tiles.gpipes.as_ref().expect("made before a morph is drawn")[k]);
                    pass.set_bind_group(0, group, &[]);
                    pass.draw(0..4, 0..*n);
                    return;
                }
                pass.set_pipeline(&self.tiles.pipes.as_ref().expect("made before a morph is drawn")[k]);
                pass.set_bind_group(0, &self.tiles.group, &[]);
                pass.set_vertex_buffer(0, self.tiles.buf.slice(..));
                pass.draw(0..4, 0..tiles);
                pass.set_vertex_buffer(0, self.instances.slice(..));
            };
            pass.set_vertex_buffer(0, self.instances.slice(..));
            // In the ink, a see-through group is drawn into an image of its own and laid on as one
            // (`El::opacity`); the near image takes its instances as they are.
            let groups: &[crate::scene::Group] = if k == 0 && self.group.is_some() { &scene.groups } else { &[] };
            let draw = |pass: &mut wgpu::RenderPass<'_>, from: u32, to: u32| {
                for b in &scene.batches {
                    let (s0, s1) = (b.start.max(from), b.end.min(to));
                    if s0 >= s1 { continue; }
                    pass.set_pipeline(if b.start >= layer { top } else { pipe });
                    let Some(group) = self.batch_group(b.picture) else { continue };
                    pass.set_bind_group(0, group, &[]);
                    pass.draw(0..4, s0..s1);
                }
            };
            let end = scene.instances.len() as u32;
            // The tiles of a morph go in where the layer starts.
            let mut at = 0u32;
            let mut stops: Vec<u32> = groups.iter().flat_map(|g| [g.start, g.end]).chain(scene.plates.iter().filter(|p| self.plates[p.slot as usize].is_some()).flat_map(|p| [p.start, p.end])).collect();
            if pending && layer < end { stops.push(layer); }
            // One past the end: a plate starting at the end (empty) is still laid on.
            stops.push(end + 1);
            stops.sort_unstable();
            let mut kept = scene.plates.iter().filter(|p| self.plates[p.slot as usize].is_some()).peekable();
            for stop in stops {
                if stop <= at { if pending && stop == layer && at == layer { draw_tiles(&mut pass); pending = false; } continue; }
                // Plates in order; several may start at once (empty ones among them).
                while let Some(p) = kept.next_if(|p| p.start == at) {
                    // A plate that has never held ink (nothing drawn since it was made or cleared) is not
                    // laid on: the page costs and looks as one without it.
                    if !self.plate_inked[p.slot as usize] { continue; }
                    if k == 0 { self.plates_laid.set(self.plates_laid.get() + 1); }
                    // A plate (DESIGN.md §1.33): this frame's ink goes into its kept images (and when it
                    // was drawn, into its age), then what they hold is laid on, aged, in its place.
                    let pt = self.plates[p.slot as usize].as_ref().expect("checked");
                    drop(pass);
                    {
                        let mut pp = load(enc, if k == 0 { &pt.ink } else { &pt.near });
                        if let Some([x, y, w, h]) = scissor { pp.set_scissor_rect(x, y, w, h); }
                        pp.set_vertex_buffer(0, self.instances.slice(..));
                        draw(&mut pp, p.start, p.end);
                    }
                    // When each pixel was drawn: where the ink is, and where its nearness reaches.
                    for (target, pipe) in [(&pt.age, &self.plate_pipes[0]), (&pt.near_age, &self.plate_pipes[3])].into_iter().take(if k == 0 { 2 } else { 0 }) {
                        let mut ap = load(enc, target);
                        if let Some([x, y, w, h]) = scissor { ap.set_scissor_rect(x, y, w, h); }
                        ap.set_vertex_buffer(0, self.instances.slice(..));
                        for b in &scene.batches {
                            let (s0, s1) = (b.start.max(p.start), b.end.min(p.end));
                            if s0 >= s1 { continue; }
                            let Some(group) = self.batch_group(b.picture) else { continue };
                            ap.set_pipeline(pipe);
                            ap.set_bind_group(0, group, &[]);
                            ap.draw(0..4, s0..s1);
                        }
                    }
                    pass = load(enc, view);
                    if let Some([x, y, w, h]) = scissor { pass.set_scissor_rect(x, y, w, h); }
                    pass.set_pipeline(&self.plate_pipes[1 + k]);
                    pass.set_bind_group(0, &pt.bind, &[]);
                    let v = p.slot as u32 * 3;
                    pass.draw(v..v + 3, 0..1);
                    pass.set_vertex_buffer(0, self.instances.slice(..));
                    // The whole plate is drawn at once (a group in it goes in plainly); what follows it
                    // up to this stop is drawn below.
                    at = p.end;
                    if pending && at == layer { draw_tiles(&mut pass); pending = false; }
                }
                if stop <= at { continue; }
                match groups.iter().find(|g| g.start == at) {
                    Some(g) => {
                        let (gview, gbind) = self.group.as_ref().expect("made before the frame");
                        drop(pass);
                        {
                            let mut gp = clear(enc, gview, wgpu::Color::TRANSPARENT);
                            if let Some([x, y, w, h]) = scissor { gp.set_scissor_rect(x, y, w, h); }
                            gp.set_vertex_buffer(0, self.instances.slice(..));
                            draw(&mut gp, g.start, g.end.min(stop));
                        }
                        pass = load(enc, view);
                        if let Some([x, y, w, h]) = scissor { pass.set_scissor_rect(x, y, w, h); }
                        // The group's pixels times its opacity (passed in the vertex numbers, in thousandths:
                        // WebGL2 has no first instance).
                        pass.set_pipeline(&self.group_pipe);
                        pass.set_bind_group(0, gbind, &[]);
                        let o = (g.opacity.clamp(0.0, 1.0) * 1000.0).round() as u32;
                        pass.draw(o * 3..o * 3 + 3, 0..1);
                        pass.set_vertex_buffer(0, self.instances.slice(..));
                    }
                    None => draw(&mut pass, at, stop),
                }
                at = stop;
                if pending && at == layer { draw_tiles(&mut pass); pending = false; }
            }
            if pending { draw_tiles(&mut pass); }
        }
    }

    /// Both stages of one frame into `target`, submitted on its own so that each frame gets its own
    /// uniforms and instances. `with_before` binds the old frame (not when drawing into it).
    fn frame(&mut self, target: &wgpu::TextureView, f: Frame<'_>, with_before: bool) { self.frame_q(target, f, with_before, None); }
    /// `lo`: draw the material at reduced resolution first (`qual` for the uniforms), then upsample.
    fn frame_q(&mut self, target: &wgpu::TextureView, f: Frame<'_>, with_before: bool, lo: Option<[f32; 4]>) {
        let scene = f.scene;
        self.plate_now = (scene.plate_now / PLATE_TICK).round();
        self.plate_fade = [[0.0; 4]; 4];
        for p in &scene.plates { self.plate_fade[p.slot as usize] = [(p.fade_s / PLATE_TICK).round(), 1.0, 0.0, 0.0]; }
        self.uniforms(f.field, f.look, f.scale, f.t, f.mt, f.split, lo.unwrap_or([1.0, 1.0, self.size.0 as f32, self.size.1 as f32]));
        self.upload(scene);
        self.tiles.grid_now = None;
        let tiles = match (f.tiles, f.grid) {
            (Some([clock, curl, ease, pic, len]), Some(g)) => {
                let u = self.part_uniforms(f.look, f.scale, clock, [curl, ease, pic, len], Some(g), f.field, f.t);
                self.queue.write_buffer(&self.tiles.ubuf, 0, bytemuck::bytes_of(&u));
                let sim = self.tiles.sim.as_ref().expect("stepped before the frame");
                let group = Self::tile_group(&self.device, &self.tiles.layout, &self.tiles.ubuf, &self.tiles.snaps, &sim.states[sim.cur]);
                self.tiles.grid_now = Some((group, g.0.count()));
                g.0.count()
            }
            (Some([clock, curl, ease, pic, len]), None) if self.tiles.count > 0 => {
                let u = self.part_uniforms(f.look, f.scale, clock, [curl, ease, pic, len], None, f.field, f.t);
                self.queue.write_buffer(&self.tiles.ubuf, 0, bytemuck::bytes_of(&u));
                self.tiles.count
            }
            _ => 0,
        };
        let annots = scene.annot.len() as u32;
        if annots > 0 {
            if scene.annot.len() > self.annot_capacity {
                self.annot_capacity = scene.annot.len().next_power_of_two();
                self.annots = Self::instance_buffer(&self.device, self.annot_capacity);
            }
            self.queue.write_buffer(&self.annots, 0, bytemuck::cast_slice(&scene.annot));
        }
        let mut enc = self.device.create_command_encoder(&Default::default());
        self.prepare_meshes(&mut enc, scene, f.lib, f.look.marks);
        if !scene.groups.is_empty() && self.group.is_none() {
            let view = Self::target(&self.device, self.size.0, self.size.1, INK);
            let bind = Self::bind(&self.device, &self.layout, &self.ubuf, &self.sampler, [&view, &self.blank, &self.marks_view, &self.palette_view, &self.chars_view, &self.blank, &self.blank, &self.fmap_view, &self.blank]);
            self.group = Some((view, bind));
        }
        let (ink, near) = (self.ink.clone(), self.near.clone());
        // The main frame is drawn into the kept frame, only within what changed, then copied out.
        let (w, h) = self.size;
        let boxes = match (with_before, self.region) {
            (true, crate::dirty::Region::Boxes { stage1, stage2 }) => Some((crate::dirty::scissor(stage1, w, h), crate::dirty::scissor(stage2, w, h))),
            _ => None,
        };
        self.last_region = boxes.map(|(a, b)| (a.unwrap_or([0; 4]), b.unwrap_or([0; 4])));
        let frame_view = self.frame_view.clone();
        let out = if with_before { &frame_view } else { target };
        self.plates_ready(&mut enc, scene);
        self.plates_laid.set(0);
        self.hold_pass(&mut enc);
        match boxes {
            None => self.stage1(&mut enc, scene, &ink, &near, tiles, None),
            Some((Some(b), _)) => self.stage1(&mut enc, scene, &ink, &near, tiles, Some(b)),
            Some((None, _)) => {}
        }
        if lo.is_some() {
            let lo_view = self.lo.clone();
            let mut pass = clear(&mut enc, &lo_view, wgpu::Color::BLACK);
            pass.set_pipeline(&self.pipes[3]);
            pass.set_bind_group(0, if with_before { &self.material } else { &self.material_first }, &[]);
            pass.draw(0..3, 0..1);
        }
        let stage2 = match boxes { None => Some(None), Some((_, Some(b))) => Some(Some(b)), Some((_, None)) => None };
        if let Some(sc) = stage2 {
            let mut pass = match sc { None => clear(&mut enc, out, wgpu::Color::BLACK), Some(_) => load(&mut enc, out) };
            if let Some([x, y, sw, sh]) = sc { pass.set_scissor_rect(x, y, sw, sh); }
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
                pass.set_pipeline(&self.pipes[4]);
                pass.set_bind_group(0, self.pictures.get(&u32::MAX).expect("blank picture"), &[]);
                pass.set_vertex_buffer(0, self.annots.slice(..));
                pass.draw(0..4, 0..annots);
            }
        }
        if with_before {
            let mut pass = clear(&mut enc, target, wgpu::Color::BLACK);
            pass.set_pipeline(match (&self.post, f.look.post.active()) { (Some(p), true) => p, _ => &self.blit });
            pass.set_bind_group(0, &self.blit_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([enc.finish()]);
    }

    #[allow(clippy::too_many_arguments)]
    fn uniforms(&self, field: &Field, look: &Look, scale: f32, t: f32, mt: [f32; 2], split: [f32; 4], qual: [f32; 4]) {
        let (fx, th) = (&look.fx, &look.theme);
        let c = |v: [f32; 3]| [v[0], v[1], v[2], 1.0];
        let (bg, fg, accent) = (c(th.ground), c(th.ink), c(th.accent));
        let flag = |b: bool| if b { 1.0 } else { 0.0 };
        let mut pics = [[0.0; 4]; 56];
        for (i, p) in look.pics.iter().enumerate() { pics[i * 7..i * 7 + 7].copy_from_slice(&p.pack()); }
        let mut mats = [[0.0; 4]; 2 * crate::materials::MAX];
        let added = (MATERIAL_COUNT as u8..crate::materials::MAX as u8).map_while(crate::materials::added).map(|a| a.params);
        for (i, m) in look.params.iter().copied().chain(added).enumerate() {
            mats[i * 2] = [m.0[0], m.0[1], m.0[2], m.0[3]];
            mats[i * 2 + 1] = [m.0[4], m.0[5], m.0[6], m.0[7]];
        }
        let post = look.post.pack(scale);
        let u = Uniforms {
            res: [self.size.0 as f32, self.size.1 as f32, scale, t],
            fa: [fx.pitch, mt[0], fx.jitter, fx.glitch],
            fb: [fx.wave_speed, fx.wave_life, fx.wave_width, fx.wave_tail],
            fc: [fx.push, fx.lens, fx.enter, flag(fx.square)],
            fd: [look.material.id() as f32, flag(self.out_format.is_srgb()), SPREAD as f32, marks::PALETTES.len() as f32],
            bg, fg, accent, pointer: [field.pointer[0], field.pointer[1], field.pointer[2], fx.lens_hard], wipe: [field.wipe[0], field.wipe[1], flag(look.off.ground), field.wipe[3]], wipe_rect: field.wipe_rect, split, fronts: field.fronts, pics,
            ground2: [th.ground2[0], th.ground2[1], th.ground2[2], look.loose_scale], second: c(th.second), third: c(th.third),
            burst: field.burst, burst_t: [field.burst_t[0], field.burst_t[1], field.burst_t[2], look.legible_cap], mats, calm: field.calm, qual, scopes: field.scopes,
            rip: [look.loose_max, if split[3] > 0.0 { self.tr_loose } else { 0.0 }, if split[3] > 0.0 { look.transition.steps.iter().rposition(|st| st.kind != crate::preset::StepKind::None).map_or(0.0, |i| i as f32 + 1.0) } else { 0.0 }, mt[1]],
            tr: if split[3] > 0.0 { self.tr } else { [[0.0; 4]; 9] }, boxes: self.boxes,
            scrolls: field.scrolls, scrollo: field.scrollo,
            scrp: [look.scroll.trail * scale, look.scroll.misreg * scale, look.scroll.slip * scale, look.scroll.band.max(4.0) * scale],
            scrq: [look.scroll.slip_ms.max(16.0) / 1000.0, if look.beat.on() { self.t_cont } else { t }, th.press_keep(look.legible_cap), if self.reduced { self.fade_s } else { 0.0 }],
            post: post.0, pal: post.1, fmap: self.fmap_u, sbox: if split[3] > 0.0 { self.sbox } else { [0.0; 4] },
            plate: [self.plate_now, 0.0, 0.0, 0.0], plates: self.plate_fade,
        };
        self.queue.write_buffer(&self.ubuf, 0, bytemuck::bytes_of(&u));
    }
}

/// A pass that keeps what the target holds.
fn load<'a>(enc: &'a mut wgpu::CommandEncoder, view: &'a wgpu::TextureView) -> wgpu::RenderPass<'a> {
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None, depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
    })
}
fn clear<'a>(enc: &'a mut wgpu::CommandEncoder, view: &'a wgpu::TextureView, color: wgpu::Color) -> wgpu::RenderPass<'a> {
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None, depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(color), store: wgpu::StoreOp::Store } })],
    })
}

/// One mesh's pieces: the buffer, its first vertex and how many.
pub type MeshPieces = Vec<(u32, u32, u32)>;

/// Meshes' vertices (triangle lists) in buffers of at most `cap` vertices: a mesh goes whole into the
/// current buffer when it fits, and one larger than a buffer is cut at whole triangles. Returns the
/// buffers and each mesh's pieces (buffer, first vertex, count).
pub fn mesh_chunks<I: IntoIterator<Item = Vec<[f32; 6]>>>(meshes: I, cap: usize) -> (Vec<Vec<[f32; 6]>>, Vec<MeshPieces>) {
    let cap = (cap / 3 * 3).max(3);
    let mut chunks: Vec<Vec<[f32; 6]>> = vec![];
    let mut ranges = vec![];
    for v in meshes {
        let mut parts = vec![];
        let mut rest = &v[..];
        while !rest.is_empty() {
            if chunks.last().is_none_or(|c| c.len() == cap || (c.len() + rest.len() > cap && !c.is_empty() && rest.len() <= cap)) { chunks.push(Vec::with_capacity(cap.min(rest.len()))); }
            let at = (chunks.len() - 1) as u32;
            let c = chunks.last_mut().expect("pushed");
            let n = rest.len().min(cap - c.len()) / 3 * 3;
            let n = if n == 0 { rest.len().min(cap - c.len()) } else { n };
            parts.push((at, c.len() as u32, n as u32));
            c.extend_from_slice(&rest[..n]);
            rest = &rest[n..];
        }
        ranges.push(parts);
    }
    (chunks, ranges)
}
