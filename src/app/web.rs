//! A canvas on a web page. [`Host`] owns the runtime, the renderer and the canvas's surface and answers
//! the page's events; [`web_app!`](crate::web_app) generates the module's exports from it, so an
//! application's crate only names its state, its message, and how to build its `Ui`.
//!
//! The page (the web shell) calls `start(canvas, sans, pixel, mono, scale, hash)` once, then forwards
//! pointer, wheel, key, text, composition, clipboard and resize events and calls `frame(t)` from its
//! animation frame loop while `frame` asks for one. Positions are CSS (logical) px, sizes device px,
//! times seconds on the page's clock (`performance.now() / 1000`).
use super::{App, Appearance, Dropped};
use crate::el::{Key, Mods};
use crate::render::Renderer;
use crate::text::Face;
use crate::ui::{PointerKind, Ui};

#[doc(hidden)]
pub mod __rt {
    pub use console_error_panic_hook;
    pub use wasm_bindgen;
    pub use wasm_bindgen_futures;
    pub use web_sys;
}

/// The running application on a canvas.
pub struct Host<S, M> {
    /// The runtime.
    pub ui: Ui<S, M>,
    /// The renderer (for the application's own pictures and meshes).
    pub r: Renderer,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    started: bool,
    /// Files of a drop the page is still reading.
    dropping: Vec<Dropped>,
}

/// Log a line on the page's console.
pub fn log(s: &str) { web_sys::console::warn_1(&s.into()); }

/// The panic hook a web module runs with: the panic goes to the console as `console_error_panic_hook`
/// writes it, and the module's [`Slot`](super::slot::Slot) learns of it, so the next call from the page
/// goes on instead of finding the trapped call's application still taken.
pub fn panic_hook() {
    static SET: std::sync::Once = std::sync::Once::new();
    SET.call_once(|| std::panic::set_hook(Box::new(|info| { super::slot::note_panic(); console_error_panic_hook::hook(info); })));
}

/// A pointer kind from the page: 0 mouse, 1 touch, 2 pen (DESIGN.md §1.8.2).
pub fn pointer_kind(k: u32) -> PointerKind { match k { 1 => PointerKind::Touch, 2 => PointerKind::Pen, _ => PointerKind::Mouse } }

/// Why a module could not start on its canvas (`Host::start`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartError {
    /// No graphics adapter: the browser has no WebGPU (or, with the `webgl` feature, no WebGL2 either),
    /// or it would not give one for this canvas.
    Adapter(String),
    /// An adapter, but no device, surface or format the renderer can draw with.
    Device(String),
}
impl StartError {
    /// What the page is told (the module's `start_failure` export): `adapter` or `device`.
    pub fn kind(&self) -> &'static str { match self { Self::Adapter(_) => "adapter", Self::Device(_) => "device" } }
}
impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { match self { Self::Adapter(e) => write!(f, "no graphics adapter: {e}"), Self::Device(e) => write!(f, "{e}") } }
}
impl std::error::Error for StartError {}

fn backend_name(b: wgpu::Backend) -> &'static str { match b { wgpu::Backend::BrowserWebGpu => "WebGPU", wgpu::Backend::Gl => "WebGL2", _ => "other" } }

/// An adapter and a device, from `instance`, for `surface` where one is given (WebGL2's adapter needs it).
async fn open(instance: &wgpu::Instance, surface: Option<&wgpu::Surface<'static>>) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), StartError> {
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions { compatible_surface: surface, ..Default::default() }).await.map_err(|e| StartError::Adapter(e.to_string()))?;
    // WebGPU gets compute (the particle simulation's tier); WebGL2 has none.
    let base = if adapter.get_info().backend == wgpu::Backend::BrowserWebGpu { wgpu::Limits::downlevel_defaults() } else { wgpu::Limits::downlevel_webgl2_defaults() };
    let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor { required_limits: base.using_resolution(adapter.limits()), ..Default::default() })
        .await.map_err(|e| StartError::Device(e.to_string()))?;
    Ok((adapter, device, queue))
}

