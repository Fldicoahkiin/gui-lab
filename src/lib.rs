pub mod demo;
pub mod el;
pub mod gallery;
pub mod gaze;
pub mod marks;
pub mod morph;
#[cfg(not(target_arch = "wasm32"))]
pub mod offscreen;
pub mod render;
pub mod scene;
pub mod style;
pub mod text;
pub mod ui;
pub mod widgets;
pub mod wrap;

/// Web entry: the page is a canvas that forwards pointer, wheel, key and resize events here, and the
/// page's `location.hash` (the gallery page, so it can be linked). Positions are CSS (logical) px;
/// sizes are device px.
#[cfg(target_arch = "wasm32")]
mod web {
    use crate::demo;
    use crate::gallery::{self, GalleryUi};
    use crate::el::Key;
    use crate::render::Renderer;
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;

    struct State { ui: GalleryUi, r: Renderer, surface: wgpu::Surface<'static>, config: wgpu::SurfaceConfiguration, started: bool }
    thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

    #[wasm_bindgen]
    pub async fn start(canvas: web_sys::HtmlCanvasElement, sans: Vec<u8>, pixel: Vec<u8>, mono: Vec<u8>, scale: f32, hash: String) -> Result<(), JsValue> {
        console_error_panic_hook::set_once();
        let (w, h) = (canvas.width(), canvas.height());
        // WebGPU where an adapter really exists, WebGL2 otherwise.
        let instance = wgpu::util::new_instance_with_webgpu_detection(wgpu::InstanceDescriptor::new_without_display_handle()).await;
        let surface = instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas)).map_err(|e| e.to_string())?;
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions { compatible_surface: Some(&surface), ..Default::default() }).await.map_err(|e| e.to_string())?;
        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()), ..Default::default()
        }).await.map_err(|e| e.to_string())?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let config = surface.get_default_config(&adapter, w, h).map(|mut c| { c.format = format; c }).ok_or("surface not supported")?;
        surface.configure(&device, &config);
        let mut r = Renderer::new(device, queue, format, w, h);
        for (id, pw, ph, px) in demo::pictures() { r.set_picture(id, pw, ph, &px); }
        let mut ui = gallery::new_ui(crate::text::bundled(sans, pixel, mono), w, h, scale);
        if ui.state.route(&hash) { ui.relayout(); }
        STATE.with(|s| *s.borrow_mut() = Some(State { ui, r, surface, config, started: false }));
        Ok(())
    }
    fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> { STATE.with(|s| s.borrow_mut().as_mut().map(f)) }

    #[wasm_bindgen] pub fn resize(w: u32, h: u32, scale: f32) {
        with(|s| { s.config.width = w.max(1); s.config.height = h.max(1); s.surface.configure(s.r.device(), &s.config); s.ui.resize(w, h, scale); });
    }
    #[wasm_bindgen] pub fn pointer_move(x: f32, y: f32) { with(|s| s.ui.pointer_move(x, y)); }
    #[wasm_bindgen] pub fn pointer_leave() { with(|s| s.ui.pointer_leave()); }
    #[wasm_bindgen] pub fn pointer_down(x: f32, y: f32, t: f32) { with(|s| { s.ui.tick(t); s.ui.press(x, y); }); }
    #[wasm_bindgen] pub fn pointer_up(x: f32, y: f32) { with(|s| s.ui.release(x, y)); }
    #[wasm_bindgen] pub fn wheel(x: f32, y: f32, dx: f32, dy: f32) { with(|s| s.ui.wheel_xy(x, y, dx, dy)); }
    /// The page's hash changed (a link, Back): show that page. Returns whether it names one.
    #[wasm_bindgen] pub fn route(hash: &str) -> bool {
        with(|s| { let ok = s.ui.state.route(hash); if ok { s.ui.relayout(); } ok }).unwrap_or(false)
    }
    /// The current page's hash name, for the page to put in `location.hash`.
    #[wasm_bindgen] pub fn hash() -> String { with(|s| s.ui.state.page.hash()).unwrap_or_default() }
    /// A DOM key name and Shift. Returns whether the page should not act on it itself.
    #[wasm_bindgen] pub fn key(name: &str, shift: bool, t: f32) -> bool {
        let Some(k) = Key::from_name(name) else { return false };
        with(|s| { s.ui.tick(t); s.ui.key(k, shift) }).unwrap_or(false)
    }
    /// A DOM key name let go.
    #[wasm_bindgen] pub fn key_up(name: &str, t: f32) {
        let Some(k) = Key::from_name(name) else { return };
        with(|s| { s.ui.tick(t); s.ui.key_up(k) });
    }
    #[wasm_bindgen] pub fn frame(t: f32) {
        with(|s| {
            if !s.started { s.ui.restart(t); s.started = true; }
            s.ui.tick(t);
            let tex = match s.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                _ => { s.surface.configure(s.r.device(), &s.config); return; }
            };
            let view = tex.texture.create_view(&Default::default());
            let look = s.ui.state.look();
            s.r.draw(&view, &mut s.ui, &look);
            drop(view);
            s.r.queue().present(tex);
        });
    }
}
