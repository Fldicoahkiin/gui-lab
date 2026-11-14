//! The gallery in a native window (winit + wgpu): Metal on macOS, Vulkan elsewhere. It forwards the
//! pointer, wheel, keys and size to the framework, as the web page does.
//! usage: cargo run --release --bin window [page]     (page is a hash name, e.g. dialog)
use lab::el::Key;
use lab::gallery::{self, GalleryUi};
use lab::render::Renderer;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, OwnedDisplayHandle};
use winit::keyboard::{Key as WKey, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

struct Running {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    r: Renderer,
    ui: GalleryUi,
}

struct App {
    display: OwnedDisplayHandle,
    page: Option<String>,
    run: Option<Running>,
    start: Instant,
    mods: ModifiersState,
    /// Logical px.
    cursor: (f32, f32),
}

impl App {
    fn now(&self) -> f32 { self.start.elapsed().as_secs_f32() }

    fn open(&mut self, el: &ActiveEventLoop) -> Result<Running, String> {
        let attrs = Window::default_attributes().with_title("gui-lab").with_inner_size(LogicalSize::new(1280.0, 860.0));
        let window = Arc::new(el.create_window(attrs).map_err(|e| e.to_string())?);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(Box::new(self.display.clone())));
        let surface = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { compatible_surface: Some(&surface), ..Default::default() })).map_err(|e| e.to_string())?;
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).map_err(|e| e.to_string())?;
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        // A linear format: the material writes the colours it means (it encodes for sRGB surfaces itself).
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let mut config = surface.get_default_config(&adapter, size.width.max(1), size.height.max(1)).ok_or("surface not supported")?;
        config.format = format;
        surface.configure(&device, &config);
        let mut r = Renderer::new(device, queue, format, config.width, config.height);
        for (id, pw, ph, px) in lab::demo::pictures() { r.set_picture(id, pw, ph, &px); }
        let faces = lab::text::load_bundled(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).map_err(|e| e.to_string())?;
        let mut ui = gallery::new_ui(faces, config.width, config.height, window.scale_factor() as f32);
        if let Some(p) = &self.page && ui.state.route(p) { ui.relayout(); }
        ui.restart(self.now());
        Ok(Running { window, surface, config, r, ui })
    }
}

/// A winit key as the framework's key.
fn key(k: &WKey) -> Vec<Key> {
    match k {
        WKey::Named(n) => match n {
            NamedKey::Enter => vec![Key::Enter], NamedKey::Backspace => vec![Key::Backspace], NamedKey::Delete => vec![Key::Delete],
            NamedKey::Escape => vec![Key::Escape], NamedKey::Tab => vec![Key::Tab], NamedKey::Space => vec![Key::Char(' ')],
            NamedKey::ArrowUp => vec![Key::Up], NamedKey::ArrowDown => vec![Key::Down], NamedKey::ArrowLeft => vec![Key::Left], NamedKey::ArrowRight => vec![Key::Right],
            NamedKey::Home => vec![Key::Home], NamedKey::End => vec![Key::End], NamedKey::PageUp => vec![Key::PageUp], NamedKey::PageDown => vec![Key::PageDown],
            _ => vec![],
        },
        WKey::Character(s) => s.chars().map(Key::Char).collect(),
        _ => vec![],
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.run.is_some() { return; }
        match self.open(el) {
            Ok(r) => self.run = Some(r),
            Err(e) => { eprintln!("could not open a window: {e}"); el.exit(); }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let now = self.now();
        let Some(s) = self.run.as_mut() else { return };
        let scale = s.window.scale_factor() as f32;
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                s.config.width = size.width.max(1);
                s.config.height = size.height.max(1);
                s.surface.configure(s.r.device(), &s.config);
                s.ui.resize(s.config.width, s.config.height, scale);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => s.ui.resize(s.config.width, s.config.height, scale_factor as f32),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32 / scale, position.y as f32 / scale);
                s.ui.pointer_move(self.cursor.0, self.cursor.1);
            }
            WindowEvent::CursorLeft { .. } => s.ui.pointer_leave(),
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                let (x, y) = self.cursor;
                s.ui.tick(now);
                if state == ElementState::Pressed { s.ui.press(x, y) } else { s.ui.release(x, y) }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // winit: positive is away from the user (up / left); the framework scrolls down for positive.
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (-x * 40.0, -y * 40.0),
                    MouseScrollDelta::PixelDelta(p) => (-p.x as f32 / scale, -p.y as f32 / scale),
                };
                s.ui.wheel_xy(self.cursor.0, self.cursor.1, dx, dy);
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                // Shortcuts with Command / Control / Alt belong to the system.
                if self.mods.control_key() || self.mods.super_key() || self.mods.alt_key() { return; }
                s.ui.tick(now);
                for k in key(&event.logical_key) { s.ui.key(k, self.mods.shift_key()); }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                s.ui.tick(now);
                for k in key(&event.logical_key) { s.ui.key_up(k); }
            }
            WindowEvent::RedrawRequested => {
                s.ui.tick(now);
                let frame = match s.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    _ => { s.surface.configure(s.r.device(), &s.config); return; }
                };
                let view = frame.texture.create_view(&Default::default());
                let look = s.ui.state.look();
                s.r.draw(&view, &mut s.ui, &look);
                drop(view);
                s.r.queue().present(frame);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        // The field keeps moving (fronts, condensing, the ground), so draw continuously.
        if let Some(s) = &self.run { s.window.request_redraw(); }
    }
}

fn main() -> std::process::ExitCode {
    let el = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => { eprintln!("no window system here: {e}"); return std::process::ExitCode::FAILURE; }
    };
    let mut app = App { display: el.owned_display_handle(), page: std::env::args().nth(1), run: None, start: Instant::now(), mods: ModifiersState::empty(), cursor: (0.0, 0.0) };
    match el.run_app(&mut app) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => { eprintln!("{e}"); std::process::ExitCode::FAILURE }
    }
}