/// The canvas's surface, adapter and device: WebGPU first, its surface made only once its device is
/// there (a canvas that has given a WebGPU context can give no other); then, with the `webgl` feature,
/// WebGL2 on the untouched canvas, when the browser has no WebGPU adapter or its device fails. A device
/// failure is reported as `Device` only when WebGL2 fails too.
async fn gpu(canvas: web_sys::HtmlCanvasElement) -> Result<(wgpu::Surface<'static>, wgpu::Adapter, wgpu::Device, wgpu::Queue), StartError> {
    let desc = |backends| wgpu::InstanceDescriptor { backends, ..wgpu::InstanceDescriptor::new_without_display_handle() };
    let mut failed = StartError::Adapter("the browser has no WebGPU".into());
    if wgpu::util::is_browser_webgpu_supported().await {
        let instance = wgpu::Instance::new(desc(wgpu::Backends::BROWSER_WEBGPU));
        match open(&instance, None).await {
            Ok((adapter, device, queue)) => {
                let surface = instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas)).map_err(|e| StartError::Device(e.to_string()))?;
                return Ok((surface, adapter, device, queue));
            }
            Err(e) => { log(&format!("amenui: WebGPU: {e}")); failed = e; }
        }
    }
    #[cfg(feature = "webgl")]
    {
        let instance = wgpu::Instance::new(desc(wgpu::Backends::GL));
        let gl = match instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas)) {
            Ok(surface) => open(&instance, Some(&surface)).await.map(|(a, d, q)| (surface, a, d, q)),
            Err(e) => Err(StartError::Adapter(e.to_string())),
        };
        match gl {
            Ok(g) => return Ok(g),
            // A WebGPU device that failed is still the reason, whatever WebGL2 says.
            Err(e) => { log(&format!("amenui: WebGL2: {e}")); if !matches!(failed, StartError::Device(_)) { failed = e; } }
        }
    }
    #[cfg(not(feature = "webgl"))]
    drop(canvas);
    Err(failed)
}

