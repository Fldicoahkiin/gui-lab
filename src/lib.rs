pub mod app;
pub mod text;

#[cfg(target_arch = "wasm32")]
mod web {
    use crate::app::App;
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;

    struct State { app: App, surface: wgpu::Surface<'static>, config: wgpu::SurfaceConfiguration }
    thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

    #[wasm_bindgen]
    pub async fn start(canvas: web_sys::HtmlCanvasElement, font: Vec<u8>, scale: f32) -> Result<(), JsValue> {
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
        let app = App::new(device, queue, format, font, w, h, scale);
        STATE.with(|s| *s.borrow_mut() = Some(State { app, surface, config }));
        Ok(())
    }
    fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> { STATE.with(|s| s.borrow_mut().as_mut().map(f)) }

    #[wasm_bindgen] pub fn resize(w: u32, h: u32, scale: f32) { with(|s| { s.config.width = w.max(1); s.config.height = h.max(1); s.surface.configure(s.app.device(), &s.config); s.app.resize(w, h, scale); }); }
    #[wasm_bindgen] pub fn pointer(x: f32, y: f32, strength: f32) { with(|s| s.app.pointer(x, y, strength)); }
    #[wasm_bindgen] pub fn press(x: f32, y: f32, t: f32) { with(|s| s.app.press(x, y, t)); }
    #[wasm_bindgen] pub fn set_params(pitch: f32, pic_pitch: f32, glitch: f32, jitter: f32, square: bool, light: bool) {
        with(|s| s.app.params = crate::app::Params { pitch, pic_pitch, glitch, jitter, square, light });
    }
    #[wasm_bindgen] pub fn frame(t: f32) {
        with(|s| {
            let tex = match s.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                _ => { s.surface.configure(s.app.device(), &s.config); return; }
            };
            let view = tex.texture.create_view(&Default::default());
            s.app.render(&view, t);
            drop(view);
            s.app.queue().present(tex);
        });
    }
}