impl<S: App, M: Clone> Host<S, M> {
    /// Start on a canvas: WebGPU where an adapter and a device really exist, WebGL2 otherwise (WebGPU
    /// only without the `webgl` feature: a browser without it fails with `StartError::Adapter`). `fonts` are the
    /// bytes of the three main bundled faces, sans, pixel and mono (the page fetches them); Geist Pixel and the
    /// serif are compiled into the module. `hash` is the page's `location.hash` ([`App::route`]); an
    /// empty one leaves the application where `make` put it.
    pub async fn start(
        canvas: web_sys::HtmlCanvasElement, fonts: [Vec<u8>; 3], scale: f32, hash: &str,
        setup: impl FnOnce(&mut Renderer), make: impl FnOnce(Vec<Face>, u32, u32, f32) -> Ui<S, M>,
    ) -> Result<Self, StartError> {
        let (w, h) = (canvas.width().max(1), canvas.height().max(1));
        let (surface, adapter, device, queue) = gpu(canvas).await?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).or_else(|| caps.formats.first().copied()).ok_or_else(|| StartError::Device("the canvas offers no format".into()))?;
        let config = surface.get_default_config(&adapter, w, h).map(|mut c| { c.format = format; c }).ok_or_else(|| StartError::Device("the canvas is not supported by the adapter".into()))?;
        surface.configure(&device, &config);
        // Made a piece at a time (DESIGN.md §1.9): the first frame is plain and comes at once; each frame
        // after it makes the next pieces within `WARM_MS` (`Host::frame`).
        // A GPU error (a buffer the adapter refuses, a lost surface) is logged, not a panic: the page
        // keeps running, and what failed is drawn again or left out.
        device.on_uncaptured_error(std::sync::Arc::new(|e: wgpu::Error| log(&format!("amenui: GPU error: {e}"))));
        let mut r = Renderer::begin(device, queue, format, w, h).into_renderer();
        r.set_particle_tier(crate::render::particle_tier(&adapter));
        setup(&mut r);
        let [sans, pixel, mono] = fonts;
        // Geist Pixel and the serif come later, when the page uses them (`faces_wanted`, `add_face`).
        let faces = crate::text::with_declared(crate::text::bundled(sans, pixel, mono));
        let mut ui = make(faces, w, h, scale);
        // The page hands over every pointer move and wheel; they are laid out once a frame.
        ui.coalesce(true);
        if !hash.trim_start_matches('#').is_empty() && ui.state.route(hash) { ui.relayout(); }
        ui.set_warm(Some(r.warm_progress()));
        ui.perf_mut().backend = backend_name(adapter.get_info().backend).into();
        ui.perf_mut().adapter = adapter.get_info().name;
        Ok(Self { ui, r, surface, config, started: false, dropping: vec![] })
    }

    /// The canvas changed size: `w`, `h` device px, `scale` device px per CSS px.
    pub fn resize(&mut self, w: u32, h: u32, scale: f32) {
        self.config.width = w.max(1);
        self.config.height = h.max(1);
        self.surface.configure(self.r.device(), &self.config);
        self.ui.resize(self.config.width, self.config.height, scale);
    }

    // Every input first brings the clock to its time: frames are drawn on demand, so the last tick may
    // be long ago, and what the input starts (a burst, a front) starts now. `te` is when the event
    // happened (its timestamp), `t` when it is handled.
    /// The pointer moved.
    pub fn pointer_move(&mut self, id: u32, kind: u32, (x, y): (f32, f32), t: f32, te: f32) {
        self.ui.tick(t); self.ui.event_time(te.min(t)); self.ui.pointer_moved(id as u64, pointer_kind(kind), x, y);
    }
    /// The pointer left the canvas.
    pub fn pointer_leave(&mut self, t: f32) { self.ui.tick(t); self.ui.pointer_leave(); }
    /// A button or finger went down.
    pub fn pointer_down(&mut self, id: u32, kind: u32, (x, y): (f32, f32), shift: bool, t: f32, te: f32) {
        self.ui.tick(t); self.ui.event_time(te.min(t)); self.ui.pointer_down(id as u64, pointer_kind(kind), x, y, shift);
    }
    /// It came up.
    pub fn pointer_up(&mut self, id: u32, kind: u32, x: f32, y: f32, t: f32) { self.ui.tick(t); self.ui.pointer_up(id as u64, pointer_kind(kind), x, y); }
    /// The browser took the pointer (a scroll gesture, a system dialog).
    pub fn pointer_cancel(&mut self, id: u32, t: f32) { self.ui.tick(t); self.ui.pointer_cancel(id as u64); }
    /// The wheel or a trackpad scrolled by `dx`, `dy` CSS px.
    pub fn wheel(&mut self, x: f32, y: f32, dx: f32, dy: f32, t: f32) { self.ui.tick(t); self.ui.wheel_xy(x, y, dx, dy); }
    /// A DOM key name with its modifiers (`cmd` is Ctrl or ⌘). Returns whether the page should not act on it.
    pub fn key(&mut self, name: &str, shift: bool, cmd: bool, alt: bool, t: f32) -> bool {
        let Some(k) = Key::from_name(name) else { return false };
        self.ui.tick(t);
        self.ui.key_mods(k, Mods { shift, cmd, alt })
    }
    /// A key came up.
    pub fn key_up(&mut self, name: &str, t: f32) {
        let Some(k) = Key::from_name(name) else { return };
        self.ui.tick(t);
        self.ui.key_up(k);
    }
    /// Text typed into the text agent. Returns whether a field took it.
    pub fn text_input(&mut self, text: &str, t: f32) -> bool { self.ui.tick(t); self.ui.text_input(text) }
    /// The input method's composition so far.
    pub fn ime_preedit(&mut self, text: &str, t: f32) -> bool { self.ui.tick(t); self.ui.ime_preedit(text, None) }
    /// The input method committed its text.
    pub fn ime_commit(&mut self, text: &str, t: f32) -> bool { self.ui.tick(t); self.ui.ime_commit(text) }
    /// Text pasted from the clipboard. Returns whether a field took it.
    pub fn paste(&mut self, text: &str, t: f32) -> bool { self.ui.tick(t); self.ui.paste(text) }
    /// Cut the selection; returns its text for the clipboard.
    pub fn cut(&mut self, t: f32) -> Option<String> { self.ui.tick(t); self.ui.cut() }
    /// The page's hash changed (a link, Back). Returns whether it names something.
    pub fn route(&mut self, hash: &str) -> bool { let ok = self.ui.state.route(hash); if ok { self.ui.relayout(); } ok }
    /// `theme` and `material` by name ([`App::set_look`]).
    pub fn set_look(&mut self, theme: &str, material: &str) { if self.ui.state.set_look(theme, material) { self.ui.relayout(); } }
    /// The language by BCP 47 tag ([`App::set_locale`]).
    pub fn set_locale(&mut self, tag: &str) {
        if let Some(l) = crate::i18n::Locale::from_tag(tag) && self.ui.state.set_locale(l) { self.ui.relayout(); }
    }
    /// The faces the page uses that have not come, by name (`Ui::faces_wanted`), as JSON.
    pub fn faces_wanted(&self) -> String { serde_json::to_string(&self.ui.faces_wanted()).unwrap_or_else(|_| "[]".into()) }
    /// A face's bytes came (`Ui::add_face`); returns its number, or -1 when they are not a font.
    pub fn add_face(&mut self, name: &str, bytes: Vec<u8>, t: f32) -> i32 { self.ui.tick(t); self.ui.add_face(name, bytes).map_or(-1, i32::from) }
    /// The resources asked for that have not come, by name (`Ui::assets_wanted`), as JSON.
    pub fn assets_wanted(&self) -> String { serde_json::to_string(&self.ui.assets_wanted()).unwrap_or_else(|_| "[]".into()) }
    /// A resource came (`Ui::add_asset`).
    pub fn add_asset(&mut self, name: &str, bytes: Vec<u8>, t: f32) { self.ui.tick(t); self.ui.add_asset(name, bytes); }
    /// `prefers-reduced-motion: reduce`, at start and when it changes.
    pub fn set_reduced_motion(&mut self, on: bool) { self.ui.state.set_reduced_motion(on); self.ui.set_reduced_motion(on); }
    /// `prefers-color-scheme: dark` and `prefers-contrast: more`, at start and when they change.
    pub fn set_appearance(&mut self, dark: bool, high_contrast: bool) {
        if self.ui.state.set_appearance(Appearance { dark, high_contrast }) { self.ui.relayout(); }
    }
    /// Files dragged over the canvas, or gone without a drop.
    pub fn files_hovering(&mut self, on: bool) { if self.ui.state.files_hovering(on) { self.ui.relayout(); } }
    /// One file of a drop (the page reads them one by one), then [`Host::drop_done`].
    pub fn drop_file(&mut self, name: &str, bytes: Vec<u8>) { self.dropping.push(Dropped::bytes(name, bytes)); }
    /// The drop's files have all been read: they go to the application together.
    pub fn drop_done(&mut self, t: f32) {
        self.ui.tick(t);
        let files = std::mem::take(&mut self.dropping);
        let changed = self.ui.state.files_hovering(false) | (!files.is_empty() && self.ui.state.files_dropped(files));
        if changed { self.ui.relayout(); }
    }
    /// Assistive technology focused a mirrored element.
    pub fn a11y_focus(&mut self, key: &str, t: f32) { self.ui.tick(t); self.ui.focus_key(key); }
    /// Assistive technology activated a mirrored element.
    pub fn a11y_activate(&mut self, key: &str, t: f32) -> bool { self.ui.tick(t); self.ui.activate(key) }
    /// What changed in the accessibility mirror since the last call ([`super::a11y_json`]).
    pub fn a11y_update(&mut self) -> String { super::a11y_json(&self.ui.a11y_diff()) }
    /// The frames of the last few seconds: how many, their mean and largest cost (ms), and the mean time
    /// between them (ms), for measuring a page headless.
    pub fn frame_costs(&self) -> Vec<f32> {
        let f = &self.ui.perf().frames;
        let v: Vec<f32> = f.iter().map(|(_, c)| c.total()).collect();
        let gap = if f.len() > 1 { ((f[f.len() - 1].0 - f[0].0) / (f.len() - 1) as f64) as f32 } else { 0.0 };
        vec![v.len() as f32, v.iter().sum::<f32>() / v.len().max(1) as f32, v.iter().copied().fold(0.0, f32::max), gap]
    }

    /// Draw a frame if one is needed. Returns when the next one is: 0 as soon as possible (keep the
    /// animation frame loop going), a delay in seconds (a timed wake), or -1 (only input can change
    /// anything: stop until then).
    pub fn frame(&mut self, t: f32) -> f32 {
        if !self.started { self.ui.restart(t); self.started = true; }
        self.ui.tick(t);
        if self.ui.needs_frame(t) { self.draw(); self.r.warm_frame(&mut self.ui, crate::render::WARM_MS); } else { self.ui.perf_mut().skipped += 1; }
        self.ui.next_wake(t).map_or(-1.0, |w| (w - t).max(0.0))
    }

    /// The warm-up in idle time (`Renderer::warm_idle`), for up to `budget_ms`: 1 while pieces are
    /// left, 0 once it is done, plus 2 when the page wants a frame now (the look's material can be drawn).
    /// It draws nothing: the page shell calls it from `requestIdleCallback` until it says 0 or 2.
    pub fn warm_idle(&mut self, budget_ms: f32, t: f32) -> u32 {
        let more = self.r.warm_idle(&mut self.ui, budget_ms);
        u32::from(more) + if self.ui.needs_frame(t) { 2 } else { 0 }
    }

    /// How far the renderer's warm-up is, `[done, total]` (`Ui::warm`); empty once it is done.
    pub fn warm(&self) -> Vec<u32> { self.ui.warm().map(|p| vec![p.done, p.total]).unwrap_or_default() }

    fn draw(&mut self) {
        let tex = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => { self.surface.configure(self.r.device(), &self.config); self.ui.invalidate(); return; }
        };
        let view = tex.texture.create_view(&Default::default());
        let look = self.ui.look();
        self.r.draw(&view, &mut self.ui, &look);
        drop(view);
        self.r.queue().present(tex);
    }
}

/// The web module's exports for an application: `web_app!(State, Msg, setup, make)`, where `setup` is a
/// `fn(&mut Renderer)` (pictures, meshes) and `make` a `fn(Vec<Face>, u32, u32, f32) -> Ui<State, Msg>`.
/// Put it in the crate built as the `cdylib`; the web shell calls the generated functions. It also
/// defines `host(|h| …)` for the crate's own extra exports.
///
/// ```ignore
/// amenui::web_app!(Counter, Msg, |_| {}, |faces, w, h, scale| Ui::new(Counter { n: 0 }, view, update, faces, w, h, scale));
/// ```
#[macro_export]
macro_rules! web_app {
    ($state:ty, $msg:ty, $setup:expr, $make:expr) => {
        mod __amenui_web {
            use super::*;
            use $crate::app::web::__rt::wasm_bindgen::prelude::*;
            type H = $crate::app::web::Host<$state, $msg>;
            ::std::thread_local! { static HOST: $crate::app::slot::Slot<H> = const { $crate::app::slot::Slot::new() }; }
            ::std::thread_local! { static FAILED: ::std::cell::Cell<&'static str> = const { ::std::cell::Cell::new("") }; }
            /// Run `f` on the running application, if it has started. After a panic in an earlier call the
            /// application goes on from where the panic left it (`amenui::app::slot`).
            pub fn host<R>(f: impl FnOnce(&mut H) -> R) -> ::std::option::Option<R> {
                let (r, how) = HOST.with(|s| s.call(f));
                if how == $crate::app::slot::Call::RanAfterPanic { $crate::app::web::log("amenui: going on after a panic in an earlier call"); }
                r
            }

            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen, wasm_bindgen_futures = $crate::app::web::__rt::wasm_bindgen_futures)]
            pub async fn start(canvas: $crate::app::web::__rt::web_sys::HtmlCanvasElement, sans: Vec<u8>, pixel: Vec<u8>, mono: Vec<u8>, scale: f32, hash: String) -> Result<(), JsValue> {
                $crate::app::web::panic_hook();
                let h = H::start(canvas, [sans, pixel, mono], scale, &hash, $setup, $make).await.map_err(|e| { FAILED.with(|f| f.set(e.kind())); JsValue::from_str(&e.to_string()) })?;
                HOST.with(|s| s.set(h));
                Ok(())
            }
            /// Why `start` failed (`StartError::kind`: `adapter`, `device`), or empty.
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn start_failure() -> String { FAILED.with(|f| f.get()).to_owned() }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn resize(w: u32, h: u32, scale: f32) { host(|s| s.resize(w, h, scale)); }
            /// The warm-up in idle time: 1 while pieces are left, plus 2 when a frame is wanted now. A call
            /// that finds the module busy says 1, so the page asks again.
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn warm_idle(budget_ms: f32, t: f32) -> u32 { host(|s| s.warm_idle(budget_ms, t)).unwrap_or(1) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn pointer_move(id: u32, k: u32, x: f32, y: f32, t: f32, te: f32) { host(|s| s.pointer_move(id, k, (x, y), t, te)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn pointer_leave(t: f32) { host(|s| s.pointer_leave(t)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn pointer_down(id: u32, k: u32, x: f32, y: f32, shift: bool, t: f32, te: f32) { host(|s| s.pointer_down(id, k, (x, y), shift, t, te)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn pointer_up(id: u32, k: u32, x: f32, y: f32, t: f32) { host(|s| s.pointer_up(id, k, x, y, t)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn pointer_cancel(id: u32, t: f32) { host(|s| s.pointer_cancel(id, t)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn wheel(x: f32, y: f32, dx: f32, dy: f32, t: f32) { host(|s| s.wheel(x, y, dx, dy, t)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn key(name: &str, shift: bool, t: f32) -> bool { host(|s| s.key(name, shift, false, false, t)).unwrap_or(false) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn key_mods(name: &str, shift: bool, cmd: bool, alt: bool, t: f32) -> bool { host(|s| s.key(name, shift, cmd, alt, t)).unwrap_or(false) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn key_up(name: &str, t: f32) { host(|s| s.key_up(name, t)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn text_focused() -> bool { host(|s| s.ui.text_focused()).unwrap_or(false) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn text_input(text: &str, t: f32) -> bool { host(|s| s.text_input(text, t)).unwrap_or(false) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn ime_preedit(text: &str, t: f32) -> bool { host(|s| s.ime_preedit(text, t)).unwrap_or(false) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn ime_commit(text: &str, t: f32) -> bool { host(|s| s.ime_commit(text, t)).unwrap_or(false) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn ime_rect() -> Vec<f32> { host(|s| s.ui.ime_rect().map(|r| vec![r.x, r.y, r.w, r.h]).unwrap_or_default()).unwrap_or_default() }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn paste(text: &str, t: f32) -> bool { host(|s| s.paste(text, t)).unwrap_or(false) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn copy() -> Option<String> { host(|s| s.ui.copy()).flatten() }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn cut(t: f32) -> Option<String> { host(|s| s.cut(t)).flatten() }
            /// Text a copy button asked to put on the clipboard (once).
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn take_clipboard() -> Option<String> { host(|s| $crate::app::App::take_clipboard(&mut s.ui.state)).flatten() }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn route(hash: &str) -> bool { host(|s| s.route(hash)).unwrap_or(false) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn hash() -> String { host(|s| $crate::app::App::hash(&s.ui.state)).unwrap_or_default() }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn set_look(theme: &str, material: &str) { host(|s| s.set_look(theme, material)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn set_locale(tag: &str) { host(|s| s.set_locale(tag)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn set_reduced_motion(on: bool) { host(|s| s.set_reduced_motion(on)); }
            /// The faces the page uses that have not come, as a JSON list of names: fetch each, then `add_face`.
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn faces_wanted() -> String { host(|s| s.faces_wanted()).unwrap_or_else(|| "[]".into()) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn add_face(name: &str, bytes: Vec<u8>, t: f32) -> i32 { host(|s| s.add_face(name, bytes, t)).unwrap_or(-1) }
            /// The resources asked for that have not come, as a JSON list of names.
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn assets_wanted() -> String { host(|s| s.assets_wanted()).unwrap_or_else(|| "[]".into()) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn add_asset(name: &str, bytes: Vec<u8>, t: f32) { host(|s| s.add_asset(name, bytes, t)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn set_appearance(dark: bool, high_contrast: bool) { host(|s| s.set_appearance(dark, high_contrast)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn files_hovering(on: bool) { host(|s| s.files_hovering(on)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn drop_file(name: &str, bytes: Vec<u8>) { host(|s| s.drop_file(name, bytes)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn drop_done(t: f32) { host(|s| s.drop_done(t)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn a11y_update() -> String { host(|s| s.a11y_update()).unwrap_or_else(|| "[]".into()) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn a11y_focus(key: &str, t: f32) { host(|s| s.a11y_focus(key, t)); }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn a11y_activate(key: &str, t: f32) -> bool { host(|s| s.a11y_activate(key, t)).unwrap_or(false) }
            /// The CSS cursor for where the pointer is.
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn cursor() -> String { host(|s| s.ui.cursor().css().to_owned()).unwrap_or_else(|| "default".into()) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn frame(t: f32) -> f32 { host(|s| s.frame(t)).unwrap_or(-1.0) }
            /// How far the warm-up is, `[done, total]`; empty once it is done.
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn warm() -> Vec<u32> { host(|s| s.warm()).unwrap_or_default() }
            // For automation and tests.
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn flinging() -> bool { host(|s| s.ui.flinging()).unwrap_or(false) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn scroll_offset(key: &str) -> f32 { host(|s| s.ui.scroll_offset(key)).unwrap_or(0.0) }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn node_rect(key: &str) -> Vec<f32> { host(|s| s.ui.node(key).map(|n| vec![n.rect.x, n.rect.y, n.rect.w, n.rect.h]).unwrap_or_default()).unwrap_or_default() }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn frames() -> Vec<u32> { host(|s| vec![s.ui.perf().drawn as u32, s.ui.perf().skipped as u32]).unwrap_or_default() }
            #[wasm_bindgen(wasm_bindgen = $crate::app::web::__rt::wasm_bindgen)]
            pub fn frame_costs() -> Vec<f32> { host(|s| s.frame_costs()).unwrap_or_default() }
        }
        pub use __amenui_web::host;
    };
}
